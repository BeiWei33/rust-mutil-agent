//! Approval request model, policy helpers, and persistence.

use crate::agent::action::RiskLevel;
use crate::error::AgentError;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Mutex;

/// Approval lifecycle for high-risk actions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
    Cancelled,
}

/// A durable request asking the user to approve one action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRequest {
    pub id: String,
    pub task_id: Option<String>,
    pub step_id: Option<String>,
    pub title: String,
    pub reason: String,
    pub risk: RiskLevel,
    pub action_type: String,
    pub action_payload: Value,
    pub status: ApprovalStatus,
    pub requested_by: String,
    pub decided_by: Option<String>,
    pub decision_note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub decided_at: Option<DateTime<Utc>>,
}

/// Request payload used by tests and future producers to create approvals.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateApprovalRequest {
    pub task_id: Option<String>,
    pub step_id: Option<String>,
    pub title: String,
    pub reason: String,
    pub risk: RiskLevel,
    pub action_type: String,
    pub action_payload: Value,
    pub requested_by: Option<String>,
}

/// Frontend/user decision payload.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalDecisionRequest {
    pub approval_id: String,
    pub approved: bool,
    pub note: Option<String>,
    pub decided_by: Option<String>,
}

/// Approval list response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalListResponse {
    pub approvals: Vec<ApprovalRequest>,
}

/// SQLite-backed approval store.
pub struct ApprovalStore {
    conn: Mutex<Connection>,
}

impl ApprovalRequest {
    pub fn new(input: CreateApprovalRequest) -> Result<Self, AgentError> {
        let title = clean_required("审批标题", &input.title)?;
        let reason = clean_required("审批原因", &input.reason)?;
        let action_type = clean_required("审批动作类型", &input.action_type)?;
        let _requires_approval = requires_user_approval(&input.risk);
        let requested_by = input
            .requested_by
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("system")
            .to_string();
        let now = Utc::now();

        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            task_id: clean_optional(input.task_id.as_ref()),
            step_id: clean_optional(input.step_id.as_ref()),
            title,
            reason,
            risk: input.risk,
            action_type,
            action_payload: input.action_payload,
            status: ApprovalStatus::Pending,
            requested_by,
            decided_by: None,
            decision_note: None,
            created_at: now,
            updated_at: now,
            decided_at: None,
        })
    }

    fn decide(
        &mut self,
        approved: bool,
        decided_by: Option<&str>,
        note: Option<&str>,
    ) -> Result<(), AgentError> {
        if self.status != ApprovalStatus::Pending {
            return Err(AgentError::MessageFormat(format!(
                "审批请求 [{}] 当前状态为 {:?}，不能重复决策",
                self.id, self.status
            )));
        }

        self.status = if approved {
            ApprovalStatus::Approved
        } else {
            ApprovalStatus::Rejected
        };
        self.decided_by = decided_by
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .or_else(|| Some("user".to_string()));
        self.decision_note = note
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        let now = Utc::now();
        self.updated_at = now;
        self.decided_at = Some(now);
        Ok(())
    }
}

impl ApprovalStore {
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
            CREATE TABLE IF NOT EXISTS approval_requests (
                id TEXT PRIMARY KEY,
                status TEXT NOT NULL,
                request_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_approval_requests_status_updated
                ON approval_requests(status, updated_at);
            ",
        )?;
        Ok(())
    }

    pub fn create_request(
        &self,
        input: CreateApprovalRequest,
    ) -> Result<ApprovalRequest, AgentError> {
        let request = ApprovalRequest::new(input)?;
        self.save_request(&request)?;
        Ok(request)
    }

    pub fn save_request(&self, request: &ApprovalRequest) -> Result<(), AgentError> {
        let request_json = serde_json::to_string(request)?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO approval_requests
             (id, status, request_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                request.id,
                status_value(&request.status),
                request_json,
                request.created_at.to_rfc3339(),
                request.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn list_requests(
        &self,
        status: Option<ApprovalStatus>,
        limit: Option<usize>,
    ) -> Result<Vec<ApprovalRequest>, AgentError> {
        let limit = limit.unwrap_or(50).clamp(1, 100);
        let conn = self.conn()?;

        let rows = if let Some(status) = status {
            let mut stmt = conn.prepare(
                "SELECT request_json
                 FROM approval_requests
                 WHERE status = ?1
                 ORDER BY updated_at DESC, rowid DESC
                 LIMIT ?2",
            )?;
            let mapped = stmt.query_map(params![status_value(&status), limit as i64], |row| {
                row.get::<_, String>(0)
            })?;
            mapped.collect::<Result<Vec<_>, _>>()?
        } else {
            let mut stmt = conn.prepare(
                "SELECT request_json
                 FROM approval_requests
                 ORDER BY updated_at DESC, rowid DESC
                 LIMIT ?1",
            )?;
            let mapped = stmt.query_map(params![limit as i64], |row| row.get::<_, String>(0))?;
            mapped.collect::<Result<Vec<_>, _>>()?
        };

        rows.into_iter()
            .map(|raw| serde_json::from_str(&raw).map_err(AgentError::from))
            .collect()
    }

    pub fn decide(
        &self,
        decision: ApprovalDecisionRequest,
    ) -> Result<Option<ApprovalRequest>, AgentError> {
        let Some(mut request) = self.get_request(&decision.approval_id)? else {
            return Ok(None);
        };
        request.decide(
            decision.approved,
            decision.decided_by.as_deref(),
            decision.note.as_deref(),
        )?;
        self.save_request(&request)?;
        Ok(Some(request))
    }

    pub fn get_request(&self, approval_id: &str) -> Result<Option<ApprovalRequest>, AgentError> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT request_json FROM approval_requests WHERE id = ?1 LIMIT 1")?;
        let mut rows = stmt.query(params![approval_id])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let raw: String = row.get(0)?;
        Ok(Some(serde_json::from_str(&raw)?))
    }

    fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, AgentError> {
        self.conn
            .lock()
            .map_err(|err| AgentError::Internal(err.to_string()))
    }
}

/// Whether a risk level must pause for user approval.
pub fn requires_user_approval(risk: &RiskLevel) -> bool {
    matches!(
        risk,
        RiskLevel::Medium | RiskLevel::High | RiskLevel::Critical
    )
}

pub fn parse_approval_status(value: Option<&str>) -> Result<Option<ApprovalStatus>, AgentError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    match value {
        "pending" => Ok(Some(ApprovalStatus::Pending)),
        "approved" => Ok(Some(ApprovalStatus::Approved)),
        "rejected" => Ok(Some(ApprovalStatus::Rejected)),
        "cancelled" => Ok(Some(ApprovalStatus::Cancelled)),
        other => Err(AgentError::MessageFormat(format!(
            "未知审批状态 [{}]",
            other
        ))),
    }
}

fn clean_required(label: &str, value: &str) -> Result<String, AgentError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AgentError::MessageFormat(format!("{label}不能为空")));
    }
    Ok(trimmed.to_string())
}

fn clean_optional(value: Option<&String>) -> Option<String> {
    value
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
}

fn status_value(status: &ApprovalStatus) -> &'static str {
    match status {
        ApprovalStatus::Pending => "pending",
        ApprovalStatus::Approved => "approved",
        ApprovalStatus::Rejected => "rejected",
        ApprovalStatus::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_request(title: &str, risk: RiskLevel) -> CreateApprovalRequest {
        CreateApprovalRequest {
            task_id: Some("task-1".to_string()),
            step_id: Some("step-1".to_string()),
            title: title.to_string(),
            reason: "需要用户确认高风险操作。".to_string(),
            risk,
            action_type: "workspace.applyPatch".to_string(),
            action_payload: serde_json::json!({ "files": ["src/main.rs"] }),
            requested_by: Some("Planner".to_string()),
        }
    }

    #[test]
    fn policy_requires_approval_for_medium_and_above() {
        assert!(!requires_user_approval(&RiskLevel::Low));
        assert!(requires_user_approval(&RiskLevel::Medium));
        assert!(requires_user_approval(&RiskLevel::High));
        assert!(requires_user_approval(&RiskLevel::Critical));
    }

    #[test]
    fn approval_store_creates_lists_and_decides_requests() {
        let store = ApprovalStore::open(":memory:").unwrap();
        let approval = store
            .create_request(create_request("应用 patch", RiskLevel::High))
            .unwrap();

        let pending = store
            .list_requests(Some(ApprovalStatus::Pending), Some(10))
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, approval.id);
        assert_eq!(pending[0].status, ApprovalStatus::Pending);

        let decided = store
            .decide(ApprovalDecisionRequest {
                approval_id: approval.id.clone(),
                approved: true,
                note: Some("同意".to_string()),
                decided_by: Some("tester".to_string()),
            })
            .unwrap()
            .unwrap();

        assert_eq!(decided.status, ApprovalStatus::Approved);
        assert_eq!(decided.decided_by.as_deref(), Some("tester"));
        assert_eq!(decided.decision_note.as_deref(), Some("同意"));
        assert!(decided.decided_at.is_some());
        assert!(store
            .list_requests(Some(ApprovalStatus::Pending), Some(10))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn approval_store_rejects_duplicate_decisions() {
        let store = ApprovalStore::open(":memory:").unwrap();
        let approval = store
            .create_request(create_request("删除文件", RiskLevel::Critical))
            .unwrap();
        store
            .decide(ApprovalDecisionRequest {
                approval_id: approval.id.clone(),
                approved: false,
                note: None,
                decided_by: None,
            })
            .unwrap();

        let err = store
            .decide(ApprovalDecisionRequest {
                approval_id: approval.id,
                approved: true,
                note: None,
                decided_by: None,
            })
            .unwrap_err();
        assert!(err.to_string().contains("不能重复决策"));
    }
}
