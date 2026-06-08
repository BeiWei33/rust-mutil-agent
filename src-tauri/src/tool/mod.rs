//! 工具模块 — 可扩展的工具注册与调用系统
//!
//! 重新导出 tool_agent 中定义的工具注册表和相关类型。

pub use crate::agent::tool_agent::{register_builtin_tools, ToolDescription, ToolFn, ToolRegistry};
