//! 消息总线模块 — Agent 间通信基础设施
//!
//! 基于 Tokio broadcast 通道实现发布/订阅模式。

pub mod message_bus;

pub use message_bus::MessageBus;
