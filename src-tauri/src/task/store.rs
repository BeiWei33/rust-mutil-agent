//! SQLite persistence for tasks and task events.

use crate::error::AgentError;
use crate::task::{Task, TaskEvent};
use rusqlite::{params, Connection};
use std::sync::Mutex;

/// SQLite-backed task/event store.
pub struct TaskStore {
    conn: Mutex<Connection>,
}

impl TaskStore {
    pub fn open(db_path: impl Into<String>) -> Result<Self, AgentError> {
        let db_path = db_path.into();
        let conn = Connection::open(&db_path)?;
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
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                task_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS task_events (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                event_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_tasks_updated_at
                ON tasks(updated_at);
            CREATE INDEX IF NOT EXISTS idx_task_events_task_created
                ON task_events(task_id, created_at);
            ",
        )?;
        Ok(())
    }

    pub fn save_task(&self, task: &Task) -> Result<(), AgentError> {
        let task_json = serde_json::to_string(task)?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO tasks (id, task_json, updated_at)
             VALUES (?1, ?2, ?3)",
            params![task.id, task_json, task.updated_at.to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn append_event(&self, event: &TaskEvent) -> Result<(), AgentError> {
        let event_json = serde_json::to_string(event)?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT OR IGNORE INTO task_events (id, task_id, event_json, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                event.id,
                event.task_id,
                event_json,
                event.created_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn load_tasks(&self) -> Result<Vec<Task>, AgentError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT task_json
             FROM tasks
             ORDER BY updated_at DESC, rowid DESC",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;

        let mut tasks = Vec::new();
        for row in rows {
            let task_json = row?;
            tasks.push(serde_json::from_str(&task_json)?);
        }
        Ok(tasks)
    }

    pub fn load_events(&self) -> Result<Vec<TaskEvent>, AgentError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT event_json
             FROM task_events
             ORDER BY created_at ASC, rowid ASC",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;

        let mut events = Vec::new();
        for row in rows {
            let event_json = row?;
            events.push(serde_json::from_str(&event_json)?);
        }
        Ok(events)
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
    use crate::task::{TaskEventKind, TaskStatus};

    #[test]
    fn task_store_saves_and_loads_tasks_and_events() {
        let store = TaskStore::open(":memory:").unwrap();
        let mut task = Task::new("task-1", "持久化任务");
        task.status = TaskStatus::Running;
        let event = TaskEvent::new(
            "task-1",
            None,
            TaskEventKind::Created,
            "任务已创建。",
            serde_json::Value::Null,
        );

        store.save_task(&task).unwrap();
        store.append_event(&event).unwrap();

        let tasks = store.load_tasks().unwrap();
        let events = store.load_events().unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "task-1");
        assert_eq!(tasks[0].status, TaskStatus::Running);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].task_id, "task-1");
    }

    #[test]
    fn task_store_reopens_file_database() {
        let path = std::env::temp_dir().join(format!(
            "rust-mutil-agent-task-store-{}.db",
            uuid::Uuid::new_v4()
        ));
        let path_str = path.to_string_lossy().to_string();

        {
            let store = TaskStore::open(&path_str).unwrap();
            store.save_task(&Task::new("task-1", "重启恢复")).unwrap();
        }
        {
            let store = TaskStore::open(&path_str).unwrap();
            let tasks = store.load_tasks().unwrap();
            assert_eq!(tasks.len(), 1);
            assert_eq!(tasks[0].user_goal, "重启恢复");
        }

        let _ = std::fs::remove_file(path);
    }
}
