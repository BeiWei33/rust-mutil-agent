//! 任务规划 Agent（PlannerAgent）
//!
//! 负责将用户的自然语言目标分解为结构化的执行计划。
//! 计划由多个步骤（Step）组成，每个步骤指定了负责的 Agent 和具体指令。
//!
//! # 工作流程
//! 1. 接收用户目标（通过 AgentMessage）
//! 2. 分析目标复杂度，生成执行计划
//! 3. 将计划步骤封装为消息，发布到总线供其他 Agent 执行
//!
//! # 计划结构
//! ```text
//! TaskPlan {
//!     task_id: "uuid",
//!     goal: "整理今日热点新闻",
//!     steps: [
//!         Step { agent: "Tool", instruction: "搜索今日新闻" },
//!         Step { agent: "Executor", instruction: "总结每条新闻" },
//!         Step { agent: "Echo", instruction: "生成最终报告" },
//!     ]
//! }
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::traits::{Agent, AgentMessage, Capability};
use crate::error::AgentError;

// ============================================================
// 计划数据结构
// ============================================================

/// 执行计划的单个步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    /// 步骤 ID（在计划内唯一）
    pub step_id: String,

    /// 步骤序号（从 1 开始）
    pub order: u32,

    /// 负责执行的 Agent 名称
    pub agent: String,

    /// 该步骤的具体指令
    pub instruction: String,

    /// 依赖的前置步骤 ID 列表（用于 DAG 图结构，预留）
    pub depends_on: Vec<String>,

    /// 步骤状态
    pub status: StepStatus,
}

/// 步骤执行状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StepStatus {
    /// 等待执行
    Pending,

    /// 执行中
    Running,

    /// 执行完成
    Completed,

    /// 执行失败
    Failed(String),
}

/// 完整的任务执行计划
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    /// 关联的任务 ID
    pub task_id: String,

    /// 用户原始目标描述
    pub goal: String,

    /// 执行步骤列表
    pub steps: Vec<PlanStep>,

    /// 计划创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl TaskPlan {
    /// 根据目标生成简单线性计划（演示用）
    ///
    /// 实际生产环境中，这里应调用 LLM 来智能分解任务。
    pub fn from_goal(goal: &str) -> Self {
        Self::from_goal_with_id(goal, Uuid::new_v4().to_string())
    }

    /// 根据目标和外部任务 ID 生成简单线性计划。
    pub fn from_goal_with_id(goal: &str, task_id: impl Into<String>) -> Self {
        let task_id = task_id.into();

        // 演示：根据关键词简单推断所需步骤
        let steps = if goal.contains("搜索") || goal.contains("查找") || goal.contains("新闻")
        {
            vec![
                PlanStep {
                    step_id: format!("{task_id}-1"),
                    order: 1,
                    agent: "Tool".to_string(),
                    instruction: format!("搜索相关信息: {goal}"),
                    depends_on: vec![],
                    status: StepStatus::Pending,
                },
                PlanStep {
                    step_id: format!("{task_id}-2"),
                    order: 2,
                    agent: "Executor".to_string(),
                    instruction: format!("整理和分析搜索结果: {goal}"),
                    depends_on: vec![format!("{task_id}-1")],
                    status: StepStatus::Pending,
                },
                PlanStep {
                    step_id: format!("{task_id}-3"),
                    order: 3,
                    agent: "Memory".to_string(),
                    instruction: format!("存储结果到知识库: {goal}"),
                    depends_on: vec![format!("{task_id}-2")],
                    status: StepStatus::Pending,
                },
            ]
        } else if goal.contains("代码") || goal.contains("编程") || goal.contains("写") {
            vec![
                PlanStep {
                    step_id: format!("{task_id}-1"),
                    order: 1,
                    agent: "Planner".to_string(),
                    instruction: format!("分析编码需求: {goal}"),
                    depends_on: vec![],
                    status: StepStatus::Pending,
                },
                PlanStep {
                    step_id: format!("{task_id}-2"),
                    order: 2,
                    agent: "Executor".to_string(),
                    instruction: format!("编写代码: {goal}"),
                    depends_on: vec![format!("{task_id}-1")],
                    status: StepStatus::Pending,
                },
            ]
        } else {
            // 默认单步计划
            vec![PlanStep {
                step_id: format!("{task_id}-1"),
                order: 1,
                agent: "Echo".to_string(),
                instruction: format!("处理用户请求: {goal}"),
                depends_on: vec![],
                status: StepStatus::Pending,
            }]
        };

        Self {
            task_id,
            goal: goal.to_string(),
            steps,
            created_at: chrono::Utc::now(),
        }
    }

    /// 根据目标、任务 ID 和项目上下文生成计划。
    pub fn from_goal_with_context(
        goal: &str,
        task_id: impl Into<String>,
        context: &serde_json::Value,
    ) -> Self {
        let task_id = task_id.into();
        if is_software_goal(goal) {
            if let Some(project_context) = context.get("projectPlanningContext") {
                return Self::from_software_goal(goal, task_id, project_context);
            }
        }

        Self::from_goal_with_id(goal, task_id)
    }

    fn from_software_goal(
        goal: &str,
        task_id: String,
        project_context: &serde_json::Value,
    ) -> Self {
        let tech_stack = project_context
            .pointer("/snapshot/techStack")
            .and_then(|value| value.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str())
                    .take(8)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "未知技术栈".to_string());

        let search_query = project_context
            .get("searchQuery")
            .and_then(|value| value.as_str())
            .unwrap_or("未生成搜索词");

        let related_files = project_context
            .get("searchMatches")
            .and_then(|value| value.as_array())
            .map(|matches| {
                let mut paths = Vec::new();
                for item in matches {
                    if let Some(path) = item.get("path").and_then(|value| value.as_str()) {
                        if !paths.iter().any(|existing: &&str| *existing == path) {
                            paths.push(path);
                        }
                    }
                    if paths.len() >= 5 {
                        break;
                    }
                }
                paths.join(", ")
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "暂无直接命中文件，优先查看项目关键文件".to_string());

        let recommended_commands = project_context
            .pointer("/snapshot/recommendedCommands")
            .and_then(|value| value.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.get("command").and_then(|command| command.as_str()))
                    .take(4)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "cargo check, cargo test".to_string());

        let steps = vec![
            PlanStep {
                step_id: format!("{task_id}-1"),
                order: 1,
                agent: "Planner".to_string(),
                instruction: format!(
                    "基于项目快照理解软件任务: {goal}。当前技术栈: {tech_stack}。"
                ),
                depends_on: vec![],
                status: StepStatus::Pending,
            },
            PlanStep {
                step_id: format!("{task_id}-2"),
                order: 2,
                agent: "Tool".to_string(),
                instruction: format!(
                    "只读检索相关源码。搜索词: {search_query}。优先关注: {related_files}。"
                ),
                depends_on: vec![format!("{task_id}-1")],
                status: StepStatus::Pending,
            },
            PlanStep {
                step_id: format!("{task_id}-3"),
                order: 3,
                agent: "Executor".to_string(),
                instruction: format!(
                    "结合项目结构形成实现方案和模拟执行结果，不直接修改文件: {goal}。建议验证命令: {recommended_commands}。"
                ),
                depends_on: vec![format!("{task_id}-2")],
                status: StepStatus::Pending,
            },
            PlanStep {
                step_id: format!("{task_id}-4"),
                order: 4,
                agent: "Memory".to_string(),
                instruction: format!(
                    "记录本轮项目事实、相关文件和后续实现建议: {goal}。"
                ),
                depends_on: vec![format!("{task_id}-3")],
                status: StepStatus::Pending,
            },
        ];

        Self {
            task_id,
            goal: goal.to_string(),
            steps,
            created_at: chrono::Utc::now(),
        }
    }
}

fn is_software_goal(goal: &str) -> bool {
    let lower = goal.to_lowercase();
    [
        "项目", "代码", "编程", "实现", "修复", "bug", "测试", "构建", "前端", "后端", "页面",
        "组件", "文件", "ipc", "agent", "rust", "react", "tauri", "store",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword))
}

// ============================================================
// PlannerAgent 实现
// ============================================================

/// 任务规划 Agent
///
/// 接收用户目标，生成执行计划，并将计划分发给其他 Agent。
pub struct PlannerAgent {
    /// 已创建的计划数量
    plans_created: u64,
}

impl PlannerAgent {
    pub fn new() -> Self {
        Self { plans_created: 0 }
    }
}

impl Default for PlannerAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for PlannerAgent {
    fn name(&self) -> &str {
        "Planner"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![Capability::planning()]
    }

    async fn handle_message(&mut self, msg: AgentMessage) -> Result<Vec<AgentMessage>, AgentError> {
        self.plans_created += 1;

        // 根据消息内容生成执行计划；优先沿用 Orchestrator 创建的 task_id。
        let plan = match msg.task_id.as_deref() {
            Some(task_id) => TaskPlan::from_goal_with_context(&msg.content, task_id, &msg.context),
            None => TaskPlan::from_goal(&msg.content),
        };

        tracing::info!(
            "PlannerAgent 为任务 {} 创建了 {} 个步骤的计划",
            plan.task_id,
            plan.steps.len()
        );

        // 序列化计划为 JSON，放入消息 context
        let context = serde_json::to_value(&plan)
            .map_err(|e| AgentError::Internal(format!("计划序列化失败: {e}")))?;

        // 构建包含计划的回复消息
        let reply = AgentMessage::new(
            self.name(),
            &msg.from,
            &format!("已为任务创建计划，共 {} 个步骤", plan.steps.len()),
        )
        .with_type("plan_created")
        .with_context(context)
        .with_task_id(&plan.task_id);

        // 为每个步骤生成分派消息（发送给对应的 Agent）
        let mut dispatch_messages = Vec::new();
        for step in &plan.steps {
            let dispatch_msg = AgentMessage::new(self.name(), &step.agent, &step.instruction)
                .with_type("plan_step")
                .with_task_id(&plan.task_id);

            dispatch_messages.push(dispatch_msg);
        }

        // 合并回复和分派消息
        let mut all_messages = vec![reply];
        all_messages.extend(dispatch_messages);

        Ok(all_messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_planner_simple_goal() {
        let mut planner = PlannerAgent::new();

        let msg = AgentMessage::new("User", "Planner", "帮我整理今日新闻");
        let replies = planner.handle_message(msg).await.unwrap();

        // 应该有 1 个回复 + 3 个分派消息
        assert_eq!(replies.len(), 4);
        assert_eq!(replies[0].msg_type, "plan_created");

        // 检查分派消息的目标
        let targets: Vec<&str> = replies.iter().skip(1).map(|m| m.to.as_str()).collect();
        assert!(targets.contains(&"Tool"));
        assert!(targets.contains(&"Executor"));
        assert!(targets.contains(&"Memory"));
    }

    #[tokio::test]
    async fn test_planner_code_request() {
        let mut planner = PlannerAgent::new();

        let msg = AgentMessage::new("User", "Planner", "写一个排序函数");
        let replies = planner.handle_message(msg).await.unwrap();

        // 编码请求应有 1 + 2 = 3 条消息
        assert_eq!(replies.len(), 3);
    }

    /// 测试 — 默认目标生成单步计划
    /// 验证：不包含任何关键词的目标只生成一个步骤（Echo）
    #[tokio::test]
    async fn test_planner_default_single_step() {
        let mut planner = PlannerAgent::new();

        let msg = AgentMessage::new("User", "Planner", "你好");
        let replies = planner.handle_message(msg).await.unwrap();

        // 只有 1 回复 + 1 分派 = 2 条消息
        assert_eq!(replies.len(), 2);
        // 分派消息应发送给 Echo
        assert_eq!(replies[1].to, "Echo");
    }

    /// 测试 — 计划回复中包含正确的类型标记
    /// 验证：第一条回复的 msg_type 为 "plan_created"
    #[tokio::test]
    async fn test_planner_reply_type() {
        let mut planner = PlannerAgent::new();

        let msg = AgentMessage::new("User", "Planner", "帮我搜索信息");
        let replies = planner.handle_message(msg).await.unwrap();

        // 第一条消息是计划创建确认
        assert_eq!(replies[0].msg_type, "plan_created");
        // 后续消息是分派步骤
        assert_eq!(replies[1].msg_type, "plan_step");
    }

    /// 测试 — 计划包含 task_id
    /// 验证：创建的计划关联了 task_id，且分派消息也包含同一 task_id
    #[tokio::test]
    async fn test_planner_task_id_present() {
        let mut planner = PlannerAgent::new();

        let msg = AgentMessage::new("User", "Planner", "搜索新闻");
        let replies = planner.handle_message(msg).await.unwrap();

        // 检查计划回复包含 task_id
        assert!(replies[0].task_id.is_some());
        let tid = replies[0].task_id.as_ref().unwrap();

        // 所有分派消息应包含相同的 task_id
        for reply in replies.iter().skip(1) {
            assert_eq!(reply.task_id.as_ref().unwrap(), tid);
        }
    }

    /// 测试 — 计划上下文包含完整 TaskPlan JSON
    /// 验证：计划回复的 context 字段包含序列化的 TaskPlan
    #[tokio::test]
    async fn test_planner_context_contains_plan() {
        let mut planner = PlannerAgent::new();

        let msg = AgentMessage::new("User", "Planner", "搜索新闻");
        let replies = planner.handle_message(msg).await.unwrap();

        // 计划回复的 context 应包含 TaskPlan
        let ctx = &replies[0].context;
        assert!(ctx.get("task_id").is_some());
        assert!(ctx.get("goal").is_some());
        assert!(ctx.get("steps").is_some());

        // 验证步骤数组
        let steps = ctx["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 3);
    }

    /// 测试 — 多次规划后计划计数递增
    /// 验证：两次规划后 plans_created 递增
    #[tokio::test]
    async fn test_planner_multiple_calls() {
        let mut planner = PlannerAgent::new();

        // 第一次规划
        let msg1 = AgentMessage::new("User", "Planner", "搜索新闻");
        planner.handle_message(msg1).await.unwrap();
        assert_eq!(planner.plans_created, 1);

        // 第二次规划
        let msg2 = AgentMessage::new("User", "Planner", "写代码");
        planner.handle_message(msg2).await.unwrap();
        assert_eq!(planner.plans_created, 2);
    }

    /// 测试 — 计划步骤的依赖关系
    /// 验证：搜索类任务的步骤 2 依赖于步骤 1
    #[tokio::test]
    async fn test_planner_step_dependencies() {
        let mut planner = PlannerAgent::new();

        let msg = AgentMessage::new("User", "Planner", "今天有什么新闻？");
        let replies = planner.handle_message(msg).await.unwrap();

        // 从 context 解析 TaskPlan
        let plan: crate::agent::planner_agent::TaskPlan =
            serde_json::from_value(replies[0].context.clone()).unwrap();

        // 步骤 2 应依赖步骤 1
        let step2 = plan.steps.iter().find(|s| s.order == 2).unwrap();
        assert!(!step2.depends_on.is_empty());

        // 步骤 1 无依赖
        let step1 = plan.steps.iter().find(|s| s.order == 1).unwrap();
        assert!(step1.depends_on.is_empty());
    }

    /// 测试 — 编程类目标的计划结构
    /// 验证：包含"编程"关键词的目标生成 2 步计划（Planner + Executor）
    #[tokio::test]
    async fn test_planner_code_structure() {
        let mut planner = PlannerAgent::new();

        let msg = AgentMessage::new("User", "Planner", "用 Python 编程实现快速排序");
        let replies = planner.handle_message(msg).await.unwrap();

        // 1 回复 + 2 分派 = 3 条
        assert_eq!(replies.len(), 3);

        // 分派消息的目标应为 Planner 和 Executor
        let targets: Vec<&str> = replies.iter().skip(1).map(|m| m.to.as_str()).collect();
        assert!(targets.contains(&"Planner"));
        assert!(targets.contains(&"Executor"));
    }

    /// 测试 — 带项目上下文的软件任务生成项目感知计划
    #[tokio::test]
    async fn test_planner_project_aware_software_plan() {
        let mut planner = PlannerAgent::new();
        let context = serde_json::json!({
            "projectPlanningContext": {
                "snapshot": {
                    "techStack": ["Rust", "Tauri v2", "React"],
                    "recommendedCommands": [
                        { "command": "cargo test" },
                        { "command": "npm test -- --run" }
                    ]
                },
                "searchQuery": "Task",
                "searchMatches": [
                    { "path": "src-web/src/components/TaskBoard.tsx" },
                    { "path": "src-tauri/src/orchestrator/mod.rs" }
                ]
            }
        });

        let msg = AgentMessage::new("User", "Planner", "优化项目任务看板")
            .with_task_id("task-project")
            .with_context(context);
        let replies = planner.handle_message(msg).await.unwrap();

        assert_eq!(replies.len(), 5);
        let plan: TaskPlan = serde_json::from_value(replies[0].context.clone()).unwrap();
        assert_eq!(plan.steps.len(), 4);
        assert!(plan.steps[0].instruction.contains("Rust"));
        assert!(plan.steps[1].instruction.contains("TaskBoard.tsx"));
        assert!(plan.steps[2].instruction.contains("cargo test"));
    }

    /// 测试 — 普通搜索任务不会因项目上下文被误判为软件任务
    #[tokio::test]
    async fn test_planner_project_context_keeps_non_software_search_plan() {
        let mut planner = PlannerAgent::new();
        let context = serde_json::json!({
            "projectPlanningContext": {
                "snapshot": { "techStack": ["Rust"] },
                "searchQuery": "Agent",
                "searchMatches": []
            }
        });

        let msg = AgentMessage::new("User", "Planner", "帮我搜索今日新闻")
            .with_task_id("task-news")
            .with_context(context);
        let replies = planner.handle_message(msg).await.unwrap();

        assert_eq!(replies.len(), 4);
        let plan: TaskPlan = serde_json::from_value(replies[0].context.clone()).unwrap();
        assert_eq!(plan.steps.len(), 3);
        assert_eq!(plan.steps[0].agent, "Tool");
    }

    /// 测试 — PlanStep 序列化反序列化
    /// 验证：PlanStep 可以正确序列化为 JSON 再反序列化
    #[test]
    fn test_plan_step_serialization() {
        let step = PlanStep {
            step_id: "s1".to_string(),
            order: 1,
            agent: "Tool".to_string(),
            instruction: "搜索".to_string(),
            depends_on: vec![],
            status: StepStatus::Pending,
        };

        let json = serde_json::to_string(&step).unwrap();
        let restored: PlanStep = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.step_id, "s1");
        assert_eq!(restored.agent, "Tool");
        assert_eq!(restored.status, StepStatus::Pending);
    }

    /// 测试 — 默认构造器
    /// 验证：PlannerAgent::default() 创建正确的 Agent
    #[tokio::test]
    async fn test_planner_default() {
        let planner = PlannerAgent::default();
        assert_eq!(planner.name(), "Planner");
        assert_eq!(planner.plans_created, 0);
    }
}
