//! Workspace-safe file listing, reading, and text search.

pub mod patch;

use crate::error::AgentError;
pub use patch::{
    apply_patch_proposal, build_patch_proposal, CreatePatchProposalRequest, PatchApplyResult,
    PatchProposal, PatchProposalListResponse, PatchProposalStatus, PatchProposalStore,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

const MAX_READ_BYTES: u64 = 512 * 1024;
const MAX_SEARCH_BYTES: u64 = 256 * 1024;
const DEFAULT_FILE_LIMIT: usize = 500;
const DEFAULT_SEARCH_LIMIT: usize = 50;

/// A file or directory inside the workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub extension: Option<String>,
    pub size_bytes: u64,
    pub modified_at: Option<String>,
}

/// Text file contents returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileReadResponse {
    pub path: String,
    pub content: String,
    pub size_bytes: u64,
}

/// One text-search match.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchMatch {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub preview: String,
}

/// Text-search response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub query: String,
    pub matches: Vec<SearchMatch>,
    pub truncated: bool,
}

/// Return the repository workspace root.
pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

/// List regular workspace files and directories in a flat, depth-first order.
pub fn list_files(max_files: Option<usize>) -> Result<Vec<WorkspaceEntry>, AgentError> {
    let root = workspace_root();
    let limit = max_files.unwrap_or(DEFAULT_FILE_LIMIT).max(1);
    let mut entries = Vec::new();
    collect_entries(&root, &root, limit, &mut entries)?;
    Ok(entries)
}

/// Read a UTF-8 text file inside the workspace.
pub fn read_file(path: &str) -> Result<FileReadResponse, AgentError> {
    let root = workspace_root();
    let file_path = resolve_existing_path(&root, path)?;
    let metadata = fs::metadata(&file_path)
        .map_err(|e| AgentError::Internal(format!("读取文件元数据失败 [{}]: {e}", path)))?;

    if !metadata.is_file() {
        return Err(AgentError::MessageFormat(format!(
            "[{}] 不是可读取的文件",
            path
        )));
    }
    if metadata.len() > MAX_READ_BYTES {
        return Err(AgentError::MessageFormat(format!(
            "文件 [{}] 超过读取限制 {} bytes",
            path, MAX_READ_BYTES
        )));
    }
    if is_blocked_file(&file_path) {
        return Err(AgentError::MessageFormat(format!(
            "文件 [{}] 可能包含敏感信息，已拒绝读取",
            path
        )));
    }

    let content = fs::read_to_string(&file_path)
        .map_err(|e| AgentError::Internal(format!("读取文本文件失败 [{}]: {e}", path)))?;
    Ok(FileReadResponse {
        path: relative_path(&root, &file_path),
        content,
        size_bytes: metadata.len(),
    })
}

/// Search UTF-8 text files inside the workspace.
pub fn search_text(query: &str, max_results: Option<usize>) -> Result<SearchResponse, AgentError> {
    let query = query.trim();
    if query.is_empty() {
        return Err(AgentError::MessageFormat("搜索关键词不能为空".to_string()));
    }

    let root = workspace_root();
    let limit = max_results.unwrap_or(DEFAULT_SEARCH_LIMIT).max(1);
    let needle = query.to_lowercase();
    let mut matches = Vec::new();
    let mut truncated = false;

    search_dir(&root, &root, &needle, limit, &mut matches, &mut truncated)?;

    Ok(SearchResponse {
        query: query.to_string(),
        matches,
        truncated,
    })
}

fn collect_entries(
    root: &Path,
    dir: &Path,
    limit: usize,
    entries: &mut Vec<WorkspaceEntry>,
) -> Result<(), AgentError> {
    if entries.len() >= limit {
        return Ok(());
    }

    let mut children = fs::read_dir(dir)
        .map_err(|e| AgentError::Internal(format!("列出目录失败 [{}]: {e}", dir.display())))?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    children.sort_by_key(|entry| entry.path());

    for child in children {
        if entries.len() >= limit {
            break;
        }

        let path = child.path();
        if should_skip_path(&path) {
            continue;
        }

        let metadata = match child.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };

        entries.push(WorkspaceEntry {
            path: relative_path(root, &path),
            name: child.file_name().to_string_lossy().to_string(),
            is_dir: metadata.is_dir(),
            extension: path
                .extension()
                .map(|value| value.to_string_lossy().to_string()),
            size_bytes: if metadata.is_file() {
                metadata.len()
            } else {
                0
            },
            modified_at: metadata.modified().ok().map(system_time_to_iso),
        });

        if metadata.is_dir() {
            collect_entries(root, &path, limit, entries)?;
        }
    }

    Ok(())
}

fn search_dir(
    root: &Path,
    dir: &Path,
    needle: &str,
    limit: usize,
    matches: &mut Vec<SearchMatch>,
    truncated: &mut bool,
) -> Result<(), AgentError> {
    if matches.len() >= limit {
        *truncated = true;
        return Ok(());
    }

    let mut children = fs::read_dir(dir)
        .map_err(|e| AgentError::Internal(format!("搜索目录失败 [{}]: {e}", dir.display())))?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    children.sort_by_key(|entry| entry.path());

    for child in children {
        if matches.len() >= limit {
            *truncated = true;
            break;
        }

        let path = child.path();
        if should_skip_path(&path) || is_blocked_file(&path) {
            continue;
        }

        let Ok(metadata) = child.metadata() else {
            continue;
        };

        if metadata.is_dir() {
            search_dir(root, &path, needle, limit, matches, truncated)?;
        } else if metadata.is_file() && is_searchable_text_file(&path, metadata.len()) {
            search_file(root, &path, needle, limit, matches)?;
        }
    }

    Ok(())
}

fn search_file(
    root: &Path,
    path: &Path,
    needle: &str,
    limit: usize,
    matches: &mut Vec<SearchMatch>,
) -> Result<(), AgentError> {
    let Ok(content) = fs::read_to_string(path) else {
        return Ok(());
    };

    for (line_index, line) in content.lines().enumerate() {
        if matches.len() >= limit {
            break;
        }

        let haystack = line.to_lowercase();
        if let Some(index) = haystack.find(needle) {
            matches.push(SearchMatch {
                path: relative_path(root, path),
                line: line_index + 1,
                column: line[..index].chars().count() + 1,
                preview: trim_preview(line),
            });
        }
    }

    Ok(())
}

fn resolve_existing_path(root: &Path, rel_path: &str) -> Result<PathBuf, AgentError> {
    let rel = rel_path.trim();
    if rel.is_empty() {
        return Err(AgentError::MessageFormat("文件路径不能为空".to_string()));
    }

    let candidate = Path::new(rel);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(AgentError::MessageFormat(format!(
            "路径 [{}] 不允许访问 workspace 外部",
            rel_path
        )));
    }

    let canonical_root = root.canonicalize().map_err(|e| {
        AgentError::Internal(format!(
            "解析 workspace root 失败 [{}]: {e}",
            root.display()
        ))
    })?;
    let canonical_file = canonical_root.join(candidate).canonicalize().map_err(|e| {
        AgentError::MessageFormat(format!("文件 [{}] 不存在或不可访问: {e}", rel_path))
    })?;

    if !canonical_file.starts_with(&canonical_root) {
        return Err(AgentError::MessageFormat(format!(
            "路径 [{}] 不允许访问 workspace 外部",
            rel_path
        )));
    }

    Ok(canonical_file)
}

fn should_skip_path(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        matches!(
            name.as_ref(),
            ".git" | "node_modules" | "target" | "dist" | "build" | ".next" | ".tauri" | ".vite"
        )
    })
}

fn is_blocked_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|value| value.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    name == ".env"
        || name.ends_with(".pem")
        || name.ends_with(".key")
        || name.ends_with(".p12")
        || name.ends_with(".pfx")
        || name.contains("secret")
        || name.contains("token")
}

fn is_searchable_text_file(path: &Path, size_bytes: u64) -> bool {
    if size_bytes > MAX_SEARCH_BYTES {
        return false;
    }

    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref(),
        Some(
            "rs" | "ts"
                | "tsx"
                | "js"
                | "jsx"
                | "json"
                | "toml"
                | "md"
                | "css"
                | "html"
                | "yml"
                | "yaml"
                | "txt"
        )
    )
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn trim_preview(line: &str) -> String {
    let trimmed = line.trim();
    let mut chars = trimmed.chars();
    let mut preview = chars.by_ref().take(180).collect::<String>();
    if chars.next().is_some() {
        preview.push_str("...");
    }
    preview
}

fn system_time_to_iso(time: SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Utc> = time.into();
    datetime.to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_root_points_to_project_root() {
        let root = workspace_root();
        assert!(root.join("README.md").exists());
        assert!(root.join("src-tauri").join("Cargo.toml").exists());
    }

    #[test]
    fn list_files_includes_project_manifests() {
        let files = list_files(Some(200)).unwrap();
        assert!(files.iter().any(|file| file.path == "README.md"));
        assert!(files.iter().any(|file| file.path == "src-tauri/Cargo.toml"));
    }

    #[test]
    fn read_file_rejects_parent_traversal() {
        let result = read_file("../Cargo.toml");
        assert!(result.is_err());
    }

    #[test]
    fn search_text_finds_project_name() {
        let result = search_text("多 Agent", Some(10)).unwrap();
        assert!(!result.matches.is_empty());
    }
}
