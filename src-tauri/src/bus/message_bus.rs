//! 消息总线实现 — 基于 Tokio broadcast 通道的发布/订阅系统
//!
//! MessageBus 是 Agent 间通信的核心基础设施，负责：
//! 1. 消息发布（publish）：将消息广播给所有订阅方
//! 2. 消息订阅（subscribe）：订阅方注册接收广播消息
//! 3. 发送端获取（sender）：获取广播发送端的克隆引用

use crate::agent::traits::AgentMessage;
use crate::error::AgentError;
use tokio::sync::broadcast;

/// 消息总线缓冲区大小（最大暂存消息数）
const BUS_CAPACITY: usize = 1024;

/// 消息总线
///
/// 基于 Tokio broadcast 通道实现，支持多发布者、多订阅者模式。
/// 所有已发送的消息都会广播给所有在线订阅者。
///
/// # 使用示例
/// ```ignore
/// let bus = MessageBus::new();
/// let mut rx = bus.subscribe();
/// bus.publish(msg).unwrap();
/// let received = rx.recv().await.unwrap();
/// ```
#[derive(Debug)]
pub struct MessageBus {
    /// 广播发送端
    tx: broadcast::Sender<AgentMessage>,
}

impl MessageBus {
    /// 创建新的消息总线
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(BUS_CAPACITY);
        Self { tx }
    }

    /// 发布消息到总线
    ///
    /// 将消息广播给所有当前的订阅者，并返回收到消息的订阅者数量。
    ///
    /// # 参数
    /// - `msg`：要发布的 AgentMessage
    ///
    /// # 错误
    /// 当没有订阅者时返回错误（避免消息丢失）。
    pub fn publish(&self, msg: AgentMessage) -> Result<usize, AgentError> {
        let count = self.tx.receiver_count();
        if count == 0 {
            return Err(AgentError::BusError(
                "消息总线没有活跃的订阅者".to_string(),
            ));
        }
        self.tx
            .send(msg)
            .map_err(|e| AgentError::BusError(format!("广播消息发送失败: {e}")))?;
        // 发送成功后返回订阅者数量
        Ok(count)
    }

    /// 订阅消息总线
    ///
    /// 返回一个广播接收端，可以接收所有发布的消息。
    /// 注意：订阅者只能接收到订阅之后发布的消息。
    pub fn subscribe(&self) -> broadcast::Receiver<AgentMessage> {
        self.tx.subscribe()
    }

    /// 获取广播发送端的克隆引用
    ///
    /// 用于 Agent 在处理消息后将回复发送回总线。
    pub fn sender(&self) -> broadcast::Sender<AgentMessage> {
        self.tx.clone()
    }

    /// 获取当前活跃订阅者数量
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 — 基本发布/订阅通信
    /// 验证：单个订阅者可以正常接收发布的消息
    #[tokio::test]
    async fn test_publish_and_subscribe_basic() {
        // 创建消息总线
        let bus = MessageBus::new();

        // 订阅总线（必须在发布之前订阅）
        let mut rx = bus.subscribe();

        // 发布一条消息
        let msg = AgentMessage::new("TestAgent", "all", "你好，世界！");
        let subscriber_count = bus.publish(msg).unwrap();

        // 验证至少有一个订阅者
        assert!(subscriber_count > 0, "至少应有一个订阅者");

        // 接收消息并验证内容
        let received = rx.recv().await.unwrap();
        assert_eq!(received.to, "all");
        assert_eq!(received.content, "你好，世界！");
        assert_eq!(received.from, "TestAgent");
    }

    /// 测试 — 多个订阅者同时接收消息
    /// 验证：发布一条消息后，所有订阅者都能收到
    #[tokio::test]
    async fn test_multiple_subscribers() {
        // 创建消息总线
        let bus = MessageBus::new();

        // 创建 3 个订阅者
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();
        let mut rx3 = bus.subscribe();

        // 验证订阅者数量
        assert_eq!(bus.subscriber_count(), 3);

        // 发布一条消息
        let msg = AgentMessage::new("Sender", "all", "广播测试");
        let count = bus.publish(msg).unwrap();
        assert_eq!(count, 3);

        // 所有订阅者都应收到了该消息
        let r1 = rx1.recv().await.unwrap();
        let r2 = rx2.recv().await.unwrap();
        let r3 = rx3.recv().await.unwrap();

        // 验证所有消息内容一致
        assert_eq!(r1.content, "广播测试");
        assert_eq!(r2.content, "广播测试");
        assert_eq!(r3.content, "广播测试");
    }

    /// 测试 — 没有订阅者时发布消息应返回错误
    /// 验证：防止没有任何监听者的消息丢失
    #[tokio::test]
    async fn test_publish_without_subscribers_returns_error() {
        // 创建消息总线，不添加任何订阅者
        let bus = MessageBus::new();

        // 发布消息应失败
        let msg = AgentMessage::new("Test", "all", "无人接收");
        let result = bus.publish(msg);

        // 验证返回错误
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("没有活跃的订阅者"));
    }

    /// 测试 — sender() 返回的发送端可以发布消息
    /// 验证：通过 sender() 获取的发送端也能发布消息
    #[tokio::test]
    async fn test_sender_can_publish() {
        let bus = MessageBus::new();
        let mut rx = bus.subscribe();

        // 通过 sender() 获取发送端并发布
        let tx = bus.sender();
        let msg = AgentMessage::new("Worker", "all", "通过 sender 发布");
        tx.send(msg).unwrap();

        // 验证消息被收到
        let received = rx.recv().await.unwrap();
        assert_eq!(received.content, "通过 sender 发布");
    }

    /// 测试 — 消息带有上下文和类型标记
    /// 验证：复杂消息（带 context、msg_type、task_id）可以正常传输
    #[tokio::test]
    async fn test_message_with_context_and_type() {
        let bus = MessageBus::new();
        let mut rx = bus.subscribe();

        let msg = AgentMessage::new("Orchestrator", "Planner", "分析需求")
            .with_type("plan_request")
            .with_task_id("task-42")
            .with_context(serde_json::json!({ "priority": "high" }));

        bus.publish(msg).unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received.msg_type, "plan_request");
        assert_eq!(received.task_id, Some("task-42".to_string()));
        assert_eq!(received.context["priority"], "high");
    }

    /// 测试 — 多次连续发布
    /// 验证：多次发布的消息可以按序接收
    #[tokio::test]
    async fn test_sequential_publish() {
        let bus = MessageBus::new();
        let mut rx = bus.subscribe();

        for i in 0..5 {
            let msg = AgentMessage::new("Seq", "all", &format!("消息 {i}"));
            bus.publish(msg).unwrap();

            let received = rx.recv().await.unwrap();
            assert_eq!(received.content, format!("消息 {i}"));
        }
    }

    /// 测试 — 默认构造
    /// 验证：MessageBus::default() 等同于 MessageBus::new()
    #[test]
    fn test_default_constructor() {
        let bus = MessageBus::default();
        assert_eq!(bus.subscriber_count(), 0);
    }
}
