//! Tool invocation audit records.
//!
//! ToolAgent calls are stored separately from project command runs because
//! tools have different risk and approval semantics.

use crate::error::AgentError;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Mutex;

const MAX_TEXT_CHARS: usize = 512;
const MAX_ARRAY_ITEMS: usize = 20;
const MAX_OBJECT_KEYS: usize = 40;
const MAX_REDACTION_DEPTH: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInvocationRecord {
    pub id: String,
    pub task_id: Option<String>,
    pub step_id: Option<String>,
    pub approval_id: Option<String>,
    pub tool_name: String,
    pub args_summary: Value,
    pub success: bool,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInvocationListResponse {
    pub invocations: Vec<ToolInvocationRecord>,
}

#[derive(Debug, Clone)]
pub struct ToolInvocationInput {
    pub task_id: Option<String>,
    pub step_id: Option<String>,
    pub approval_id: Option<String>,
    pub tool_name: String,
    pub args: Value,
    pub success: bool,
    pub error: Option<String>,
    pub duration_ms: u64,
}

impl ToolInvocationRecord {
    pub fn from_input(input: ToolInvocationInput) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            task_id: input.task_id.and_then(clean_optional_text),
            step_id: input.step_id.and_then(clean_optional_text),
            approval_id: input.approval_id.and_then(clean_optional_text),
            tool_name: input.tool_name,
            args_summary: summarize_tool_args(&input.args),
            success: input.success,
            error: input.error.and_then(clean_optional_text),
            duration_ms: input.duration_ms,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// SQLite-backed ToolAgent invocation audit store.
pub struct ToolInvocationStore {
    conn: Mutex<Connection>,
}

impl ToolInvocationStore {
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
            CREATE TABLE IF NOT EXISTS tool_invocations (
                id TEXT PRIMARY KEY,
                task_id TEXT,
                step_id TEXT,
                approval_id TEXT,
                tool_name TEXT NOT NULL,
                args_summary TEXT NOT NULL,
                success INTEGER NOT NULL,
                error TEXT,
                duration_ms INTEGER NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_tool_invocations_created_at
                ON tool_invocations(created_at);
            CREATE INDEX IF NOT EXISTS idx_tool_invocations_task_id
                ON tool_invocations(task_id);
            CREATE INDEX IF NOT EXISTS idx_tool_invocations_approval_id
                ON tool_invocations(approval_id);
            ",
        )?;
        Ok(())
    }

    pub fn append_invocation(&self, record: &ToolInvocationRecord) -> Result<(), AgentError> {
        let conn = self.conn()?;
        let args_summary = serde_json::to_string(&record.args_summary)?;
        conn.execute(
            "INSERT OR REPLACE INTO tool_invocations
             (id, task_id, step_id, approval_id, tool_name, args_summary,
              success, error, duration_ms, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                record.id,
                record.task_id,
                record.step_id,
                record.approval_id,
                record.tool_name,
                args_summary,
                record.success,
                record.error,
                record.duration_ms,
                record.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_invocations(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<ToolInvocationRecord>, AgentError> {
        let limit = limit.unwrap_or(20).clamp(1, 100);
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, step_id, approval_id, tool_name, args_summary,
                    success, error, duration_ms, created_at
             FROM tool_invocations
             ORDER BY created_at DESC, rowid DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            let args_summary: String = row.get(5)?;
            Ok(ToolInvocationRecord {
                id: row.get(0)?,
                task_id: row.get(1)?,
                step_id: row.get(2)?,
                approval_id: row.get(3)?,
                tool_name: row.get(4)?,
                args_summary: serde_json::from_str(&args_summary).unwrap_or(Value::Null),
                success: row.get(6)?,
                error: row.get(7)?,
                duration_ms: row.get(8)?,
                created_at: row.get(9)?,
            })
        })?;

        let mut invocations = Vec::new();
        for row in rows {
            invocations.push(row?);
        }
        Ok(invocations)
    }

    fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, AgentError> {
        self.conn
            .lock()
            .map_err(|err| AgentError::Internal(err.to_string()))
    }
}

pub fn summarize_tool_args(args: &Value) -> Value {
    redact_value(args, None, 0)
}

fn redact_value(value: &Value, key: Option<&str>, depth: usize) -> Value {
    if key.is_some_and(is_sensitive_key) {
        return Value::String("[redacted]".to_string());
    }
    if depth >= MAX_REDACTION_DEPTH {
        return Value::String("[max depth reached]".to_string());
    }

    match value {
        Value::String(text) => Value::String(truncate_text(text)),
        Value::Array(items) => {
            let mut values = items
                .iter()
                .take(MAX_ARRAY_ITEMS)
                .map(|item| redact_value(item, None, depth + 1))
                .collect::<Vec<_>>();
            if items.len() > MAX_ARRAY_ITEMS {
                values.push(Value::String(format!(
                    "[{} more items]",
                    items.len() - MAX_ARRAY_ITEMS
                )));
            }
            Value::Array(values)
        }
        Value::Object(map) => {
            let mut result = serde_json::Map::new();
            for (index, (item_key, item_value)) in map.iter().enumerate() {
                if index >= MAX_OBJECT_KEYS {
                    result.insert(
                        "_truncated".to_string(),
                        Value::String(format!("{} more keys", map.len() - MAX_OBJECT_KEYS)),
                    );
                    break;
                }
                result.insert(
                    item_key.clone(),
                    redact_value(item_value, Some(item_key), depth + 1),
                );
            }
            Value::Object(result)
        }
        other => other.clone(),
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("key")
        || key.contains("token")
        || key.contains("secret")
        || key.contains("password")
        || key.contains("credential")
        || key.contains("authorization")
        || key == "auth"
}

fn truncate_text(text: &str) -> String {
    let mut result = text.chars().take(MAX_TEXT_CHARS).collect::<String>();
    if text.chars().nth(MAX_TEXT_CHARS).is_some() {
        result.push_str("[truncated]");
    }
    result
}

fn clean_optional_text(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_args_with_redaction_and_limits() {
        let summary = summarize_tool_args(&serde_json::json!({
            "query": "rust agent",
            "apiKey": "sk-secret",
            "nested": { "authorization": "Bearer secret" },
            "long": "x".repeat(MAX_TEXT_CHARS + 8),
        }));

        assert_eq!(summary["apiKey"], serde_json::json!("[redacted]"));
        assert_eq!(
            summary["nested"]["authorization"],
            serde_json::json!("[redacted]")
        );
        assert!(summary["long"].as_str().unwrap().ends_with("[truncated]"));
    }

    #[test]
    fn tool_invocation_store_saves_and_lists_recent_records() {
        let store = ToolInvocationStore::open(":memory:").unwrap();
        let first = ToolInvocationRecord {
            id: "tool-run-1".to_string(),
            task_id: Some("task-1".to_string()),
            step_id: Some("step-1".to_string()),
            approval_id: None,
            tool_name: "calculator".to_string(),
            args_summary: serde_json::json!({ "expression": "1+1" }),
            success: true,
            error: None,
            duration_ms: 3,
            created_at: "2026-06-09T01:00:00Z".to_string(),
        };
        let second = ToolInvocationRecord {
            id: "tool-run-2".to_string(),
            approval_id: Some("approval-1".to_string()),
            tool_name: "file_read".to_string(),
            args_summary: serde_json::json!({ "path": "README.md" }),
            success: false,
            error: Some("denied".to_string()),
            duration_ms: 4,
            created_at: "2026-06-09T02:00:00Z".to_string(),
            ..first.clone()
        };

        store.append_invocation(&first).unwrap();
        store.append_invocation(&second).unwrap();

        let invocations = store.list_invocations(Some(1)).unwrap();
        assert_eq!(invocations.len(), 1);
        assert_eq!(invocations[0].id, "tool-run-2");
        assert_eq!(invocations[0].approval_id.as_deref(), Some("approval-1"));
        assert!(!invocations[0].success);
        assert_eq!(
            invocations[0].args_summary["path"],
            serde_json::json!("README.md")
        );
    }
}
