//! Agent 抽象接口定义
//!
//! 定义所有 Agent 必须实现的 `Agent` trait，以及 Agent 间通信的消息格式。
//!
//! # 设计原则
//! - **异步优先**：所有 Agent 操作都是异步的，适配 Tokio 运行时
//! - **消息驱动**：Agent 之间通过 `AgentMessage` 通信，不直接调用彼此方法
//! - **可扩展**：通过 trait 实现添加新 Agent，无需修改框架代码

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AgentError;

// ============================================================
// Agent 消息定义
// ============================================================

/// Agent 间通信的消息结构
///
/// 所有 Agent 通过 MessageBus 传递此结构进行协作。
/// 支持点对点通信（指定 `to`）和广播（`to = "all"`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    /// 消息唯一标识符
    pub id: String,

    /// 发送方 Agent 名称
    pub from: String,

    /// 接收方 Agent 名称（"all" 表示广播给所有 Agent）
    pub to: String,

    /// 消息正文（自然语言或结构化指令）
    pub content: String,

    /// 附加上下文信息（JSON 格式，携带结构化数据）
    pub context: serde_json::Value,

    /// 请求期临时上下文，不参与序列化/持久化。
    ///
    /// 用于传递 API Key 等敏感配置；Agent 回复默认不会继承该字段。
    #[serde(skip, default)]
    pub transient_context: serde_json::Value,

    /// 关联的任务 ID（用于追踪执行流）
    pub task_id: Option<String>,

    /// 回复通道接收端 ID（用于构建请求-响应模式）
    /// 已序列化存储，通过 MessageBus 的 reply_to_map 路由
    pub reply_id: Option<String>,

    /// 消息类型标识（可选，如 "plan", "tool_call", "result"）
    pub msg_type: String,
}

impl AgentMessage {
    /// 创建一条新消息
    ///
    /// 自动生成唯一消息 ID。
    pub fn new(from: &str, to: &str, content: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            from: from.to_string(),
            to: to.to_string(),
            content: content.to_string(),
            context: serde_json::Value::Null,
            transient_context: serde_json::Value::Null,
            task_id: None,
            reply_id: None,
            msg_type: "generic".to_string(),
        }
    }

    /// 创建带任务 ID 的消息
    pub fn new_with_task(from: &str, to: &str, content: &str, task_id: &str) -> Self {
        let mut msg = Self::new(from, to, content);
        msg.task_id = Some(task_id.to_string());
        msg
    }

    /// 创建回复消息（复制原消息的相关字段）
    pub fn reply_to(&self, content: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            from: self.to.clone(),
            to: self.from.clone(),
            content: content.to_string(),
            context: self.context.clone(),
            transient_context: serde_json::Value::Null,
            task_id: self.task_id.clone(),
            reply_id: self.reply_id.clone(),
            msg_type: "reply".to_string(),
        }
    }

    /// 设置消息类型
    pub fn with_type(mut self, msg_type: &str) -> Self {
        self.msg_type = msg_type.to_string();
        self
    }

    /// 设置上下文数据
    pub fn with_context(mut self, context: serde_json::Value) -> Self {
        self.context = merge_message_context(self.context, context);
        self
    }

    /// 设置请求期临时上下文。
    pub fn with_transient_context(mut self, context: serde_json::Value) -> Self {
        self.transient_context = merge_message_context(self.transient_context, context);
        self
    }

    /// 设置任务 ID
    pub fn with_task_id(mut self, task_id: &str) -> Self {
        self.task_id = Some(task_id.to_string());
        self
    }
}

fn merge_message_context(base: serde_json::Value, next: serde_json::Value) -> serde_json::Value {
    match (base, next) {
        (serde_json::Value::Object(mut base), serde_json::Value::Object(next)) => {
            for (key, value) in next {
                base.insert(key, value);
            }
            serde_json::Value::Object(base)
        }
        (_, next) => next,
    }
}

// ============================================================
// Agent 能力定义
// ============================================================

/// Agent 能力等级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CapabilityLevel {
    /// 基础能力（如简单对话）
    Basic,
    /// 高级能力（如复杂推理、工具使用）
    Advanced,
    /// 专家级别（如特定领域知识）
    Expert,
}

/// Agent 能力描述
///
/// 每个 Agent 注册自己的能力列表，Orchestrator 根据能力匹配合适的 Agent。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    /// 能力名称（如 "chat", "planning", "code_execution"）
    pub name: String,

    /// 能力描述
    pub description: String,

    /// 能力等级
    pub level: CapabilityLevel,
}

impl Capability {
    /// 聊天能力
    pub fn chat() -> Self {
        Self {
            name: "chat".to_string(),
            description: "通用对话能力，可理解和回复自然语言".to_string(),
            level: CapabilityLevel::Basic,
        }
    }

    /// 任务规划能力
    pub fn planning() -> Self {
        Self {
            name: "planning".to_string(),
            description: "将复杂目标分解为可执行步骤".to_string(),
            level: CapabilityLevel::Advanced,
        }
    }

    /// 代码执行能力
    pub fn code_execution() -> Self {
        Self {
            name: "code_execution".to_string(),
            description: "执行代码、脚本和命令".to_string(),
            level: CapabilityLevel::Advanced,
        }
    }

    /// 信息检索能力
    pub fn retrieval() -> Self {
        Self {
            name: "retrieval".to_string(),
            description: "从知识库或外部源检索信息".to_string(),
            level: CapabilityLevel::Basic,
        }
    }

    /// 工具使用能力
    pub fn tool_use() -> Self {
        Self {
            name: "tool_use".to_string(),
            description: "调用外部工具和 API".to_string(),
            level: CapabilityLevel::Advanced,
        }
    }

    /// 记忆管理能力
    pub fn memory() -> Self {
        Self {
            name: "memory".to_string(),
            description: "管理对话历史和长期记忆".to_string(),
            level: CapabilityLevel::Basic,
        }
    }

    /// 审查能力
    pub fn review() -> Self {
        Self {
            name: "review".to_string(),
            description: "审查执行结果、风险和缺失验证".to_string(),
            level: CapabilityLevel::Expert,
        }
    }

    /// 演进建议能力
    pub fn evolution() -> Self {
        Self {
            name: "evolution".to_string(),
            description: "总结任务经验并提出后续改进建议".to_string(),
            level: CapabilityLevel::Advanced,
        }
    }
}

// ============================================================
// Agent Trait 定义
// ============================================================

/// Agent 核心接口
///
/// 所有 Agent 必须实现此 trait。
///
/// # 实现指南
/// 1. `name()` — 返回 Agent 唯一名称
/// 2. `capabilities()` — 声明 Agent 的能力列表
/// 3. `handle_message()` — 处理收到的消息并返回回复
///
/// # 示例
/// ```ignore
/// struct MyAgent;
///
/// #[async_trait]
/// impl Agent for MyAgent {
///     fn name(&self) -> &str { "MyAgent" }
///     fn capabilities(&self) -> Vec<Capability> { vec![Capability::chat()] }
///     
///     async fn handle_message(
///         &mut self,
///         msg: AgentMessage,
///     ) -> Result<Vec<AgentMessage>, AgentError> {
///         let reply = msg.reply_to("收到消息！");
///         Ok(vec![reply])
///     }
/// }
/// ```
#[async_trait]
pub trait Agent: Send + Sync {
    /// 返回 Agent 的唯一名称
    fn name(&self) -> &str;

    /// 返回 Agent 的能力列表
    ///
    /// Orchestrator 根据此列表将任务分派给合适的 Agent。
    fn capabilities(&self) -> Vec<Capability>;

    /// 处理收到的消息
    ///
    /// # 参数
    /// - `msg`：收到的 AgentMessage
    ///
    /// # 返回值
    /// - 成功时返回要发回的回复消息列表（可能为空）
    /// - 失败时返回 AgentError
    async fn handle_message(&mut self, msg: AgentMessage) -> Result<Vec<AgentMessage>, AgentError>;

    /// Agent 的主循环（可选覆盖）
    ///
    /// 默认实现：从接收器持续读取消息，调用 handle_message 处理，
    /// 将回复发送回总线。
    ///
    /// # 参数
    /// - `rx`：消息接收端
    /// - `tx`：消息发送端（总线的发送器）
    async fn run(
        &mut self,
        mut rx: tokio::sync::mpsc::UnboundedReceiver<AgentMessage>,
        tx: tokio::sync::broadcast::Sender<AgentMessage>,
    ) {
        let agent_name = self.name().to_string();
        tracing::info!("Agent [{agent_name}] 启动，等待消息...");

        while let Some(msg) = rx.recv().await {
            tracing::debug!("Agent [{agent_name}] 收到消息: {:?}", msg.id);

            match self.handle_message(msg).await {
                Ok(replies) => {
                    for reply in replies {
                        if let Err(e) = tx.send(reply) {
                            tracing::error!("Agent [{agent_name}] 发送回复失败: {e}");
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Agent [{agent_name}] 处理消息出错: {e}");
                }
            }
        }

        tracing::info!("Agent [{agent_name}] 已停止");
    }
}

// ============================================================
// Agent 状态
// ============================================================

/// Agent 运行状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentStatus {
    /// 空闲中，等待新任务
    Idle,
    /// 忙碌中，正在处理消息
    Busy,
    /// 发生错误
    Error(String),
    /// 已停止
    Stopped,
}

/// Agent 运行时信息（用于前端状态面板展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    /// Agent 名称
    pub name: String,
    /// 当前状态
    pub status: AgentStatus,
    /// 能力列表
    pub capabilities: Vec<Capability>,
    /// 当前处理的任务 ID（如果有）
    pub current_task: Option<String>,
    /// 已处理的消息数量
    pub messages_processed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_to_preserves_step_context() {
        let msg = AgentMessage::new("Orchestrator", "Executor", "执行步骤").with_context(
            serde_json::json!({
                "taskId": "task-1",
                "stepId": "task-1-2"
            }),
        );

        let reply = msg
            .reply_to("完成")
            .with_context(serde_json::json!({ "success": true }));

        assert_eq!(reply.context["taskId"], "task-1");
        assert_eq!(reply.context["stepId"], "task-1-2");
        assert_eq!(reply.context["success"], true);
    }

    #[test]
    fn transient_context_is_not_serialized_or_copied_to_replies() {
        let msg = AgentMessage::new("User", "Planner", "规划任务")
            .with_context(serde_json::json!({ "safe": true }))
            .with_transient_context(serde_json::json!({
                "plannerLlmSettings": { "apiKey": "sk-secret" }
            }));

        let serialized = serde_json::to_string(&msg).unwrap();
        let reply = msg.reply_to("ok");

        assert!(!serialized.contains("sk-secret"));
        assert!(reply.transient_context.is_null());
        assert_eq!(reply.context["safe"], serde_json::json!(true));
    }
}
