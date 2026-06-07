//! 回显 Agent（EchoAgent）
//!
//! 最简单的 Agent 实现，用于测试消息通信系统。
//! 收到任何消息后，原样回显给发送方。
//!
//! # 用途
//! - 验证 MessageBus 通信是否正常
//! - 演示 Agent 接口实现方式
//! - 集成测试中的桩 Agent

use async_trait::async_trait;

use super::traits::{Agent, AgentMessage, Capability};
use crate::error::AgentError;

/// 回显 Agent
///
/// 将收到的消息内容加前缀 "Echo:" 后返回给发送方。
pub struct EchoAgent {
    /// 已处理消息计数（用于统计和调试）
    count: u64,
}

impl EchoAgent {
    /// 创建新的 EchoAgent
    pub fn new() -> Self {
        Self { count: 0 }
    }
}

impl Default for EchoAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for EchoAgent {
    fn name(&self) -> &str {
        "Echo"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![Capability::chat()]
    }

    async fn handle_message(
        &mut self,
        msg: AgentMessage,
    ) -> Result<Vec<AgentMessage>, AgentError> {
        self.count += 1;

        // 构建回显消息
        let reply = AgentMessage::new(
            self.name(),
            &msg.from,
            &format!("[Echo #{count}] {content}", count = self.count, content = msg.content),
        )
        .with_type("echo_reply");

        tracing::debug!(
            "EchoAgent 处理第 {} 条消息，来自: {}",
            self.count,
            msg.from
        );

        Ok(vec![reply])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_echo_agent_basic() {
        let mut agent = EchoAgent::new();
        assert_eq!(agent.name(), "Echo");
        assert_eq!(agent.capabilities().len(), 1);

        let msg = AgentMessage::new("User", "Echo", "你好，世界");
        let replies = agent.handle_message(msg).await.unwrap();

        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].from, "Echo");
        assert_eq!(replies[0].to, "User");
        assert!(replies[0].content.contains("你好，世界"));
        assert!(replies[0].content.contains("Echo #1"));
    }

    #[tokio::test]
    async fn test_echo_agent_count_tracking() {
        let mut agent = EchoAgent::new();

        let msg1 = AgentMessage::new("A", "Echo", "一");
        let msg2 = AgentMessage::new("B", "Echo", "二");

        let replies1 = agent.handle_message(msg1).await.unwrap();
        assert!(replies1[0].content.contains("#1"));

        let replies2 = agent.handle_message(msg2).await.unwrap();
        assert!(replies2[0].content.contains("#2"));
    }

    /// 测试 — 默认构造器
    /// 验证：Default trait 实现正确，初始计数为 0
    #[tokio::test]
    async fn test_echo_agent_default() {
        let mut agent = EchoAgent::default();
        assert_eq!(agent.name(), "Echo");

        // 验证默认构造的 Agent 可以正常处理消息
        let msg = AgentMessage::new("User", "Echo", "测试默认");
        let replies = agent.handle_message(msg).await.unwrap();
        assert!(replies[0].content.contains("#1"));
    }

    /// 测试 — 空消息回显
    /// 验证：即使收到空内容的消息也能正常处理
    #[tokio::test]
    async fn test_echo_agent_empty_message() {
        let mut agent = EchoAgent::new();

        let msg = AgentMessage::new("User", "Echo", "");
        let replies = agent.handle_message(msg).await.unwrap();

        // 空消息也应该能正常回显
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].msg_type, "echo_reply");
        assert!(replies[0].content.contains("Echo #1"));
    }

    /// 测试 — 回显消息保留消息类型标记
    /// 验证：回显时元数据（from、to、msg_type）正确设置
    #[tokio::test]
    async fn test_echo_agent_reply_metadata() {
        let mut agent = EchoAgent::new();

        let msg = AgentMessage::new("Alice", "Echo", "你好")
            .with_task_id("task-123");

        let replies = agent.handle_message(msg).await.unwrap();
        let reply = &replies[0];

        // 验证回复的元数据
        assert_eq!(reply.from, "Echo");
        assert_eq!(reply.to, "Alice");
        assert_eq!(reply.msg_type, "echo_reply");
        // 回显时保留唯一消息 ID（和原消息不同）
        assert_ne!(reply.id, "");
    }

    /// 测试 — 多次回显计数递增
    /// 验证：连续处理 5 条消息后计数正确递增
    #[tokio::test]
    async fn test_echo_agent_consecutive_messages() {
        let mut agent = EchoAgent::new();

        for i in 1..=5 {
            let msg = AgentMessage::new("User", "Echo", &format!("消息{i}"));
            let replies = agent.handle_message(msg).await.unwrap();
            assert!(replies[0].content.contains(&format!("#{i}")));
        }
    }

    /// 测试 — 能力列表内容验证
    /// 验证：EchoAgent 仅声明聊天能力，且能力名称为 "chat"
    #[tokio::test]
    async fn test_echo_agent_capability_details() {
        let agent = EchoAgent::new();
        let caps = agent.capabilities();

        assert_eq!(caps.len(), 1);
        assert_eq!(caps[0].name, "chat");
        assert_eq!(caps[0].level, crate::agent::CapabilityLevel::Basic);
        assert!(caps[0].description.contains("对话"));
    }
}
