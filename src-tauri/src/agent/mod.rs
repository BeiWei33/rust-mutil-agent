//! Agent 模块
//!
//! 定义 Agent 抽象接口和内置 Agent 实现。
//! 
//! # 架构
//! - `traits`：Agent 核心 trait（Agent trait、AgentMessage、Capability）
//! - `echo_agent`：回显 Agent，用于测试和演示
//! - `planner_agent`：任务规划 Agent，将用户目标分解为执行步骤
//! - `executor_agent`：执行 Agent，执行具体的工具调用和命令
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

pub mod echo_agent;
pub mod executor_agent;
pub mod memory_agent;
pub mod planner_agent;
pub mod tool_agent;
pub mod traits;

// 重新导出常用类型，方便外部使用
pub use traits::{Agent, AgentMessage, Capability, CapabilityLevel};
pub use echo_agent::EchoAgent;
pub use planner_agent::PlannerAgent;
pub use executor_agent::ExecutorAgent;
pub use memory_agent::MemoryAgent;
pub use tool_agent::ToolAgent;
