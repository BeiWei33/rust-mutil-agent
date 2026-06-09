//! Patch proposal model, diff preview generation, and persistence.

use crate::error::AgentError;
use crate::workspace::workspace_root;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

const MAX_FILES_PER_PROPOSAL: usize = 10;
const MAX_FILE_CONTENT_BYTES: usize = 128 * 1024;
const MAX_TOTAL_DIFF_BYTES: usize = 512 * 1024;
const CONTEXT_LINES: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PatchProposalStatus {
    Draft,
    PendingApproval,
    Approved,
    Rejected,
    Applied,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PatchChangeType {
    Modify,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchFileInput {
    pub path: String,
    pub old_content: String,
    pub new_content: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePatchProposalRequest {
    pub summary: String,
    pub files: Vec<PatchFileInput>,
    pub task_id: Option<String>,
    pub step_id: Option<String>,
    pub requested_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PatchFileChange {
    pub path: String,
    pub change_type: PatchChangeType,
    pub old_content: String,
    pub new_content: String,
    pub diff: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PatchProposal {
    pub id: String,
    pub task_id: Option<String>,
    pub step_id: Option<String>,
    pub approval_id: Option<String>,
    pub summary: String,
    pub status: PatchProposalStatus,
    pub files: Vec<PatchFileChange>,
    pub unified_diff: String,
    pub requested_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub applied_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub applied_by: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchProposalListResponse {
    pub proposals: Vec<PatchProposal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PatchApplyResult {
    pub patch_id: String,
    pub status: PatchProposalStatus,
    pub files: Vec<String>,
    pub applied_at: DateTime<Utc>,
    pub already_applied: bool,
}

/// SQLite-backed patch proposal store.
pub struct PatchProposalStore {
    conn: Mutex<Connection>,
}

impl PatchProposal {
    pub fn attach_approval(&mut self, approval_id: impl Into<String>) {
        self.approval_id = Some(approval_id.into());
        self.status = PatchProposalStatus::PendingApproval;
        self.updated_at = Utc::now();
    }

    pub fn set_status(&mut self, status: PatchProposalStatus) {
        self.status = status;
        self.updated_at = Utc::now();
    }

    fn mark_applied(&mut self, applied_at: DateTime<Utc>, applied_by: Option<String>) {
        self.status = PatchProposalStatus::Applied;
        self.applied_at = Some(applied_at);
        self.applied_by = applied_by;
        self.updated_at = applied_at;
    }
}

impl PatchProposalStore {
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
            CREATE TABLE IF NOT EXISTS patch_proposals (
                id TEXT PRIMARY KEY,
                status TEXT NOT NULL,
                approval_id TEXT,
                proposal_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_patch_proposals_status_updated
                ON patch_proposals(status, updated_at);

            CREATE INDEX IF NOT EXISTS idx_patch_proposals_approval_id
                ON patch_proposals(approval_id);
            ",
        )?;
        Ok(())
    }

    pub fn save_proposal(&self, proposal: &PatchProposal) -> Result<(), AgentError> {
        let proposal_json = serde_json::to_string(proposal)?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO patch_proposals
             (id, status, approval_id, proposal_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                proposal.id,
                status_value(&proposal.status),
                proposal.approval_id,
                proposal_json,
                proposal.created_at.to_rfc3339(),
                proposal.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn list_proposals(&self, limit: Option<usize>) -> Result<Vec<PatchProposal>, AgentError> {
        let limit = limit.unwrap_or(50).clamp(1, 100);
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT proposal_json
             FROM patch_proposals
             ORDER BY updated_at DESC, rowid DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| row.get::<_, String>(0))?;

        let mut proposals = Vec::new();
        for row in rows {
            proposals.push(serde_json::from_str(&row?)?);
        }
        Ok(proposals)
    }

    pub fn get_proposal(&self, patch_id: &str) -> Result<Option<PatchProposal>, AgentError> {
        let patch_id = patch_id.trim();
        if patch_id.is_empty() {
            return Ok(None);
        }

        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT proposal_json FROM patch_proposals WHERE id = ?1 LIMIT 1")?;
        let mut rows = stmt.query(params![patch_id])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let raw: String = row.get(0)?;
        Ok(Some(serde_json::from_str(&raw)?))
    }

    pub fn update_status(
        &self,
        patch_id: &str,
        status: PatchProposalStatus,
    ) -> Result<Option<PatchProposal>, AgentError> {
        let Some(mut proposal) = self.get_proposal(patch_id)? else {
            return Ok(None);
        };
        proposal.set_status(status);
        self.save_proposal(&proposal)?;
        Ok(Some(proposal))
    }

    fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, AgentError> {
        self.conn
            .lock()
            .map_err(|err| AgentError::Internal(err.to_string()))
    }
}

pub fn build_patch_proposal(
    input: CreatePatchProposalRequest,
) -> Result<PatchProposal, AgentError> {
    let summary = clean_required("补丁摘要", &input.summary)?;
    if input.files.is_empty() {
        return Err(AgentError::MessageFormat(
            "补丁提案至少需要包含一个文件变更".to_string(),
        ));
    }
    if input.files.len() > MAX_FILES_PER_PROPOSAL {
        return Err(AgentError::MessageFormat(format!(
            "单个补丁提案最多支持 {} 个文件",
            MAX_FILES_PER_PROPOSAL
        )));
    }

    let mut files = Vec::with_capacity(input.files.len());
    for file in input.files {
        files.push(build_file_change(file)?);
    }

    let unified_diff = files
        .iter()
        .map(|file| file.diff.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if unified_diff.len() > MAX_TOTAL_DIFF_BYTES {
        return Err(AgentError::MessageFormat(format!(
            "补丁 diff 超过大小限制 {} bytes",
            MAX_TOTAL_DIFF_BYTES
        )));
    }

    let now = Utc::now();
    Ok(PatchProposal {
        id: uuid::Uuid::new_v4().to_string(),
        task_id: clean_optional(input.task_id.as_ref()),
        step_id: clean_optional(input.step_id.as_ref()),
        approval_id: None,
        summary,
        status: PatchProposalStatus::Draft,
        files,
        unified_diff,
        requested_by: input
            .requested_by
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("system")
            .to_string(),
        created_at: now,
        updated_at: now,
        applied_at: None,
        applied_by: None,
    })
}

pub fn apply_patch_proposal(
    proposal: &mut PatchProposal,
    applied_by: Option<&str>,
) -> Result<PatchApplyResult, AgentError> {
    if proposal.status == PatchProposalStatus::Applied {
        let applied_at = proposal.applied_at.unwrap_or_else(|| {
            let now = Utc::now();
            proposal.applied_at = Some(now);
            now
        });
        return Ok(PatchApplyResult {
            patch_id: proposal.id.clone(),
            status: proposal.status.clone(),
            files: proposal
                .files
                .iter()
                .map(|file| file.path.clone())
                .collect(),
            applied_at,
            already_applied: true,
        });
    }

    if proposal.status != PatchProposalStatus::Approved {
        return Err(AgentError::MessageFormat(format!(
            "补丁提案 [{}] 当前状态为 {:?}，不能应用",
            proposal.id, proposal.status
        )));
    }

    let mut resolved_files = Vec::with_capacity(proposal.files.len());
    for file in &proposal.files {
        match file.change_type {
            PatchChangeType::Modify => {
                let (path, file_path) = resolve_existing_text_file(&file.path)?;
                let current_content = fs::read_to_string(&file_path).map_err(|err| {
                    AgentError::Internal(format!("读取当前文件失败 [{}]: {err}", path))
                })?;
                if current_content != file.old_content {
                    return Err(AgentError::MessageFormat(format!(
                        "文件 [{}] 当前内容与补丁基线不一致，已拒绝应用",
                        path
                    )));
                }
                resolved_files.push((path, file_path, file.new_content.clone()));
            }
        }
    }

    for (path, file_path, new_content) in &resolved_files {
        fs::write(file_path, new_content)
            .map_err(|err| AgentError::Internal(format!("写入补丁文件失败 [{}]: {err}", path)))?;
    }

    let applied_at = Utc::now();
    proposal.mark_applied(applied_at, clean_applied_by(applied_by));
    Ok(PatchApplyResult {
        patch_id: proposal.id.clone(),
        status: proposal.status.clone(),
        files: resolved_files
            .into_iter()
            .map(|(path, _, _)| path)
            .collect(),
        applied_at,
        already_applied: false,
    })
}

fn build_file_change(input: PatchFileInput) -> Result<PatchFileChange, AgentError> {
    let (path, file_path) = resolve_existing_text_file(&input.path)?;
    validate_content_size("原始内容", &input.old_content)?;
    validate_content_size("目标内容", &input.new_content)?;

    if input.old_content == input.new_content {
        return Err(AgentError::MessageFormat(format!(
            "文件 [{}] 没有内容变化",
            path
        )));
    }

    let current_content = fs::read_to_string(&file_path)
        .map_err(|err| AgentError::Internal(format!("读取当前文件失败 [{}]: {err}", path)))?;
    if current_content != input.old_content {
        return Err(AgentError::MessageFormat(format!(
            "文件 [{}] 当前内容与提案基线不一致，请刷新后重新生成 diff",
            path
        )));
    }

    let diff = build_unified_diff(&path, &input.old_content, &input.new_content);
    Ok(PatchFileChange {
        path,
        change_type: PatchChangeType::Modify,
        old_content: input.old_content,
        new_content: input.new_content,
        diff,
    })
}

pub fn build_unified_diff(path: &str, old_content: &str, new_content: &str) -> String {
    let old_lines = old_content.lines().collect::<Vec<_>>();
    let new_lines = new_content.lines().collect::<Vec<_>>();

    let mut prefix = 0;
    while prefix < old_lines.len()
        && prefix < new_lines.len()
        && old_lines[prefix] == new_lines[prefix]
    {
        prefix += 1;
    }

    let mut suffix = 0;
    while suffix < old_lines.len().saturating_sub(prefix)
        && suffix < new_lines.len().saturating_sub(prefix)
        && old_lines[old_lines.len() - 1 - suffix] == new_lines[new_lines.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let old_change_end = old_lines.len() - suffix;
    let new_change_end = new_lines.len() - suffix;
    let context_start = prefix.saturating_sub(CONTEXT_LINES);
    let old_context_end = (old_change_end + CONTEXT_LINES).min(old_lines.len());
    let new_context_end = (new_change_end + CONTEXT_LINES).min(new_lines.len());
    let old_hunk_len = old_context_end.saturating_sub(context_start);
    let new_hunk_len = new_context_end.saturating_sub(context_start);
    let old_start = if old_hunk_len == 0 {
        0
    } else {
        context_start + 1
    };
    let new_start = if new_hunk_len == 0 {
        0
    } else {
        context_start + 1
    };

    let mut diff = String::new();
    diff.push_str(&format!("diff --git a/{path} b/{path}\n"));
    diff.push_str(&format!("--- a/{path}\n"));
    diff.push_str(&format!("+++ b/{path}\n"));
    diff.push_str(&format!(
        "@@ -{} +{} @@\n",
        hunk_range(old_start, old_hunk_len),
        hunk_range(new_start, new_hunk_len)
    ));

    for line in &old_lines[context_start..prefix] {
        diff.push_str(&format!(" {line}\n"));
    }
    for line in &old_lines[prefix..old_change_end] {
        diff.push_str(&format!("-{line}\n"));
    }
    for line in &new_lines[prefix..new_change_end] {
        diff.push_str(&format!("+{line}\n"));
    }
    for line in &old_lines[old_change_end..old_context_end] {
        diff.push_str(&format!(" {line}\n"));
    }

    diff
}

fn resolve_existing_text_file(path: &str) -> Result<(String, PathBuf), AgentError> {
    let rel_path = normalize_rel_path(path)?;
    let root = workspace_root();
    let canonical_root = root.canonicalize().map_err(|err| {
        AgentError::Internal(format!(
            "解析 workspace root 失败 [{}]: {err}",
            root.display()
        ))
    })?;
    let canonical_file = canonical_root
        .join(Path::new(&rel_path))
        .canonicalize()
        .map_err(|err| {
            AgentError::MessageFormat(format!("文件 [{}] 不存在或不可访问: {err}", path))
        })?;

    if !canonical_file.starts_with(&canonical_root) {
        return Err(AgentError::MessageFormat(format!(
            "路径 [{}] 不允许访问 workspace 外部",
            path
        )));
    }
    if !canonical_file.is_file() {
        return Err(AgentError::MessageFormat(format!(
            "路径 [{}] 不是可修改的文件",
            path
        )));
    }
    if is_blocked_path(&canonical_file) {
        return Err(AgentError::MessageFormat(format!(
            "路径 [{}] 处于受保护位置，不能生成补丁提案",
            path
        )));
    }

    let relative = canonical_file
        .strip_prefix(&canonical_root)
        .unwrap_or(&canonical_file)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/");

    Ok((relative, canonical_file))
}

fn normalize_rel_path(path: &str) -> Result<String, AgentError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(AgentError::MessageFormat(
            "补丁文件路径不能为空".to_string(),
        ));
    }

    let normalized = trimmed.replace('\\', "/");
    let candidate = Path::new(&normalized);
    if candidate.is_absolute() {
        return Err(AgentError::MessageFormat(format!(
            "路径 [{}] 不允许访问 workspace 外部",
            path
        )));
    }

    let mut parts = Vec::new();
    for component in candidate.components() {
        match component {
            Component::Normal(value) => parts.push(value.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(AgentError::MessageFormat(format!(
                    "路径 [{}] 不允许访问 workspace 外部",
                    path
                )));
            }
        }
    }

    if parts.is_empty() {
        return Err(AgentError::MessageFormat(
            "补丁文件路径不能为空".to_string(),
        ));
    }
    Ok(parts.join("/"))
}

fn is_blocked_path(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy().to_lowercase();
        matches!(
            name.as_str(),
            ".git" | "node_modules" | "target" | "dist" | "build" | ".next" | ".tauri" | ".vite"
        )
    }) || path
        .file_name()
        .map(|value| {
            let name = value.to_string_lossy().to_lowercase();
            name == ".env"
                || name.ends_with(".pem")
                || name.ends_with(".key")
                || name.ends_with(".p12")
                || name.ends_with(".pfx")
                || name.contains("secret")
                || name.contains("token")
        })
        .unwrap_or(false)
}

fn validate_content_size(label: &str, value: &str) -> Result<(), AgentError> {
    if value.len() > MAX_FILE_CONTENT_BYTES {
        return Err(AgentError::MessageFormat(format!(
            "{}超过大小限制 {} bytes",
            label, MAX_FILE_CONTENT_BYTES
        )));
    }
    Ok(())
}

fn hunk_range(start: usize, len: usize) -> String {
    if len == 1 {
        start.to_string()
    } else {
        format!("{start},{len}")
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

fn clean_applied_by(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
}

fn status_value(status: &PatchProposalStatus) -> &'static str {
    match status {
        PatchProposalStatus::Draft => "draft",
        PatchProposalStatus::PendingApproval => "pendingApproval",
        PatchProposalStatus::Approved => "approved",
        PatchProposalStatus::Rejected => "rejected",
        PatchProposalStatus::Applied => "applied",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unified_diff_shows_context_and_changes() {
        let diff = build_unified_diff(
            "src/main.rs",
            "one\ntwo\nthree\nfour\nfive\n",
            "one\ntwo\nTHREE\nfour\nfive\n",
        );

        assert!(diff.contains("diff --git a/src/main.rs b/src/main.rs"));
        assert!(diff.contains("-three"));
        assert!(diff.contains("+THREE"));
        assert!(diff.contains(" two"));
        assert!(diff.contains(" four"));
    }

    #[test]
    fn patch_store_saves_lists_gets_and_updates_status() {
        let store = PatchProposalStore::open(":memory:").unwrap();
        let now = Utc::now();
        let proposal = PatchProposal {
            id: "patch-1".to_string(),
            task_id: Some("task-1".to_string()),
            step_id: None,
            approval_id: Some("approval-1".to_string()),
            summary: "更新 README".to_string(),
            status: PatchProposalStatus::PendingApproval,
            files: vec![PatchFileChange {
                path: "README.md".to_string(),
                change_type: PatchChangeType::Modify,
                old_content: "old".to_string(),
                new_content: "new".to_string(),
                diff: build_unified_diff("README.md", "old", "new"),
            }],
            unified_diff: build_unified_diff("README.md", "old", "new"),
            requested_by: "test".to_string(),
            created_at: now,
            updated_at: now,
            applied_at: None,
            applied_by: None,
        };

        store.save_proposal(&proposal).unwrap();

        let proposals = store.list_proposals(Some(10)).unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].id, "patch-1");

        let updated = store
            .update_status("patch-1", PatchProposalStatus::Approved)
            .unwrap()
            .unwrap();
        assert_eq!(updated.status, PatchProposalStatus::Approved);

        let loaded = store.get_proposal("patch-1").unwrap().unwrap();
        assert_eq!(loaded.status, PatchProposalStatus::Approved);
    }

    #[test]
    fn build_patch_proposal_rejects_parent_traversal() {
        let err = build_patch_proposal(CreatePatchProposalRequest {
            summary: "bad".to_string(),
            files: vec![PatchFileInput {
                path: "../README.md".to_string(),
                old_content: "old".to_string(),
                new_content: "new".to_string(),
            }],
            task_id: None,
            step_id: None,
            requested_by: None,
        })
        .unwrap_err();

        assert!(err.to_string().contains("workspace 外部"));
    }

    struct TestFile {
        path: PathBuf,
    }

    impl Drop for TestFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    fn workspace_test_file(content: &str) -> (String, TestFile) {
        let name = format!("patch-apply-test-{}.txt", uuid::Uuid::new_v4());
        let path = workspace_root().join(&name);
        fs::write(&path, content).unwrap();
        (name, TestFile { path })
    }

    fn approved_test_proposal(path: String, old_content: &str, new_content: &str) -> PatchProposal {
        let now = Utc::now();
        PatchProposal {
            id: "patch-apply-1".to_string(),
            task_id: None,
            step_id: None,
            approval_id: Some("approval-1".to_string()),
            summary: "应用测试补丁".to_string(),
            status: PatchProposalStatus::Approved,
            files: vec![PatchFileChange {
                path: path.clone(),
                change_type: PatchChangeType::Modify,
                old_content: old_content.to_string(),
                new_content: new_content.to_string(),
                diff: build_unified_diff(&path, old_content, new_content),
            }],
            unified_diff: build_unified_diff(&path, old_content, new_content),
            requested_by: "test".to_string(),
            created_at: now,
            updated_at: now,
            applied_at: None,
            applied_by: None,
        }
    }

    #[test]
    fn apply_patch_proposal_writes_files_and_is_idempotent_after_apply() {
        let (path, test_file) = workspace_test_file("old\n");
        let mut proposal = approved_test_proposal(path.clone(), "old\n", "new\n");

        let result = apply_patch_proposal(&mut proposal, Some("tester")).unwrap();

        assert_eq!(result.patch_id, proposal.id);
        assert_eq!(result.status, PatchProposalStatus::Applied);
        assert_eq!(result.files, vec![path.clone()]);
        assert!(!result.already_applied);
        assert_eq!(proposal.status, PatchProposalStatus::Applied);
        assert_eq!(proposal.applied_by.as_deref(), Some("tester"));
        assert_eq!(fs::read_to_string(&test_file.path).unwrap(), "new\n");

        let second = apply_patch_proposal(&mut proposal, Some("tester")).unwrap();
        assert!(second.already_applied);
        assert_eq!(fs::read_to_string(&test_file.path).unwrap(), "new\n");
    }

    #[test]
    fn apply_patch_proposal_rejects_baseline_mismatch() {
        let (path, test_file) = workspace_test_file("changed\n");
        let mut proposal = approved_test_proposal(path, "old\n", "new\n");

        let err = apply_patch_proposal(&mut proposal, Some("tester")).unwrap_err();

        assert!(err.to_string().contains("基线不一致"));
        assert_eq!(proposal.status, PatchProposalStatus::Approved);
        assert_eq!(fs::read_to_string(&test_file.path).unwrap(), "changed\n");
    }
}
