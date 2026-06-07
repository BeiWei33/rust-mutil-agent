//! 自定义错误类型
//!
//! 定义整个应用使用的错误类型，通过 thiserror derive 宏简化实现，
//! 并自动转换来自外部 crate 的错误。

use thiserror::Error;

/// Agent 系统的主要错误类型
///
/// 涵盖 Agent 通信、工具调用、数据库操作、LLM 请求等所有可能的错误。
#[derive(Debug, Error)]
pub enum AppError {
    /// Agent 相关错误（消息处理、能力匹配等）
    #[error("Agent 错误: {0}")]
    Agent(String),

    /// 消息总线错误
    #[error("消息总线错误: {0}")]
    Bus(String),

    /// 调度器错误
    #[error("调度器错误: {0}")]
    Orchestrator(String),

    /// 工具调用错误
    #[error("工具调用错误: {0}")]
    Tool(String),

    /// 数据库错误
    #[error("数据库错误: {0}")]
    Database(#[from] rusqlite::Error),

    /// LLM API 错误
    #[error("LLM 错误: {0}")]
    Llm(String),

    /// HTTP 请求错误
    #[error("网络请求错误: {0}")]
    Http(#[from] reqwest::Error),

    /// 序列化/反序列化错误
    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),

    /// 配置错误
    #[error("配置错误: {0}")]
    Config(String),

    /// IO 错误
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    /// UUID 解析错误
    #[error("UUID 错误: {0}")]
    Uuid(#[from] uuid::Error),

    /// 通用错误（未知或未分类）
    #[error("未知错误: {0}")]
    Unknown(String),
}

/// Agent 系统结果类型别名
///
/// 简化函数签名，所有 Agent 系统函数统一使用此 Result 类型。
pub type AppResult<T> = Result<T, AppError>;

/// 旧版 Agent 错误类型（保持向后兼容）
/// 
/// 使用方式：AgentError::BusError("...".into())
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Agent 内部错误: {0}")]
    Internal(String),

    #[error("消息格式错误: {0}")]
    MessageFormat(String),

    #[error("通信超时")]
    Timeout,

    #[error("总线错误: {0}")]
    BusError(String),

    #[error("工具调用错误: {0}")]
    Tool(String),

    #[error("工具未找到: {0}")]
    ToolNotFound(String),

    #[error("数据库错误: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("LLM 调用错误: {0}")]
    LlmError(String),

    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl From<AppError> for AgentError {
    fn from(err: AppError) -> Self {
        match err {
            AppError::Agent(s) => AgentError::Internal(s),
            AppError::Bus(s) => AgentError::BusError(s),
            AppError::Orchestrator(s) => AgentError::Internal(s),
            AppError::Tool(s) => AgentError::ToolNotFound(s),
            AppError::Database(e) => AgentError::Database(e),
            AppError::Llm(s) => AgentError::LlmError(s),
            AppError::Serialization(e) => AgentError::Serialization(e),
            AppError::Http(e) => AgentError::BusError(e.to_string()),
            AppError::Config(s) => AgentError::Internal(s),
            AppError::Io(e) => AgentError::Internal(e.to_string()),
            AppError::Uuid(e) => AgentError::Internal(e.to_string()),
            AppError::Unknown(s) => AgentError::Internal(s),
        }
    }
}
