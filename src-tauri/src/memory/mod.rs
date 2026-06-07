//! 记忆/知识库模块 — 持久化存储与向量检索

use crate::error::AgentError;
use std::sync::Mutex;

/// 知识库管理器，封装 SQLite 操作
pub struct KnowledgeBase {
    db_path: String,
    conn: Mutex<Option<rusqlite::Connection>>,
}

impl KnowledgeBase {
    /// 创建或打开知识库
    pub fn new(db_path: impl Into<String>) -> Self {
        Self {
            db_path: db_path.into(),
            conn: Mutex::new(None),
        }
    }

    /// 获取或创建数据库连接
    fn get_conn(&self) -> Result<std::sync::MutexGuard<Option<rusqlite::Connection>>, AgentError> {
        let mut guard = self.conn.lock().map_err(|e| AgentError::Internal(e.to_string()))?;
        if guard.is_none() {
            *guard = Some(rusqlite::Connection::open(&self.db_path)?);
        }
        Ok(guard)
    }

    /// 初始化数据库表结构
    pub fn initialize(&self) -> Result<(), AgentError> {
        let mut guard = self.get_conn()?;
        let conn = guard.as_ref().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS knowledge (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                source TEXT,
                tags TEXT DEFAULT '[]',
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_knowledge_title ON knowledge(title);
            CREATE INDEX IF NOT EXISTS idx_knowledge_tags ON knowledge(tags);
            ",
        )?;
        tracing::info!("[KnowledgeBase] 数据库表初始化完成: {}", self.db_path);
        Ok(())
    }

    /// 存储知识条目
    pub fn store_knowledge(
        &self,
        title: &str,
        content: &str,
        source: Option<&str>,
        tags: Option<&[String]>,
    ) -> Result<String, AgentError> {
        let mut guard = self.get_conn()?;
        let conn = guard.as_ref().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let tags_json = serde_json::to_string(&tags.unwrap_or(&[])).unwrap_or_default();
        conn.execute(
            "INSERT INTO knowledge (id, title, content, source, tags) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, title, content, source, tags_json],
        )?;
        Ok(id)
    }

    /// 按关键词搜索
    pub fn search_knowledge(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<serde_json::Value>, AgentError> {
        let mut guard = self.get_conn()?;
        let conn = guard.as_ref().unwrap();
        let like_pattern = format!("%{}%", query);
        let mut stmt = conn.prepare(
            "SELECT id, title, content, source, created_at
             FROM knowledge
             WHERE title LIKE ?1 OR content LIKE ?1
             ORDER BY created_at DESC, rowid DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![like_pattern, limit as i64],
            |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "title": row.get::<_, String>(1)?,
                    "content": row.get::<_, String>(2)?,
                    "source": row.get::<_, Option<String>>(3)?,
                    "created_at": row.get::<_, String>(4)?,
                }))
            },
        )?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn db_path(&self) -> &str {
        &self.db_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knowledge_base_init() {
        let kb = KnowledgeBase::new(":memory:");
        assert!(kb.initialize().is_ok());
    }

    #[test]
    fn test_store_and_search_knowledge() {
        let kb = KnowledgeBase::new(":memory:");
        kb.initialize().unwrap();
        let id = kb
            .store_knowledge("Rust 入门", "Rust 是一门系统编程语言...", Some("教程"), Some(&["rust".to_string(), "编程".to_string()]))
            .unwrap();
        assert!(!id.is_empty());
        let results = kb.search_knowledge("Rust", 5).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_store_multiple_and_search_with_limit() {
        let kb = KnowledgeBase::new(":memory:");
        kb.initialize().unwrap();
        for i in 1..=5 {
            kb.store_knowledge(&format!("条目{i}"), &format!("这是第 {i} 条知识的内容"), None, None).unwrap();
        }
        let results = kb.search_knowledge("知识", 3).unwrap();
        assert_eq!(results.len(), 3);
        let all = kb.search_knowledge("条目", 10).unwrap();
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn test_search_by_title() {
        let kb = KnowledgeBase::new(":memory:");
        kb.initialize().unwrap();
        kb.store_knowledge("Python 入门教程", "Python 是...", None, None).unwrap();
        kb.store_knowledge("Rust 高级编程", "Rust 高级特性", None, None).unwrap();
        let results = kb.search_knowledge("Python", 5).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["title"], "Python 入门教程");
    }

    #[test]
    fn test_search_by_content() {
        let kb = KnowledgeBase::new(":memory:");
        kb.initialize().unwrap();
        kb.store_knowledge("系统设计", "分布式系统架构设计指南", None, None).unwrap();
        kb.store_knowledge("前端开发", "React 组件设计模式", None, None).unwrap();
        let results = kb.search_knowledge("分布式", 5).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["title"], "系统设计");
    }

    #[test]
    fn test_search_no_results() {
        let kb = KnowledgeBase::new(":memory:");
        kb.initialize().unwrap();
        kb.store_knowledge("文章", "内容", None, None).unwrap();
        let results = kb.search_knowledge("绝对不会匹配的内容XYZ", 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_knowledge_source_field() {
        let kb = KnowledgeBase::new(":memory:");
        kb.initialize().unwrap();
        kb.store_knowledge("新闻", "今日新闻摘要", Some("人民日报"), None).unwrap();
        let results = kb.search_knowledge("新闻", 5).unwrap();
        assert_eq!(results[0]["source"], "人民日报");
    }

    #[test]
    fn test_db_path() {
        let kb = KnowledgeBase::new(":memory:");
        assert_eq!(kb.db_path(), ":memory:");
        let kb2 = KnowledgeBase::new("/tmp/test.db");
        assert_eq!(kb2.db_path(), "/tmp/test.db");
    }

    #[test]
    fn test_initialize_is_idempotent() {
        let kb = KnowledgeBase::new(":memory:");
        assert!(kb.initialize().is_ok());
        assert!(kb.initialize().is_ok());
        assert!(kb.initialize().is_ok());
    }

    #[test]
    fn test_store_with_tags() {
        let kb = KnowledgeBase::new(":memory:");
        kb.initialize().unwrap();
        let tags = vec!["rust".to_string(), "编程语言".to_string(), "系统编程".to_string()];
        kb.store_knowledge("Rust 特性", "Rust 的所有权系统...", None, Some(&tags)).unwrap();
        let results = kb.search_knowledge("Rust", 5).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_returns_newest_first() {
        let kb = KnowledgeBase::new(":memory:");
        kb.initialize().unwrap();
        kb.store_knowledge("共同标题", "旧数据", None, None)
            .unwrap();
        // 等待足够时间确保 created_at 秒级时间戳不同
        std::thread::sleep(std::time::Duration::from_secs(1));
        kb.store_knowledge("共同标题", "新数据", None, None)
            .unwrap();
        let results = kb.search_knowledge("共同", 5).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["content"], "新数据");
        assert_eq!(results[1]["content"], "旧数据");
    }

    #[test]
    fn test_search_empty_database() {
        let kb = KnowledgeBase::new(":memory:");
        kb.initialize().unwrap();
        let results = kb.search_knowledge("anything", 10).unwrap();
        assert!(results.is_empty());
    }
}
