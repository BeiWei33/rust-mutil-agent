//! Agent 模块
//!
//! 定义 Agent 抽象接口和内置 Agent 实现。
//!
//! # 架构
//! - `traits`：Agent 核心 trait（Agent trait、AgentMessage、Capability）
//! - `coder_agent`：编码 Agent，生成受控补丁草案
//! - `echo_agent`：回显 Agent，用于测试和演示
//! - `planner_agent`：任务规划 Agent，将用户目标分解为执行步骤
//! - `executor_agent`：执行 Agent，执行具体的工具调用和命令
//! - `review_agent`：审查 Agent，输出结构化 ReviewReport
//! - `tester_agent`：测试 Agent，整理验证计划和失败信号
//! - `evolution_agent`：演进 Agent，输出结构化 EvolutionNote
//! - `memory_agent`：记忆 Agent，管理对话历史与知识检索
//! - `tool_agent`：工具 Agent，注册和调用外部工具/函数
//!
//! # 使用示例
//! ```rust
//! use rust_mutil_agent::agent::{Agent, AgentMessage, EchoAgent};
//!
//! # fn main() {
//! # let rt = tokio::runtime::Runtime::new().unwrap();
//! # rt.block_on(async {
//! let mut agent = EchoAgent::new();
//! let msg = AgentMessage::new("user", "echo", "你好");
//! let replies = agent.handle_message(msg).await.unwrap();
//! # });
//! # }
//! ```

pub mod action;
pub mod coder_agent;
pub mod echo_agent;
pub mod evolution_agent;
pub mod executor_agent;
pub mod memory_agent;
pub mod planner_agent;
pub mod review_agent;
pub mod tester_agent;
pub mod tool_agent;
pub mod traits;

// 重新导出常用类型，方便外部使用
pub use action::{AgentAction, AgentOutcome, RiskLevel};
pub use coder_agent::CoderAgent;
pub use echo_agent::EchoAgent;
pub use evolution_agent::EvolutionAgent;
pub use executor_agent::ExecutorAgent;
pub use memory_agent::MemoryAgent;
pub use planner_agent::PlannerAgent;
pub use review_agent::ReviewAgent;
pub use tester_agent::TesterAgent;
pub use tool_agent::ToolAgent;
pub use traits::{Agent, AgentMessage, Capability, CapabilityLevel};
