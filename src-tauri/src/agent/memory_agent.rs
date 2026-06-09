//! 记忆 Agent（MemoryAgent）
//!
//! 管理短期对话记忆与长期知识存储。
//!
//! # 功能
//! - **短期记忆**：维护对话轮次，提供上下文给其他 Agent
//! - **长期记忆**：将信息存入 SQLite 数据库，支持结构化查询
//! - **向量检索**：通过向量嵌入实现语义搜索（待 sqlite-vec 稳定后启用）
//!
//! # 数据库表设计
//! ```sql
//! CREATE TABLE conversations (
//!     id TEXT PRIMARY KEY,
//!     session_id TEXT NOT NULL,
//!     role TEXT NOT NULL,        -- "user" / "agent"
//!     content TEXT NOT NULL,
//!     agent_name TEXT,
//!     created_at DATETIME DEFAULT CURRENT_TIMESTAMP
//! );
//!
//! CREATE TABLE knowledge (
//!     id TEXT PRIMARY KEY,
//!     title TEXT NOT NULL,
//!     content TEXT NOT NULL,
//!     source TEXT,
//!     tags TEXT,                 -- JSON 数组
//!     created_at DATETIME DEFAULT CURRENT_TIMESTAMP
//! );
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Mutex;

use super::traits::{Agent, AgentMessage, Capability};
use crate::error::AgentError;

/// 对话轮次记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurn {
    /// 轮次 ID
    pub id: String,

    /// 会话 ID
    pub session_id: String,

    /// 角色（user / agent）
    pub role: String,

    /// 消息内容
    pub content: String,

    /// 发言人 Agent 名称
    pub agent_name: Option<String>,

    /// 创建时间
    pub created_at: String,
}

/// 短期记忆缓冲区（最大对话轮次数）
const SHORT_TERM_MEMORY_SIZE: usize = 50;

/// 记忆 Agent
pub struct MemoryAgent {
    /// 短期记忆缓冲区（对话轮次环形缓冲）
    short_term: Vec<ConversationTurn>,

    /// 长期记忆 — SQLite 数据库连接
    /// 使用 Mutex 包装，因为 rusqlite::Connection 不是 Send + Sync
    /// 实际项目中应使用连接池（如 r2d2）
    db: Option<Mutex<rusqlite::Connection>>,

    /// 当前会话 ID
    session_id: String,

    /// 已处理消息计数
    count: u64,
}

impl MemoryAgent {
    /// 创建新的 MemoryAgent
    pub fn new() -> Self {
        Self {
            short_term: Vec::with_capacity(SHORT_TERM_MEMORY_SIZE),
            db: None,
            session_id: uuid::Uuid::new_v4().to_string(),
            count: 0,
        }
    }

    /// 使用 SQLite 数据库初始化（生产环境使用）
    ///
    /// 传入 `:memory:` 使用内存数据库（测试用），
    /// 传入文件路径使用持久化数据库。
    pub fn with_database(db_path: &str) -> Result<Self, AgentError> {
        let conn = rusqlite::Connection::open(db_path)?;

        // 创建数据库表
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                agent_name TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS knowledge (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                source TEXT,
                tags TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS idx_conv_session
                ON conversations(session_id);
            CREATE INDEX IF NOT EXISTS idx_knowledge_title
                ON knowledge(title);",
        )?;

        Ok(Self {
            short_term: Vec::with_capacity(SHORT_TERM_MEMORY_SIZE),
            db: Some(Mutex::new(conn)),
            session_id: uuid::Uuid::new_v4().to_string(),
            count: 0,
        })
    }

    /// 存储对话轮次到短期记忆
    fn store_in_short_term(&mut self, turn: ConversationTurn) {
        if self.short_term.len() >= SHORT_TERM_MEMORY_SIZE {
            // 移除最早的轮次（环形缓冲）
            self.short_term.remove(0);
        }
        self.short_term.push(turn);
    }

    /// 获取最近的对话轮次（供其他 Agent 获取上下文）
    pub fn recent_turns(&self, limit: usize) -> Vec<&ConversationTurn> {
        self.short_term
            .iter()
            .rev()
            .take(limit)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    /// 存储对话轮次到长期记忆（SQLite）
    fn store_in_long_term(
        db: &Mutex<rusqlite::Connection>,
        turn: &ConversationTurn,
    ) -> Result<(), AgentError> {
        let conn = db
            .lock()
            .map_err(|e| AgentError::Internal(format!("数据库锁获取失败: {e}")))?;

        conn.execute(
            "INSERT OR REPLACE INTO conversations (id, session_id, role, content, agent_name)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                turn.id,
                turn.session_id,
                turn.role,
                turn.content,
                turn.agent_name,
            ],
        )?;

        Ok(())
    }

    fn search_long_term(
        db: &Mutex<rusqlite::Connection>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<String>, AgentError> {
        let conn = db
            .lock()
            .map_err(|e| AgentError::Internal(format!("数据库锁获取失败: {e}")))?;
        let like_pattern = format!("%{}%", query);
        let mut stmt = conn.prepare(
            "SELECT role, content
             FROM conversations
             WHERE content LIKE ?1
             ORDER BY created_at DESC, rowid DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![like_pattern, limit as i64], |row| {
            let role: String = row.get(0)?;
            let content: String = row.get(1)?;
            Ok(format!("[{}] {}", role, content))
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }
}

impl Default for MemoryAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for MemoryAgent {
    fn name(&self) -> &str {
        "Memory"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![Capability::memory(), Capability::retrieval()]
    }

    async fn handle_message(&mut self, msg: AgentMessage) -> Result<Vec<AgentMessage>, AgentError> {
        self.count += 1;

        // 根据消息类型执行不同操作
        match msg.msg_type.as_str() {
            "store" | "remember" => {
                // 存储模式：将消息内容存入记忆
                self.handle_store(msg).await
            }
            "query" | "recall" | "retrieve" => {
                // 检索模式：根据内容检索记忆
                self.handle_retrieve(msg).await
            }
            _ => {
                // 默认：自动存储并回复
                self.handle_auto_store(msg).await
            }
        }
    }
}

impl MemoryAgent {
    /// 处理存储请求
    async fn handle_store(&mut self, msg: AgentMessage) -> Result<Vec<AgentMessage>, AgentError> {
        let turn = ConversationTurn {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: self.session_id.clone(),
            role: msg.from.clone(),
            content: msg.content.clone(),
            agent_name: Some(msg.from.clone()),
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        // 存储到长期记忆
        if let Some(ref db) = self.db {
            Self::store_in_long_term(db, &turn)?;
        }

        // 存储到短期记忆
        self.store_in_short_term(turn);

        let reply = msg.reply_to("信息已存入记忆库");
        Ok(vec![reply.with_type("memory_stored")])
    }

    /// 处理检索请求
    async fn handle_retrieve(
        &mut self,
        msg: AgentMessage,
    ) -> Result<Vec<AgentMessage>, AgentError> {
        let limit = msg.context["limit"].as_u64().unwrap_or(5) as usize;
        let limit = limit.clamp(1, 50);

        // 从短期记忆中检索相关轮次（简单关键词匹配）
        let query_lower = msg.content.to_lowercase();
        let mut relevant: Vec<String> = self
            .short_term
            .iter()
            .filter(|turn| turn.content.to_lowercase().contains(&query_lower))
            .rev()
            .take(limit)
            .map(|turn| format!("[{}] {}", turn.role, turn.content))
            .collect();
        let short_term_count = relevant.len();

        let mut long_term_count = 0usize;
        if relevant.len() < limit {
            if let Some(ref db) = self.db {
                let mut seen = relevant.iter().cloned().collect::<HashSet<_>>();
                let remaining = limit - relevant.len();
                for item in Self::search_long_term(db, &msg.content, remaining)? {
                    if seen.insert(item.clone()) {
                        relevant.push(item);
                        long_term_count += 1;
                    }
                }
            }
        }

        let result = if relevant.is_empty() {
            "未找到相关记忆".to_string()
        } else {
            format!(
                "找到 {} 条相关记忆:\n{}",
                relevant.len(),
                relevant.join("\n")
            )
        };

        let context = serde_json::json!({
            "count": relevant.len(),
            "shortTermCount": short_term_count,
            "longTermCount": long_term_count,
            "results": relevant,
        });

        let reply = msg
            .reply_to(&result)
            .with_type("memory_retrieved")
            .with_context(context);

        Ok(vec![reply])
    }

    /// 自动存储模式：记录对话并将上下文返回
    async fn handle_auto_store(
        &mut self,
        msg: AgentMessage,
    ) -> Result<Vec<AgentMessage>, AgentError> {
        // 自动记录本轮对话
        let turn = ConversationTurn {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: self.session_id.clone(),
            role: "user".to_string(),
            content: msg.content.clone(),
            agent_name: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        if let Some(ref db) = self.db {
            Self::store_in_long_term(db, &turn)?;
        }
        self.store_in_short_term(turn);

        // 返回最近的对话上下文
        let recent = self.recent_turns(5);
        let context_summary: Vec<String> = recent
            .iter()
            .map(|t| format!("[{}] {}", t.role, t.content))
            .collect();

        let context = serde_json::json!({
            "session_id": self.session_id,
            "recent_conversations": context_summary,
        });

        let reply = msg
            .reply_to("对话已记录")
            .with_type("memory_ack")
            .with_context(context);

        Ok(vec![reply])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_store_and_retrieve() {
        let mut agent = MemoryAgent::new();

        // 存储一条消息
        let store_msg = AgentMessage::new("User", "Memory", "今天天气很好").with_type("store");
        agent.handle_message(store_msg).await.unwrap();

        // 检索消息
        let query_msg = AgentMessage::new("User", "Memory", "天气").with_type("query");
        let replies = agent.handle_message(query_msg).await.unwrap();

        assert_eq!(replies[0].msg_type, "memory_retrieved");
        assert!(replies[0].content.contains("天气"));
    }

    #[tokio::test]
    async fn test_memory_with_database() {
        let agent = MemoryAgent::with_database(":memory:").unwrap();
        assert!(agent.db.is_some());
        assert_eq!(agent.name(), "Memory");
    }

    #[tokio::test]
    async fn test_short_term_limit() {
        let mut agent = MemoryAgent::new();

        // 存储超过缓冲区的消息
        for i in 0..60 {
            let msg = AgentMessage::new("User", "Memory", &format!("消息 {i}")).with_type("store");
            agent.handle_message(msg).await.unwrap();
        }

        // 缓冲区不应超过上限
        assert!(agent.short_term.len() <= SHORT_TERM_MEMORY_SIZE);
    }

    /// 测试 — 自动存储模式
    /// 验证：默认消息类型会自动存储到短期记忆并返回确认
    #[tokio::test]
    async fn test_auto_store_mode() {
        let mut agent = MemoryAgent::new();

        let msg = AgentMessage::new("User", "Memory", "这是自动存储的消息");
        let replies = agent.handle_message(msg).await.unwrap();

        // 自动存储应返回 acknowledgment
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].msg_type, "memory_ack");
        assert!(replies[0].content.contains("已记录"));
        // 短期记忆应有内容
        assert!(!agent.short_term.is_empty());
    }

    /// 测试 — 检索无匹配结果
    /// 验证：关键词不匹配时返回"未找到相关记忆"
    #[tokio::test]
    async fn test_retrieve_no_match() {
        let mut agent = MemoryAgent::new();

        // 先存储一些消息
        let store = AgentMessage::new("User", "Memory", "今天天气真好").with_type("store");
        agent.handle_message(store).await.unwrap();

        // 用不相关的关键词检索
        let query = AgentMessage::new("User", "Memory", "量子计算").with_type("query");
        let replies = agent.handle_message(query).await.unwrap();

        assert_eq!(replies[0].msg_type, "memory_retrieved");
        assert!(replies[0].content.contains("未找到"));
    }

    /// 测试 — Manual recall 消息类型
    /// 验证："recall" 类型的消息可以检索之前存储的内容
    #[tokio::test]
    async fn test_recall_message_type() {
        let mut agent = MemoryAgent::new();

        // 存储
        agent
            .handle_message(AgentMessage::new("User", "Memory", "Rust 编程").with_type("store"))
            .await
            .unwrap();

        // 用 recall 类型检索
        let replies = agent
            .handle_message(AgentMessage::new("User", "Memory", "Rust").with_type("recall"))
            .await
            .unwrap();

        assert_eq!(replies[0].msg_type, "memory_retrieved");
        assert!(replies[0].content.contains("Rust"));
    }

    /// 测试 — 默认构造器
    /// 验证：MemoryAgent::default() 创建正确的 Agent
    #[tokio::test]
    async fn test_memory_agent_default() {
        let agent = MemoryAgent::new();
        assert_eq!(agent.name(), "Memory");
        // 默认不连接数据库
        assert!(agent.db.is_none());
    }

    /// 测试 — 数据库版本的长短期记忆同时使用
    /// 验证：带数据库的 MemoryAgent 可以同时使用短期和长期记忆
    #[tokio::test]
    async fn test_database_store_and_short_term_sync() {
        let mut agent = MemoryAgent::with_database(":memory:").unwrap();

        // 存储消息（自动存储模式）
        let msg = AgentMessage::new("Planner", "Memory", "关键信息：API Key = abc123")
            .with_type("remember");
        let replies = agent.handle_message(msg).await.unwrap();

        assert_eq!(replies[0].msg_type, "memory_stored");
        // 短期记忆应有内容
        assert!(!agent.short_term.is_empty());
    }

    /// 测试 — 自动存储会写入长期数据库
    /// 验证：清空短期记忆后仍能从 SQLite 检索到自动存储内容
    #[tokio::test]
    async fn test_auto_store_writes_and_retrieves_from_long_term() {
        let mut agent = MemoryAgent::with_database(":memory:").unwrap();

        agent
            .handle_message(AgentMessage::new(
                "User",
                "Memory",
                "长期记忆关键词 AlphaBeta",
            ))
            .await
            .unwrap();
        agent.short_term.clear();

        let replies = agent
            .handle_message(AgentMessage::new("User", "Memory", "AlphaBeta").with_type("query"))
            .await
            .unwrap();

        assert_eq!(replies[0].msg_type, "memory_retrieved");
        assert!(replies[0].content.contains("AlphaBeta"));
        assert_eq!(replies[0].context["shortTermCount"], serde_json::json!(0));
        assert_eq!(replies[0].context["longTermCount"], serde_json::json!(1));
    }

    /// 测试 — 能力列表验证
    /// 验证：MemoryAgent 声明了 memory 和 retrieval 两个能力
    #[test]
    fn test_memory_agent_capabilities() {
        let agent = MemoryAgent::new();
        let caps = agent.capabilities();

        assert_eq!(caps.len(), 2);
        let names: Vec<&str> = caps.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"memory"));
        assert!(names.contains(&"retrieval"));
    }

    /// 测试 — recent_turns 返回正确数量
    /// 验证：recent_turns 方法只返回最近 N 轮
    #[tokio::test]
    async fn test_recent_turns_limit() {
        let mut agent = MemoryAgent::new();

        // 存储 10 条消息
        for i in 0..10 {
            let msg = AgentMessage::new("User", "Memory", &format!("消息{i}")).with_type("store");
            agent.handle_message(msg).await.unwrap();
        }

        // 查询最近 3 轮
        let recent = agent.recent_turns(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].content, "消息7");
        assert_eq!(recent[2].content, "消息9");
    }

    /// 测试 — "retrieve" 消息类型检索
    /// 验证：使用 retrieve 类型可以检索存储的内容
    #[tokio::test]
    async fn test_retrieve_message_type() {
        let mut agent = MemoryAgent::new();

        // 存储两条消息
        agent
            .handle_message(
                AgentMessage::new("User", "Memory", "学习 Rust 编程语言").with_type("store"),
            )
            .await
            .unwrap();

        agent
            .handle_message(
                AgentMessage::new("User", "Memory", "学习 TypeScript 前端开发").with_type("store"),
            )
            .await
            .unwrap();

        // 用 retrieve 类型检索 Rust 相关内容
        let replies = agent
            .handle_message(AgentMessage::new("User", "Memory", "Rust").with_type("retrieve"))
            .await
            .unwrap();

        assert_eq!(replies[0].msg_type, "memory_retrieved");
        let context = &replies[0].context;
        assert!(context["count"].as_u64().unwrap() >= 1);
    }
}
