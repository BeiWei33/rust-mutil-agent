//! Task and step state for the software-engineering task runtime.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Overall task lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatus {
    Draft,
    Planning,
    WaitingApproval,
    Running,
    Reviewing,
    Failed,
    Completed,
    Cancelled,
}

/// Individual step lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StepStatus {
    Pending,
    WaitingApproval,
    Running,
    TimedOut,
    Failed,
    Completed,
    Skipped,
}

/// A single step in a task plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStep {
    pub id: String,
    pub task_id: String,
    pub order: u32,
    pub agent_id: String,
    pub title: String,
    pub instruction: String,
    pub status: StepStatus,
    pub depends_on: Vec<String>,
    pub attempts: u32,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl TaskStep {
    pub fn new(
        id: impl Into<String>,
        task_id: impl Into<String>,
        order: u32,
        agent_id: impl Into<String>,
        instruction: impl Into<String>,
        depends_on: Vec<String>,
    ) -> Self {
        let instruction = instruction.into();
        Self {
            id: id.into(),
            task_id: task_id.into(),
            order,
            agent_id: agent_id.into(),
            title: summarize_title(&instruction, 32),
            instruction,
            status: StepStatus::Pending,
            depends_on,
            attempts: 0,
            result: None,
            error: None,
            started_at: None,
            completed_at: None,
        }
    }

    pub fn start(&mut self) {
        self.status = StepStatus::Running;
        self.attempts += 1;
        self.started_at = Some(Utc::now());
        self.error = None;
    }

    pub fn complete(&mut self, result: serde_json::Value) {
        self.status = StepStatus::Completed;
        self.result = Some(result);
        self.completed_at = Some(Utc::now());
        self.error = None;
    }

    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = StepStatus::Failed;
        self.error = Some(error.into());
        self.completed_at = Some(Utc::now());
    }

    pub fn timeout(&mut self, error: impl Into<String>) {
        self.status = StepStatus::TimedOut;
        self.error = Some(error.into());
        self.completed_at = Some(Utc::now());
    }

    pub fn reset_for_retry(&mut self) {
        self.status = StepStatus::Pending;
        self.result = None;
        self.error = None;
        self.started_at = None;
        self.completed_at = None;
    }
}

/// A software-engineering task tracked by the orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub title: String,
    pub user_goal: String,
    pub status: TaskStatus,
    pub steps: Vec<TaskStep>,
    pub artifacts: Vec<serde_json::Value>,
    pub output: Option<String>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Task {
    pub fn new(id: impl Into<String>, user_goal: impl Into<String>) -> Self {
        let id = id.into();
        let user_goal = user_goal.into();
        let now = Utc::now();
        Self {
            id,
            title: summarize_title(&user_goal, 40),
            user_goal,
            status: TaskStatus::Planning,
            steps: Vec::new(),
            artifacts: Vec::new(),
            output: None,
            error: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    pub fn set_steps(&mut self, steps: Vec<TaskStep>) {
        self.steps = steps;
        self.status = if self.steps.is_empty() {
            TaskStatus::Completed
        } else {
            TaskStatus::Running
        };
        self.touch();
    }

    pub fn complete(&mut self, output: impl Into<String>) {
        self.status = TaskStatus::Completed;
        self.output = Some(output.into());
        self.error = None;
        self.touch();
    }

    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = TaskStatus::Failed;
        self.error = Some(error.into());
        self.touch();
    }

    pub fn cancel(&mut self, reason: impl Into<String>) {
        self.status = TaskStatus::Cancelled;
        self.output = Some(reason.into());
        self.error = None;
        self.touch();
    }

    pub fn retry(&mut self) {
        self.status = if self.steps.is_empty() {
            TaskStatus::Planning
        } else {
            TaskStatus::Running
        };
        self.output = None;
        self.error = None;
        self.touch();
    }

    pub fn add_artifact(&mut self, artifact: serde_json::Value) {
        self.artifacts.push(artifact);
        self.touch();
    }
}

fn summarize_title(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return "未命名任务".to_string();
    }

    let mut chars = normalized.chars();
    let mut title = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        title.push_str("...");
    }
    title
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_starts_in_planning() {
        let task = Task::new("task-1", "实现任务看板");
        assert_eq!(task.status, TaskStatus::Planning);
        assert_eq!(task.title, "实现任务看板");
    }

    #[test]
    fn step_lifecycle_updates_status_and_attempts() {
        let mut step = TaskStep::new("s1", "t1", 1, "Executor", "执行任务", vec![]);
        step.start();
        assert_eq!(step.status, StepStatus::Running);
        assert_eq!(step.attempts, 1);

        step.complete(serde_json::json!({ "ok": true }));
        assert_eq!(step.status, StepStatus::Completed);
        assert!(step.result.is_some());
    }
}
