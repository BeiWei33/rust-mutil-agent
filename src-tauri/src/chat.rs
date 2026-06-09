//! Persistent chat history storage.

use crate::error::AgentError;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// A single chat message stored for a frontend session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
    pub sender_name: Option<String>,
}

/// SQLite-backed chat history store.
pub struct ChatStore {
    conn: Mutex<Connection>,
}

impl ChatStore {
    pub fn open(db_path: impl Into<String>) -> Result<Self, AgentError> {
        let conn = Connection::open(db_path.into())?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.initialize()?;
        Ok(store)
    }

    pub fn initialize(&self) -> Result<(), AgentError> {
        let conn = self.conn()?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS chat_messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                sender_name TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_chat_messages_session_time
                ON chat_messages(session_id, timestamp);
            ",
        )?;
        Ok(())
    }

    pub fn append_message(&self, message: &ChatMessage) -> Result<(), AgentError> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO chat_messages
             (id, session_id, role, content, timestamp, sender_name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                message.id,
                message.session_id,
                message.role,
                message.content,
                message.timestamp,
                message.sender_name
            ],
        )?;
        Ok(())
    }

    pub fn list_messages(&self, session_id: &str) -> Result<Vec<ChatMessage>, AgentError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, content, timestamp, sender_name
             FROM chat_messages
             WHERE session_id = ?1
             ORDER BY timestamp ASC, rowid ASC",
        )?;
        let rows = stmt.query_map(params![session_id], |row| {
            Ok(ChatMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                timestamp: row.get(4)?,
                sender_name: row.get(5)?,
            })
        })?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    pub fn clear_session(&self, session_id: &str) -> Result<(), AgentError> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM chat_messages WHERE session_id = ?1",
            params![session_id],
        )?;
        Ok(())
    }

    fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, AgentError> {
        self.conn
            .lock()
            .map_err(|err| AgentError::Internal(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(id: &str, session_id: &str, content: &str) -> ChatMessage {
        ChatMessage {
            id: id.to_string(),
            session_id: session_id.to_string(),
            role: "user".to_string(),
            content: content.to_string(),
            timestamp: format!("2026-06-09T10:00:0{id}Z"),
            sender_name: None,
        }
    }

    #[test]
    fn chat_store_saves_lists_and_clears_messages() {
        let store = ChatStore::open(":memory:").unwrap();
        store
            .append_message(&message("1", "session-a", "第一条"))
            .unwrap();
        store
            .append_message(&message("2", "session-a", "第二条"))
            .unwrap();
        store
            .append_message(&message("3", "session-b", "其他会话"))
            .unwrap();

        let messages = store.list_messages("session-a").unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "第一条");
        assert_eq!(messages[1].content, "第二条");

        store.clear_session("session-a").unwrap();
        assert!(store.list_messages("session-a").unwrap().is_empty());
        assert_eq!(store.list_messages("session-b").unwrap().len(), 1);
    }
}
