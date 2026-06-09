//! Task event stream types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Event kind emitted while a task changes state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TaskEventKind {
    Created,
    Planned,
    StepStarted,
    StepCompleted,
    StepSkipped,
    StepTimedOut,
    StepFailed,
    Completed,
    Failed,
    Cancelled,
    Retried,
    ApprovalRequested,
    ApprovalResolved,
    ArtifactCreated,
}

/// A timestamped task event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskEvent {
    pub id: String,
    pub task_id: String,
    pub step_id: Option<String>,
    pub kind: TaskEventKind,
    pub message: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl TaskEvent {
    pub fn new(
        task_id: impl Into<String>,
        step_id: Option<String>,
        kind: TaskEventKind,
        message: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            task_id: task_id.into(),
            step_id,
            kind,
            message: message.into(),
            payload,
            created_at: Utc::now(),
        }
    }
}
