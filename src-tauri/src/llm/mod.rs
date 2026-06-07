//! LLM 客户端模块 — 统一的大语言模型接口
//!
//! 封装对多种 LLM 后端（OpenAI、本地模型等）的调用，
//! 提供统一的聊天补全接口。

use crate::error::AgentError;
use serde::{Deserialize, Serialize};

/// DeepSeek 默认 OpenAI-compatible Chat Completions 端点。
pub const DEFAULT_DEEPSEEK_ENDPOINT: &str = "https://api.deepseek.com/v1/chat/completions";
/// 默认 DeepSeek 模型。
pub const DEFAULT_DEEPSEEK_MODEL: &str = "deepseek-v4-pro";

/// LLM 提供商类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LLMProvider {
    /// OpenAI API
    OpenAI,
    /// DeepSeek API（OpenAI-compatible）
    DeepSeek,
    /// 本地 llama.cpp 服务
    LlamaCpp,
    /// 自定义 HTTP 端点
    Custom(String),
    /// 模拟模式（用于测试）
    Mock,
}

/// 聊天消息角色
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
}

/// 单条聊天消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// 消息角色
    pub role: Role,
    /// 消息内容
    pub content: String,
}

/// 聊天补全请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    /// 系统提示词
    pub system_prompt: Option<String>,
    /// 对话消息列表
    pub messages: Vec<ChatMessage>,
    /// 最大生成 token 数
    pub max_tokens: Option<u32>,
    /// 温度参数 (0.0 ~ 2.0)
    pub temperature: Option<f32>,
}

/// 聊天补全响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    /// 生成的回复内容
    pub content: String,
    /// 使用的 token 数
    pub tokens_used: Option<u32>,
    /// 模型名称
    pub model: String,
    /// 是否因为长度限制被截断
    pub truncated: bool,
}

/// LLM 客户端，封装 LLM 调用逻辑
pub struct LLMClient {
    /// LLM 提供商
    provider: LLMProvider,
    /// API 端点 URL
    endpoint: String,
    /// API 密钥
    api_key: Option<String>,
    /// 默认模型名称
    model: String,
}

impl LLMClient {
    /// 创建 LLM 客户端
    pub fn new(
        provider: LLMProvider,
        endpoint: String,
        api_key: Option<String>,
        model: String,
    ) -> Self {
        Self {
            provider,
            endpoint,
            api_key,
            model,
        }
    }

    /// 创建模拟客户端（用于测试）
    pub fn mock() -> Self {
        Self {
            provider: LLMProvider::Mock,
            endpoint: String::new(),
            api_key: None,
            model: "mock-model".to_string(),
        }
    }

    /// 创建 OpenAI 客户端
    pub fn openai(api_key: String, model: Option<String>) -> Self {
        Self {
            provider: LLMProvider::OpenAI,
            endpoint: "https://api.openai.com/v1/chat/completions".to_string(),
            api_key: Some(api_key),
            model: model.unwrap_or_else(|| "gpt-4o".to_string()),
        }
    }

    /// 创建 DeepSeek 客户端。
    pub fn deepseek(api_key: String, model: Option<String>) -> Self {
        Self {
            provider: LLMProvider::DeepSeek,
            endpoint: DEFAULT_DEEPSEEK_ENDPOINT.to_string(),
            api_key: Some(api_key),
            model: model.unwrap_or_else(|| DEFAULT_DEEPSEEK_MODEL.to_string()),
        }
    }

    /// 从用户环境变量创建默认 DeepSeek 客户端。
    ///
    /// 支持的环境变量：
    /// - `DEEPSEEK_API_KEY`：API Key
    /// - `DEEPSEEK_BASE_URL`：可选，默认 `https://api.deepseek.com/v1/chat/completions`
    /// - `DEEPSEEK_MODEL`：可选，默认 `deepseek-v4-pro`
    pub fn deepseek_from_env() -> Self {
        let api_key = std::env::var("DEEPSEEK_API_KEY")
            .ok()
            .filter(|v| !v.trim().is_empty());
        let endpoint = std::env::var("DEEPSEEK_BASE_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_DEEPSEEK_ENDPOINT.to_string());
        let model = std::env::var("DEEPSEEK_MODEL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_DEEPSEEK_MODEL.to_string());

        Self {
            provider: LLMProvider::DeepSeek,
            endpoint,
            api_key,
            model,
        }
    }

    /// 发送聊天补全请求
    ///
    /// Mock 模式返回模拟响应，真实模式将在后续集成 reqwest 后实现。
    pub async fn chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, AgentError> {
        match self.provider {
            LLMProvider::Mock => {
                let last_user_msg = request
                    .messages
                    .last()
                    .map(|m| m.content.as_str())
                    .unwrap_or("");

                Ok(ChatCompletionResponse {
                    content: format!("[Mock LLM 响应] 已收到您的消息: \"{}\"", last_user_msg),
                    tokens_used: Some(42),
                    model: self.model.clone(),
                    truncated: false,
                })
            }
            _ => Err(AgentError::LlmError(format!(
                "LLM 调用尚未实现，当前仅支持 Mock 模式。提供者: {:?}，端点: {}",
                self.provider, self.endpoint
            ))),
        }
    }

    /// 返回当前使用的 LLM 提供商
    pub fn provider(&self) -> &LLMProvider {
        &self.provider
    }

    /// 返回当前使用的模型名称
    pub fn model(&self) -> &str {
        &self.model
    }

    /// 返回当前 API 端点。
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// 返回当前 API Key（仅测试/内部校验使用，不应暴露到前端日志）。
    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_completion() {
        let client = LLMClient::mock();
        let request = ChatCompletionRequest {
            system_prompt: Some("你是一个有帮助的助手".to_string()),
            messages: vec![ChatMessage {
                role: Role::User,
                content: "你好".to_string(),
            }],
            max_tokens: Some(100),
            temperature: Some(0.7),
        };

        let response = client.chat_completion(request).await.unwrap();
        assert!(response.content.contains("你好"));
    }

    #[test]
    fn test_openai_client_creation() {
        let client = LLMClient::openai("sk-test-key".to_string(), None);
        assert_eq!(*client.provider(), LLMProvider::OpenAI);
        assert_eq!(client.model(), "gpt-4o");
    }

    /// 测试 — 使用自定义模型创建 OpenAI 客户端
    /// 验证：指定模型参数后 client.model() 返回正确值
    #[test]
    fn test_openai_client_custom_model() {
        let client = LLMClient::openai(
            "sk-custom".to_string(),
            Some("gpt-4-turbo".to_string()),
        );
        assert_eq!(*client.provider(), LLMProvider::OpenAI);
        assert_eq!(client.model(), "gpt-4-turbo");
    }

    /// 测试 — DeepSeek 客户端默认模型和端点
    /// 验证：DeepSeek 默认使用 deepseek-v4-pro 和官方 OpenAI-compatible 端点
    #[test]
    fn test_deepseek_client_defaults() {
        let client = LLMClient::deepseek("sk-test-key".to_string(), None);
        assert_eq!(*client.provider(), LLMProvider::DeepSeek);
        assert_eq!(client.model(), DEFAULT_DEEPSEEK_MODEL);
        assert_eq!(client.endpoint(), DEFAULT_DEEPSEEK_ENDPOINT);
        assert_eq!(client.api_key(), Some("sk-test-key"));
    }

    /// 测试 — 从环境变量创建 DeepSeek 客户端时无环境变量也可回退默认模型/端点
    #[test]
    fn test_deepseek_from_env_defaults() {
        let client = LLMClient::deepseek_from_env();
        assert_eq!(*client.provider(), LLMProvider::DeepSeek);
        assert!(!client.model().is_empty());
        assert!(!client.endpoint().is_empty());
    }

    /// 测试 — Mock 客户端返回固定 token 数量
    /// 验证：Mock 模式时 tokens_used 为 42 且 truncated 为 false
    #[tokio::test]
    async fn test_mock_completion_token_count() {
        let client = LLMClient::mock();
        let request = ChatCompletionRequest {
            system_prompt: None,
            messages: vec![ChatMessage {
                role: Role::User,
                content: "hello".to_string(),
            }],
            max_tokens: None,
            temperature: None,
        };

        let response = client.chat_completion(request).await.unwrap();
        assert_eq!(response.tokens_used, Some(42));
        assert!(!response.truncated);
        assert_eq!(response.model, "mock-model");
    }

    /// 测试 — Mock 响应包含原始消息内容
    /// 验证：Mock 响应中回显了用户的最后一条消息
    #[tokio::test]
    async fn test_mock_response_echoes_input() {
        let client = LLMClient::mock();
        let request = ChatCompletionRequest {
            system_prompt: Some("你是助手".to_string()),
            messages: vec![
                ChatMessage {
                    role: Role::User,
                    content: "第一轮".to_string(),
                },
                ChatMessage {
                    role: Role::Assistant,
                    content: "回复".to_string(),
                },
                ChatMessage {
                    role: Role::User,
                    content: "第二轮提问".to_string(),
                },
            ],
            max_tokens: None,
            temperature: None,
        };

        let response = client.chat_completion(request).await.unwrap();
        // Mock 响应应包含最后一条用户消息的内容
        assert!(response.content.contains("第二轮提问"));
        assert!(response.content.contains("[Mock LLM 响应]"));
    }

    /// 测试 — 非 Mock 模式返回错误
    /// 验证：OpenAI / LlamaCpp / Custom 等非 Mock 模式会返回 LlmError
    #[tokio::test]
    async fn test_non_mock_provider_returns_error() {
        let client = LLMClient::new(
            LLMProvider::LlamaCpp,
            "http://localhost:8080".to_string(),
            None,
            "llama3".to_string(),
        );

        let request = ChatCompletionRequest {
            system_prompt: None,
            messages: vec![ChatMessage {
                role: Role::User,
                content: "hello".to_string(),
            }],
            max_tokens: None,
            temperature: None,
        };

        let result = client.chat_completion(request).await;
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("尚未实现") || err.contains("Mock"));
    }

    /// 测试 — LLMProvider 的序列化
    /// 验证：LLMProvider 枚举可以正确序列化和反序列化
    #[test]
    fn test_llm_provider_serialization() {
        let provider = LLMProvider::OpenAI;
        let json = serde_json::to_string(&provider).unwrap();
        assert!(json.contains("OpenAI"));

        let restored: LLMProvider = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, LLMProvider::OpenAI);
    }

    /// 测试 — Custom 提供商的序列化
    /// 验证：LLMProvider::Custom 带 URL 参数的序列化
    #[test]
    fn test_custom_provider_serialization() {
        let provider = LLMProvider::Custom("https://my-api.com/llm".to_string());
        let json = serde_json::to_string(&provider).unwrap();
        let restored: LLMProvider = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, LLMProvider::Custom("https://my-api.com/llm".to_string()));
    }

    /// 测试 — ChatMessage 的序列化
    /// 验证：ChatMessage 的 JSON 序列化格式正确
    #[test]
    fn test_chat_message_serialization() {
        let msg = ChatMessage {
            role: Role::User,
            content: "你好".to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        let restored: ChatMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.role, Role::User);
        assert_eq!(restored.content, "你好");
    }

    /// 测试 — ChatCompletionRequest 默认字段
    /// 验证：max_tokens 和 temperature 为 None 时请求正常
    #[tokio::test]
    async fn test_request_with_optional_fields_none() {
        let client = LLMClient::mock();
        let request = ChatCompletionRequest {
            system_prompt: None,
            messages: vec![ChatMessage {
                role: Role::User,
                content: "test".to_string(),
            }],
            max_tokens: None,
            temperature: None,
        };

        let result = client.chat_completion(request).await;
        assert!(result.is_ok());
    }

    /// 测试 — Mock 客户端 provider() 和 model() 访问器
    /// 验证：访问器方法返回正确的值
    #[test]
    fn test_mock_client_accessors() {
        let client = LLMClient::mock();
        assert_eq!(*client.provider(), LLMProvider::Mock);
        assert_eq!(client.model(), "mock-model");
    }

    /// 测试 — Role 枚举的序列化
    /// 验证：Role 的 serde rename 属性生效（system/user/assistant）
    #[test]
    fn test_role_serialization() {
        let system_role = serde_json::to_string(&Role::System).unwrap();
        assert!(system_role.contains("system"));

        let user_role = serde_json::to_string(&Role::User).unwrap();
        assert!(user_role.contains("user"));

        let assistant_role = serde_json::to_string(&Role::Assistant).unwrap();
        assert!(assistant_role.contains("assistant"));
    }
}
