//! 执行 Agent（ExecutorAgent）
//!
//! 负责执行具体操作：API 调用、本地命令、文件操作等。
//!
//! # 职责
//! - 接收 Planner 或其他 Agent 的执行指令
//! - 调用外部 API（搜索、HTTP 请求等）
//! - 执行本地系统命令（安全沙箱内）
//! - 文件读写操作
//! - 返回执行结果给请求方
//!
//! # 安全说明
//! 生产环境中应限制 ExecutorAgent 的权限范围，
//! 避免执行未授权的系统命令。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::traits::{Agent, AgentMessage, Capability};
use crate::error::AgentError;

/// 执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// 是否成功
    pub success: bool,

    /// 输出内容
    pub output: String,

    /// 执行耗时（毫秒）
    pub duration_ms: u64,

    /// 错误信息（如果有）
    pub error: Option<String>,
}

/// 执行 Agent
pub struct ExecutorAgent {
    /// HTTP 客户端（用于外部 API 调用）
    http_client: reqwest::Client,

    /// 已执行任务计数
    count: u64,
}

impl ExecutorAgent {
    /// 创建新的 ExecutorAgent
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::new(),
            count: 0,
        }
    }
}

impl Default for ExecutorAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for ExecutorAgent {
    fn name(&self) -> &str {
        "Executor"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![Capability::code_execution()]
    }

    async fn handle_message(&mut self, msg: AgentMessage) -> Result<Vec<AgentMessage>, AgentError> {
        self.count += 1;

        tracing::info!(
            "ExecutorAgent 开始执行任务 #{}: {}",
            self.count,
            &msg.content
        );

        let start = std::time::Instant::now();

        // 根据消息类型执行不同操作
        let result = if msg.msg_type == "http_request" {
            // 执行 HTTP 请求
            self.execute_http(&msg.context).await
        } else if msg.content.contains("搜索") || msg.content.contains("search") {
            // 模拟搜索操作（实际应调用搜索引擎 API）
            self.execute_mock_search(&msg.content).await
        } else {
            // 通用执行：返回确认信息
            Ok(ExecutionResult {
                success: true,
                output: format!("已执行: {}", msg.content),
                duration_ms: start.elapsed().as_millis() as u64,
                error: None,
            })
        };

        match result {
            Ok(exec_result) => {
                let context =
                    serde_json::to_value(&exec_result).map_err(|e| AgentError::Serialization(e))?;

                let reply = msg
                    .reply_to(&format!("执行完成: {}", exec_result.output))
                    .with_type("execution_result")
                    .with_context(context);

                Ok(vec![reply])
            }
            Err(e) => {
                tracing::error!("ExecutorAgent 执行失败: {e}");

                let reply = msg
                    .reply_to(&format!("执行失败: {e}"))
                    .with_type("execution_error");

                Ok(vec![reply])
            }
        }
    }
}

impl ExecutorAgent {
    /// 执行 HTTP 请求
    ///
    /// 从消息 context 中解析 URL、方法、请求体等参数。
    async fn execute_http(
        &self,
        context: &serde_json::Value,
    ) -> Result<ExecutionResult, AgentError> {
        let url = context["url"]
            .as_str()
            .ok_or_else(|| AgentError::MessageFormat("缺少 URL 参数".to_string()))?;

        let method = context["method"].as_str().unwrap_or("GET");

        let start = std::time::Instant::now();

        let response = match method {
            "GET" => self.http_client.get(url),
            "POST" => {
                let body = context["body"].clone();
                self.http_client.post(url).json(&body)
            }
            _ => {
                return Err(AgentError::Internal(format!(
                    "不支持的 HTTP 方法: {method}"
                )))
            }
        };

        let resp = response
            .send()
            .await
            .map_err(|e| AgentError::Internal(format!("HTTP 请求失败: {e}")))?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();

        Ok(ExecutionResult {
            success: status.is_success(),
            output: format!("HTTP {status}: {body}"),
            duration_ms: start.elapsed().as_millis() as u64,
            error: if status.is_success() {
                None
            } else {
                Some(format!("HTTP 状态码: {status}"))
            },
        })
    }

    /// 模拟搜索操作（开发阶段占位）
    ///
    /// 未来应替换为真实的搜索引擎 API 调用。
    async fn execute_mock_search(&self, query: &str) -> Result<ExecutionResult, AgentError> {
        let start = std::time::Instant::now();

        // 模拟搜索延迟
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        Ok(ExecutionResult {
            success: true,
            output: format!(
                "[模拟搜索结果] 关于 '{}' 的搜索结果：\n\
                 1. 相关文章 A — 摘要信息\n\
                 2. 相关文章 B — 摘要信息\n\
                 3. 相关文章 C — 摘要信息",
                query
            ),
            duration_ms: start.elapsed().as_millis() as u64,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_executor_simple_task() {
        let mut executor = ExecutorAgent::new();

        let msg = AgentMessage::new("Planner", "Executor", "执行测试任务");
        let replies = executor.handle_message(msg).await.unwrap();

        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].msg_type, "execution_result");
        assert!(replies[0].content.contains("已执行"));
    }

    #[tokio::test]
    async fn test_executor_search() {
        let mut executor = ExecutorAgent::new();

        let msg = AgentMessage::new("Planner", "Executor", "搜索今日新闻");
        let replies = executor.handle_message(msg).await.unwrap();

        assert_eq!(replies[0].msg_type, "execution_result");
        assert!(replies[0].content.contains("搜索结果"));
    }

    /// 测试 — 通用执行返回确认
    /// 验证：非搜索/HTTP 类型的消息返回"已执行"确认
    #[tokio::test]
    async fn test_executor_generic_task() {
        let mut executor = ExecutorAgent::new();

        let msg = AgentMessage::new("Planner", "Executor", "整理文档");
        let replies = executor.handle_message(msg).await.unwrap();

        assert_eq!(replies[0].msg_type, "execution_result");
        assert!(replies[0].content.contains("已执行"));
        // 验证 response context 包含 ExecutionResult
        let ctx = &replies[0].context;
        assert!(ctx.get("success").unwrap().as_bool().unwrap());
        assert!(ctx.get("duration_ms").is_some());
    }

    /// 测试 — 执行结果包含耗时信息
    /// 验证：返回的消息 context 中包含 duration_ms 字段
    #[tokio::test]
    async fn test_executor_duration_tracked() {
        let mut executor = ExecutorAgent::new();

        let msg = AgentMessage::new("Planner", "Executor", "快速任务");
        let replies = executor.handle_message(msg).await.unwrap();

        let ctx = &replies[0].context;
        // duration_ms 应存在且为非负数
        let duration = ctx["duration_ms"].as_u64().unwrap();
        assert!(duration < 1000, "简单任务应在 1 秒内完成");
    }

    /// 测试 — 搜索操作包含模拟延迟
    /// 验证：搜索任务应包含 200ms 模拟延迟
    #[tokio::test]
    async fn test_executor_search_has_delay() {
        let mut executor = ExecutorAgent::new();

        let msg = AgentMessage::new("Planner", "Executor", "搜索 Rust 资料");
        let start = std::time::Instant::now();
        let replies = executor.handle_message(msg).await.unwrap();
        let elapsed = start.elapsed();

        // 搜索有 200ms 模拟延迟
        assert!(elapsed.as_millis() >= 150, "搜索应有模拟延迟");
        assert!(replies[0].content.contains("搜索结果"));
    }

    /// 测试 — 执行计数器
    /// 验证：handle_message 调用后 count 递增
    #[tokio::test]
    async fn test_executor_counter_increment() {
        let mut executor = ExecutorAgent::new();

        // 第一次调用
        executor
            .handle_message(AgentMessage::new("P", "E", "任务1"))
            .await
            .unwrap();
        assert_eq!(executor.count, 1);

        // 第二次调用
        executor
            .handle_message(AgentMessage::new("P", "E", "任务2"))
            .await
            .unwrap();
        assert_eq!(executor.count, 2);

        // 第三次调用
        executor
            .handle_message(AgentMessage::new("P", "E", "任务3"))
            .await
            .unwrap();
        assert_eq!(executor.count, 3);
    }

    /// 测试 — ExecutionResult 序列化
    /// 验证：ExecutionResult 结构体可以正确序列化和反序列化
    #[test]
    fn test_execution_result_serialization() {
        let result = ExecutionResult {
            success: true,
            output: "任务完成".to_string(),
            duration_ms: 150,
            error: None,
        };

        let json = serde_json::to_string(&result).unwrap();
        let restored: ExecutionResult = serde_json::from_str(&json).unwrap();

        assert!(restored.success);
        assert_eq!(restored.output, "任务完成");
        assert_eq!(restored.duration_ms, 150);
        assert!(restored.error.is_none());
    }

    /// 测试 — 默认构造器
    /// 验证：ExecutorAgent::default() 创建正确的对象
    #[test]
    fn test_executor_default() {
        let executor = ExecutorAgent::default();
        assert_eq!(executor.name(), "Executor");
        assert_eq!(executor.count, 0);
    }

    /// 测试 — HTTP 请求消息类型路由
    /// 验证：msg_type 为 "http_request" 时走 HTTP 执行路径
    #[tokio::test]
    async fn test_executor_http_message_type() {
        let mut executor = ExecutorAgent::new();

        // msg_type 为 "http_request" 但 context 中没有 url 应返回错误
        let msg = AgentMessage::new("Planner", "Executor", "HTTP 请求").with_type("http_request");

        let replies = executor.handle_message(msg).await.unwrap();

        // HTTP 请求失败时应返回执行错误
        assert_eq!(replies[0].msg_type, "execution_error");
    }
}
