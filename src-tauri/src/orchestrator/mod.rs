//! 调度器模块 — 多 Agent 协同任务调度中心
//!
//! Orchestrator 是系统的核心调度组件，负责：
//! 1. 管理所有已注册的 Agent
//! 2. 接收用户输入，调用 PlannerAgent 生成执行计划
//! 3. 按计划调度各 Agent 执行任务
//! 4. 收集执行结果并返回给用户

use crate::agent::echo_agent::EchoAgent;
use crate::agent::executor_agent::ExecutorAgent;
use crate::agent::memory_agent::MemoryAgent;
use crate::agent::planner_agent::PlannerAgent;
use crate::agent::tool_agent::ToolAgent;
use crate::agent::traits::{Agent, AgentMessage};
use crate::bus::message_bus::MessageBus;
use crate::error::AgentError;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Agent 运行时信息
struct AgentRuntime {
    /// 向该 Agent 发送消息的通道
    sender: tokio::sync::mpsc::UnboundedSender<AgentMessage>,
    /// Agent 名称
    name: String,
}

/// 任务执行结果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskResult {
    /// 任务 ID
    pub task_id: String,
    /// 执行状态
    pub status: String,
    /// 执行步骤
    pub steps: Vec<serde_json::Value>,
    /// 最终输出
    pub output: String,
}

/// 调度器 — 多 Agent 系统的中央调度组件
pub struct Orchestrator {
    /// 已注册的 Agent 运行时信息
    agents: HashMap<String, AgentRuntime>,
    /// 消息总线
    bus: Arc<MessageBus>,
    /// 任务执行结果缓存
    tasks: HashMap<String, TaskResult>,
}

impl Orchestrator {
    /// 创建新的调度器
    pub fn new(bus: Arc<MessageBus>) -> Self {
        Self {
            agents: HashMap::new(),
            bus,
            tasks: HashMap::new(),
        }
    }

    /// 注册所有内置 Agent 并启动其运行循环
    ///
    /// 内置 Agent 包括：Echo、Planner、Executor、Memory、Tool。
    pub async fn register_builtin_agents(&mut self) {
        tracing::info!("[Orchestrator] 正在注册内置 Agent...");

        self.register_and_spawn(Box::new(EchoAgent::new())).await;
        self.register_and_spawn(Box::new(PlannerAgent::new())).await;
        self.register_and_spawn(Box::new(ExecutorAgent::new())).await;
        self.register_and_spawn(Box::new(MemoryAgent::new())).await;
        self.register_and_spawn(Box::new(ToolAgent::default())).await;

        tracing::info!(
            "[Orchestrator] 已注册 {} 个 Agent: {:?}",
            self.agents.len(),
            self.list_agents()
        );
    }

    /// 注册单个 Agent 并启动其运行循环
    async fn register_and_spawn(&mut self, mut agent: Box<dyn Agent>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<AgentMessage>();
        let name = agent.name().to_string();

        // 启动 Agent 的主循环
        let bus_tx = self.bus.sender();
        let agent_name = name.clone();

        tokio::spawn(async move {
            tracing::info!("[{}] Agent 启动", agent_name);
            agent.run(rx, bus_tx).await;
        });

        self.agents.insert(
            name.clone(),
            AgentRuntime {
                sender: tx,
                name,
            },
        );
    }

    /// 提交用户任务。
    ///
    /// 默认入口交给 PlannerAgent（产品侧展示为“协调员/总控”）。
    pub async fn submit_task(&mut self, user_input: &str) -> Result<String, AgentError> {
        self.submit_task_to_agent("Planner", user_input, "plan_request").await
    }

    /// 提交用户任务到指定 Agent。
    ///
    /// 注意：Agent 的主循环通过 mpsc 接收消息，回复再发回 MessageBus。
    /// 因此用户入口必须使用 `send_to_agent`，不能直接 `bus.publish`，否则没有桥接订阅者时消息会丢失。
    pub async fn submit_task_to_agent(
        &mut self,
        agent_name: &str,
        user_input: &str,
        msg_type: &str,
    ) -> Result<String, AgentError> {
        let task_id = uuid::Uuid::new_v4().to_string();
        tracing::info!(
            "[Orchestrator] 接收任务 [{}]，目标 Agent [{}]: {}",
            task_id,
            agent_name,
            user_input
        );

        self.tasks.insert(
            task_id.clone(),
            TaskResult {
                task_id: task_id.clone(),
                status: "执行中".to_string(),
                steps: Vec::new(),
                output: String::new(),
            },
        );

        let msg = AgentMessage::new("Orchestrator", agent_name, user_input)
            .with_task_id(&task_id)
            .with_type(msg_type);

        self.send_to_agent(agent_name, msg)?;

        tracing::info!(
            "[Orchestrator] 任务 [{}] 已发送给 Agent [{}]",
            task_id,
            agent_name
        );

        Ok(task_id)
    }

    /// 向指定 Agent 发送消息（通过 mpsc 通道）
    pub fn send_to_agent(
        &self,
        agent_name: &str,
        msg: AgentMessage,
    ) -> Result<(), AgentError> {
        match self.agents.get(agent_name) {
            Some(runtime) => {
                runtime.sender.send(msg).map_err(|e| {
                    AgentError::BusError(format!(
                        "无法向 Agent [{}] 发送消息: {}",
                        agent_name, e
                    ))
                })
            }
            None => Err(AgentError::BusError(format!(
                "Agent [{}] 未注册",
                agent_name
            ))),
        }
    }

    /// 查询任务执行结果
    pub fn get_task_result(&self, task_id: &str) -> Option<&TaskResult> {
        self.tasks.get(task_id)
    }

    /// 列出所有已注册的 Agent 名称
    pub fn list_agents(&self) -> Vec<String> {
        self.agents.keys().cloned().collect()
    }

    /// 获取消息总线的引用
    pub fn bus(&self) -> &Arc<MessageBus> {
        &self.bus
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_orchestrator_create_and_register() {
        let bus = Arc::new(MessageBus::new());
        let mut orch = Orchestrator::new(bus);
        orch.register_builtin_agents().await;

        let agents = orch.list_agents();
        assert!(!agents.is_empty());
        assert!(agents.contains(&"Echo".to_string()));
        assert!(agents.contains(&"Planner".to_string()));
    }

    #[tokio::test]
    async fn test_submit_task() {
        let bus = Arc::new(MessageBus::new());
        let mut orch = Orchestrator::new(bus.clone());

        // 需要先订阅总线，否则 broadcast 消息会丢失
        let mut _rx = bus.subscribe();

        orch.register_builtin_agents().await;

        let task_id = orch
            .submit_task("帮我整理今日新闻")
            .await
            .unwrap();
        assert!(!task_id.is_empty());
        assert!(orch.get_task_result(&task_id).is_some());
    }

    /// 测试 — send_to_agent 向注册的 Agent 发送消息
    /// 验证：通过 mpsc 通道可以将消息发送给已注册的 Agent
    #[tokio::test]
    async fn test_send_to_registered_agent() {
        let bus = Arc::new(MessageBus::new());
        let mut orch = Orchestrator::new(bus);
        orch.register_builtin_agents().await;

        // 向 Echo Agent 发送消息
        let msg = AgentMessage::new("Test", "Echo", "测试直连消息");
        let result = orch.send_to_agent("Echo", msg);

        assert!(result.is_ok(), "应向已注册的 Agent 发送成功");
    }

    /// 测试 — send_to_agent 向未注册 Agent 发送应失败
    /// 验证：向不存在的 Agent 名称发送消息会返回错误
    #[tokio::test]
    async fn test_send_to_unregistered_agent_errors() {
        let bus = Arc::new(MessageBus::new());
        let orch = Orchestrator::new(bus);

        // 向不存在的 Agent 发送
        let msg = AgentMessage::new("Test", "GhostAgent", "幽灵消息");
        let result = orch.send_to_agent("GhostAgent", msg);

        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("未注册"));
    }

    /// 测试 — 获取不存在的任务结果返回 None
    /// 验证：查询未提交的任务 ID 返回 None
    #[tokio::test]
    async fn test_get_nonexistent_task() {
        let bus = Arc::new(MessageBus::new());
        let orch = Orchestrator::new(bus);

        let result = orch.get_task_result("non-existent-id");
        assert!(result.is_none());
    }

    /// 测试 — 注册所有内置 Agent 后列表完整
    /// 验证：register_builtin_agents 注册了全部 5 个内置 Agent
    #[tokio::test]
    async fn test_all_builtin_agents_registered() {
        let bus = Arc::new(MessageBus::new());
        let mut orch = Orchestrator::new(bus);
        orch.register_builtin_agents().await;

        let agents = orch.list_agents();
        assert_eq!(agents.len(), 5);
        assert!(agents.contains(&"Echo".to_string()));
        assert!(agents.contains(&"Planner".to_string()));
        assert!(agents.contains(&"Executor".to_string()));
        assert!(agents.contains(&"Memory".to_string()));
        assert!(agents.contains(&"Tool".to_string()));
    }

    /// 测试 — 任务提交后状态为"执行中"
    /// 验证：submit_task 创建的任务初始状态正确
    #[tokio::test]
    async fn test_task_initial_status() {
        let bus = Arc::new(MessageBus::new());
        let mut _rx = bus.subscribe();
        let mut orch = Orchestrator::new(bus);
        orch.register_builtin_agents().await;

        let task_id = orch.submit_task("测试任务状态").await.unwrap();
        let result = orch.get_task_result(&task_id).unwrap();

        assert_eq!(result.status, "执行中");
        assert_eq!(result.task_id, task_id);
        assert!(result.output.is_empty());
    }

    /// 测试 — bus() 方法返回正确的引用
    /// 验证：可以通过 orchestrator.bus() 获取消息总线引用
    #[tokio::test]
    async fn test_bus_reference() {
        let bus = Arc::new(MessageBus::new());
        let orch = Orchestrator::new(bus.clone());
        let bus_ref = orch.bus();

        // 验证返回的是同一实例
        assert_eq!(Arc::strong_count(&bus), 2); // bus + bus_ref
        assert_eq!(bus_ref.subscriber_count(), 0);
    }

    /// 测试 — TaskResult 序列化
    /// 验证：TaskResult 可以正确序列化/反序列化
    #[test]
    fn test_task_result_serialization() {
        let result = TaskResult {
            task_id: "t-1".to_string(),
            status: "完成".to_string(),
            steps: vec![serde_json::json!({"step": 1})],
            output: "任务已成功完成".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        let restored: TaskResult = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.task_id, "t-1");
        assert_eq!(restored.status, "完成");
    }
}
