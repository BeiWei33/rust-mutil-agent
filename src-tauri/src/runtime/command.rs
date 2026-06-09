//! Controlled project command runner.
//!
//! Commands are matched against a small allowlist and executed without a shell.

use crate::error::AgentError;
use crate::workspace::workspace_root;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::time::timeout;

const COMMAND_TIMEOUT_SECS: u64 = 120;
const MAX_OUTPUT_BYTES: usize = 96 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandRunRequest {
    pub command: String,
    pub working_dir: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandRunResponse {
    pub id: String,
    pub command: String,
    pub working_dir: String,
    pub exit_code: Option<i32>,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub timed_out: bool,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandRunListResponse {
    pub runs: Vec<ProjectCommandRunResponse>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommandInspection {
    pub command: String,
    pub working_dir: String,
    pub allowed: bool,
}

/// SQLite-backed command run audit store.
pub struct CommandRunStore {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone, Copy)]
struct AllowedCommand {
    command: &'static str,
    working_dir: &'static str,
    program: &'static str,
    args: &'static [&'static str],
}

#[derive(Debug, Clone)]
struct PreparedCommand {
    spec: AllowedCommand,
    working_dir_path: PathBuf,
    display_command: String,
    display_working_dir: String,
}

const ALLOWED_COMMANDS: &[AllowedCommand] = &[
    AllowedCommand {
        command: "cargo check",
        working_dir: "src-tauri",
        program: "cargo",
        args: &["check"],
    },
    AllowedCommand {
        command: "cargo test",
        working_dir: "src-tauri",
        program: "cargo",
        args: &["test"],
    },
    AllowedCommand {
        command: "npm test -- --run",
        working_dir: "src-web",
        program: "npm",
        args: &["test", "--", "--run"],
    },
    AllowedCommand {
        command: "npm run build",
        working_dir: "src-web",
        program: "npm",
        args: &["run", "build"],
    },
];

pub async fn run_project_command(
    request: ProjectCommandRunRequest,
) -> Result<ProjectCommandRunResponse, AgentError> {
    let prepared = prepare_project_command(&request)?;
    let start = Instant::now();
    let mut command = Command::new(platform_program(prepared.spec.program));
    command
        .args(prepared.spec.args)
        .current_dir(&prepared.working_dir_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let run = command.output();
    let output = match timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), run).await {
        Ok(output) => output.map_err(|err| {
            AgentError::Internal(format!(
                "启动项目命令失败 [{}]: {err}",
                prepared.display_command
            ))
        })?,
        Err(_) => {
            return Ok(ProjectCommandRunResponse {
                id: uuid::Uuid::new_v4().to_string(),
                command: prepared.display_command,
                working_dir: prepared.display_working_dir,
                exit_code: None,
                success: false,
                stdout: String::new(),
                stderr: format!("命令执行超过 {COMMAND_TIMEOUT_SECS} 秒，已终止。"),
                duration_ms: elapsed_ms(start),
                timed_out: true,
                stdout_truncated: false,
                stderr_truncated: false,
                created_at: chrono::Utc::now().to_rfc3339(),
            });
        }
    };

    let (stdout, stdout_truncated) = decode_output(&output.stdout);
    let (stderr, stderr_truncated) = decode_output(&output.stderr);
    let exit_code = output.status.code();

    Ok(ProjectCommandRunResponse {
        id: uuid::Uuid::new_v4().to_string(),
        command: prepared.display_command,
        working_dir: prepared.display_working_dir,
        exit_code,
        success: output.status.success(),
        stdout,
        stderr,
        duration_ms: elapsed_ms(start),
        timed_out: false,
        stdout_truncated,
        stderr_truncated,
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

pub fn inspect_project_command_request(
    request: &ProjectCommandRunRequest,
) -> Result<ProjectCommandInspection, AgentError> {
    inspect_project_command_request_with_path(request).map(|(inspection, _)| inspection)
}

impl CommandRunStore {
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
            CREATE TABLE IF NOT EXISTS command_runs (
                id TEXT PRIMARY KEY,
                command TEXT NOT NULL,
                working_dir TEXT NOT NULL,
                exit_code INTEGER,
                success INTEGER NOT NULL,
                stdout TEXT NOT NULL,
                stderr TEXT NOT NULL,
                duration_ms INTEGER NOT NULL,
                timed_out INTEGER NOT NULL,
                stdout_truncated INTEGER NOT NULL,
                stderr_truncated INTEGER NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_command_runs_created_at
                ON command_runs(created_at);
            ",
        )?;
        Ok(())
    }

    pub fn append_run(&self, run: &ProjectCommandRunResponse) -> Result<(), AgentError> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO command_runs
             (id, command, working_dir, exit_code, success, stdout, stderr, duration_ms,
              timed_out, stdout_truncated, stderr_truncated, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                run.id,
                run.command,
                run.working_dir,
                run.exit_code,
                run.success,
                run.stdout,
                run.stderr,
                run.duration_ms,
                run.timed_out,
                run.stdout_truncated,
                run.stderr_truncated,
                run.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_runs(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<ProjectCommandRunResponse>, AgentError> {
        let limit = limit.unwrap_or(20).clamp(1, 100);
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, command, working_dir, exit_code, success, stdout, stderr,
                    duration_ms, timed_out, stdout_truncated, stderr_truncated, created_at
             FROM command_runs
             ORDER BY created_at DESC, rowid DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(ProjectCommandRunResponse {
                id: row.get(0)?,
                command: row.get(1)?,
                working_dir: row.get(2)?,
                exit_code: row.get(3)?,
                success: row.get(4)?,
                stdout: row.get(5)?,
                stderr: row.get(6)?,
                duration_ms: row.get(7)?,
                timed_out: row.get(8)?,
                stdout_truncated: row.get(9)?,
                stderr_truncated: row.get(10)?,
                created_at: row.get(11)?,
            })
        })?;

        let mut runs = Vec::new();
        for row in rows {
            runs.push(row?);
        }
        Ok(runs)
    }

    fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, AgentError> {
        self.conn
            .lock()
            .map_err(|err| AgentError::Internal(err.to_string()))
    }
}

fn prepare_project_command(
    request: &ProjectCommandRunRequest,
) -> Result<PreparedCommand, AgentError> {
    let (inspection, working_dir_path) = inspect_project_command_request_with_path(request)?;
    let spec = ALLOWED_COMMANDS
        .iter()
        .copied()
        .find(|allowed| {
            allowed.command == inspection.command && allowed.working_dir == inspection.working_dir
        })
        .ok_or_else(|| {
            AgentError::MessageFormat(format!(
                "命令 [{}] 不在受控允许列表中，或工作目录 [{}] 不匹配",
                inspection.command, inspection.working_dir
            ))
        })?;

    Ok(PreparedCommand {
        spec,
        working_dir_path,
        display_command: inspection.command,
        display_working_dir: inspection.working_dir,
    })
}

fn inspect_project_command_request_with_path(
    request: &ProjectCommandRunRequest,
) -> Result<(ProjectCommandInspection, PathBuf), AgentError> {
    let command = normalize_command(&request.command);
    if command.is_empty() {
        return Err(AgentError::MessageFormat("项目命令不能为空".to_string()));
    }
    if contains_shell_metacharacter(&request.command) {
        return Err(AgentError::MessageFormat(
            "项目命令不能包含 shell 控制字符".to_string(),
        ));
    }

    let (working_dir_path, working_dir) = resolve_working_dir(&request.working_dir)?;
    let allowed = ALLOWED_COMMANDS
        .iter()
        .any(|allowed| allowed.command == command && allowed.working_dir == working_dir);

    Ok((
        ProjectCommandInspection {
            command,
            working_dir,
            allowed,
        },
        working_dir_path,
    ))
}

fn resolve_working_dir(working_dir: &str) -> Result<(PathBuf, String), AgentError> {
    let rel = working_dir.trim();
    if rel.is_empty() {
        return Err(AgentError::MessageFormat(
            "命令工作目录不能为空".to_string(),
        ));
    }

    let candidate = Path::new(rel);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(AgentError::MessageFormat(format!(
            "工作目录 [{}] 不允许访问 workspace 外部",
            working_dir
        )));
    }

    let root = workspace_root();
    let canonical_root = root.canonicalize().map_err(|err| {
        AgentError::Internal(format!(
            "解析 workspace root 失败 [{}]: {err}",
            root.display()
        ))
    })?;
    let canonical_dir = canonical_root
        .join(candidate)
        .canonicalize()
        .map_err(|err| {
            AgentError::MessageFormat(format!(
                "工作目录 [{}] 不存在或不可访问: {err}",
                working_dir
            ))
        })?;

    if !canonical_dir.starts_with(&canonical_root) {
        return Err(AgentError::MessageFormat(format!(
            "工作目录 [{}] 不允许访问 workspace 外部",
            working_dir
        )));
    }
    if !canonical_dir.is_dir() {
        return Err(AgentError::MessageFormat(format!(
            "工作目录 [{}] 不是目录",
            working_dir
        )));
    }

    let display = canonical_dir
        .strip_prefix(&canonical_root)
        .unwrap_or(&canonical_dir)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/");

    Ok((canonical_dir, display))
}

fn normalize_command(command: &str) -> String {
    command.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn contains_shell_metacharacter(command: &str) -> bool {
    command
        .chars()
        .any(|ch| matches!(ch, ';' | '&' | '|' | '<' | '>' | '\n' | '\r'))
}

fn platform_program(program: &str) -> &str {
    if cfg!(windows) && program == "npm" {
        "npm.cmd"
    } else {
        program
    }
}

fn decode_output(bytes: &[u8]) -> (String, bool) {
    let truncated = bytes.len() > MAX_OUTPUT_BYTES;
    let slice = if truncated {
        &bytes[..MAX_OUTPUT_BYTES]
    } else {
        bytes
    };
    let mut text = String::from_utf8_lossy(slice).to_string();
    if truncated {
        text.push_str("\n\n[output truncated]");
    }
    (text, truncated)
}

fn elapsed_ms(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(command: &str, working_dir: &str) -> ProjectCommandRunRequest {
        ProjectCommandRunRequest {
            command: command.to_string(),
            working_dir: working_dir.to_string(),
        }
    }

    #[test]
    fn prepares_allowed_rust_command() {
        let prepared = prepare_project_command(&request(" cargo   check ", "src-tauri")).unwrap();
        assert_eq!(prepared.display_command, "cargo check");
        assert_eq!(prepared.display_working_dir, "src-tauri");
        assert_eq!(prepared.spec.args, &["check"]);
    }

    #[test]
    fn prepares_allowed_frontend_command() {
        let prepared = prepare_project_command(&request("npm test -- --run", "src-web")).unwrap();
        assert_eq!(prepared.display_command, "npm test -- --run");
        assert_eq!(prepared.display_working_dir, "src-web");
        assert_eq!(prepared.spec.args, &["test", "--", "--run"]);
    }

    #[test]
    fn rejects_shell_control_characters() {
        let err =
            prepare_project_command(&request("cargo test && echo bad", "src-tauri")).unwrap_err();
        assert!(err.to_string().contains("shell 控制字符"));
    }

    #[test]
    fn rejects_unlisted_commands() {
        let err = prepare_project_command(&request("cargo clippy", "src-tauri")).unwrap_err();
        assert!(err.to_string().contains("允许列表"));
    }

    #[test]
    fn inspects_unlisted_command_without_allowing_execution() {
        let inspection =
            inspect_project_command_request(&request(" cargo   clippy ", "src-tauri")).unwrap();
        assert_eq!(inspection.command, "cargo clippy");
        assert_eq!(inspection.working_dir, "src-tauri");
        assert!(!inspection.allowed);
    }

    #[test]
    fn rejects_parent_traversal_working_dir() {
        let err = prepare_project_command(&request("cargo test", "../src-tauri")).unwrap_err();
        assert!(err.to_string().contains("workspace 外部"));
    }

    #[test]
    fn decodes_and_truncates_large_output() {
        let bytes = vec![b'a'; MAX_OUTPUT_BYTES + 4];
        let (text, truncated) = decode_output(&bytes);
        assert!(truncated);
        assert!(text.ends_with("[output truncated]"));
    }

    #[test]
    fn command_run_store_saves_and_lists_recent_runs() {
        let store = CommandRunStore::open(":memory:").unwrap();
        let first = ProjectCommandRunResponse {
            id: "run-1".to_string(),
            command: "cargo check".to_string(),
            working_dir: "src-tauri".to_string(),
            exit_code: Some(0),
            success: true,
            stdout: "ok".to_string(),
            stderr: String::new(),
            duration_ms: 12,
            timed_out: false,
            stdout_truncated: false,
            stderr_truncated: false,
            created_at: "2026-06-09T01:00:00Z".to_string(),
        };
        let second = ProjectCommandRunResponse {
            id: "run-2".to_string(),
            command: "cargo test".to_string(),
            working_dir: "src-tauri".to_string(),
            exit_code: Some(101),
            success: false,
            stdout: String::new(),
            stderr: "failed".to_string(),
            duration_ms: 34,
            timed_out: false,
            stdout_truncated: false,
            stderr_truncated: false,
            created_at: "2026-06-09T02:00:00Z".to_string(),
        };

        store.append_run(&first).unwrap();
        store.append_run(&second).unwrap();

        let runs = store.list_runs(Some(1)).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, "run-2");
        assert_eq!(runs[0].exit_code, Some(101));
        assert!(!runs[0].success);
    }
}
