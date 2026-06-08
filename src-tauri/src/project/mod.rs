//! Project scanning and snapshot generation.

use crate::error::AgentError;
use crate::workspace::{self, workspace_root, SearchMatch};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// High-level project snapshot used by Planner and frontend views.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshot {
    pub root: String,
    pub name: String,
    pub tech_stack: Vec<String>,
    pub manifests: Vec<ProjectManifest>,
    pub important_files: Vec<ProjectImportantFile>,
    pub recommended_commands: Vec<ProjectCommand>,
    pub generated_at: String,
}

/// A discovered manifest file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectManifest {
    pub path: String,
    pub kind: String,
    pub summary: String,
}

/// An important project file or directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectImportantFile {
    pub path: String,
    pub kind: String,
    pub description: String,
}

/// A recommended low-risk command for validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommand {
    pub label: String,
    pub command: String,
    pub working_dir: String,
    pub kind: String,
}

/// Project context injected into Planner messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPlanningContext {
    pub snapshot: ProjectSnapshot,
    pub search_query: String,
    pub search_matches: Vec<SearchMatch>,
    pub search_error: Option<String>,
}

/// Generate a fresh project snapshot for the current workspace.
pub fn scan_project() -> Result<ProjectSnapshot, AgentError> {
    let root = workspace_root();
    let root_name = root
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "workspace".to_string());

    let mut snapshot = ProjectSnapshot {
        root: root.display().to_string(),
        name: root_name,
        tech_stack: Vec::new(),
        manifests: Vec::new(),
        important_files: Vec::new(),
        recommended_commands: Vec::new(),
        generated_at: chrono::Utc::now().to_rfc3339(),
    };

    detect_rust_tauri(&root, &mut snapshot)?;
    detect_web_stack(&root, &mut snapshot)?;
    detect_docs(&root, &mut snapshot);
    snapshot.tech_stack.sort();
    snapshot.tech_stack.dedup();

    Ok(snapshot)
}

/// Build a lightweight project context for a user goal.
pub fn build_planning_context(goal: &str) -> Result<ProjectPlanningContext, AgentError> {
    let snapshot = scan_project()?;
    let search_query = select_search_query(goal);
    let (search_matches, search_error) = match workspace::search_text(&search_query, Some(8)) {
        Ok(response) => (response.matches, None),
        Err(err) => (Vec::new(), Some(format!("{}", err))),
    };

    Ok(ProjectPlanningContext {
        snapshot,
        search_query,
        search_matches,
        search_error,
    })
}

fn detect_rust_tauri(root: &Path, snapshot: &mut ProjectSnapshot) -> Result<(), AgentError> {
    let cargo_path = root.join("src-tauri").join("Cargo.toml");
    if cargo_path.exists() {
        snapshot.tech_stack.push("Rust".to_string());
        let summary = summarize_cargo_manifest(&cargo_path)?;
        snapshot.manifests.push(ProjectManifest {
            path: "src-tauri/Cargo.toml".to_string(),
            kind: "Cargo manifest".to_string(),
            summary,
        });
        snapshot.important_files.push(ProjectImportantFile {
            path: "src-tauri/src/main.rs".to_string(),
            kind: "Rust entrypoint".to_string(),
            description:
                "Tauri app startup, Agent runtime initialization, IPC command registration."
                    .to_string(),
        });
        snapshot.recommended_commands.push(ProjectCommand {
            label: "Rust tests".to_string(),
            command: "cargo test".to_string(),
            working_dir: "src-tauri".to_string(),
            kind: "test".to_string(),
        });
        snapshot.recommended_commands.push(ProjectCommand {
            label: "Rust check".to_string(),
            command: "cargo check".to_string(),
            working_dir: "src-tauri".to_string(),
            kind: "check".to_string(),
        });
    }

    let tauri_conf = root.join("src-tauri").join("tauri.conf.json");
    if tauri_conf.exists() {
        snapshot.tech_stack.push("Tauri v2".to_string());
        let summary = summarize_tauri_config(&tauri_conf)?;
        snapshot.manifests.push(ProjectManifest {
            path: "src-tauri/tauri.conf.json".to_string(),
            kind: "Tauri config".to_string(),
            summary,
        });
    }

    Ok(())
}

fn detect_web_stack(root: &Path, snapshot: &mut ProjectSnapshot) -> Result<(), AgentError> {
    let package_path = root.join("src-web").join("package.json");
    if package_path.exists() {
        let summary = summarize_package_manifest(&package_path, snapshot)?;
        snapshot.manifests.push(ProjectManifest {
            path: "src-web/package.json".to_string(),
            kind: "Node package".to_string(),
            summary,
        });
        snapshot.important_files.push(ProjectImportantFile {
            path: "src-web/src/App.tsx".to_string(),
            kind: "React shell".to_string(),
            description: "Main frontend route shell and sidebar navigation.".to_string(),
        });
        snapshot.important_files.push(ProjectImportantFile {
            path: "src-web/src/store/useAgentStore.ts".to_string(),
            kind: "Frontend state".to_string(),
            description: "Zustand store for chat, agents, tasks, and app settings.".to_string(),
        });
        snapshot.recommended_commands.push(ProjectCommand {
            label: "Frontend tests".to_string(),
            command: "npm test -- --run".to_string(),
            working_dir: "src-web".to_string(),
            kind: "test".to_string(),
        });
        snapshot.recommended_commands.push(ProjectCommand {
            label: "Frontend build".to_string(),
            command: "npm run build".to_string(),
            working_dir: "src-web".to_string(),
            kind: "build".to_string(),
        });
    }

    if root.join("src-web").join("vite.config.ts").exists() {
        snapshot.tech_stack.push("Vite".to_string());
        snapshot.important_files.push(ProjectImportantFile {
            path: "src-web/vite.config.ts".to_string(),
            kind: "Vite config".to_string(),
            description: "Frontend dev server, build, aliases, and Vitest configuration."
                .to_string(),
        });
    }

    if root.join("src-web").join("tsconfig.json").exists() {
        snapshot.tech_stack.push("TypeScript".to_string());
    }

    Ok(())
}

fn detect_docs(root: &Path, snapshot: &mut ProjectSnapshot) {
    for (path, description) in [
        (
            "README.md",
            "Project overview, architecture, running instructions, and current capabilities.",
        ),
        (
            "docs/TECHNICAL_DOCUMENTATION.md",
            "Current technical documentation and known limitations.",
        ),
        (
            "docs/SELF_EVOLVING_AGENT_ROADMAP.md",
            "Roadmap for the self-evolving software-engineering Agent system.",
        ),
    ] {
        if root.join(path).exists() {
            snapshot.important_files.push(ProjectImportantFile {
                path: path.to_string(),
                kind: "Documentation".to_string(),
                description: description.to_string(),
            });
        }
    }
}

fn summarize_cargo_manifest(path: &Path) -> Result<String, AgentError> {
    let value = read_toml(path)?;
    let package = value.get("package");
    let name = package
        .and_then(|pkg| pkg.get("name"))
        .and_then(|name| name.as_str())
        .unwrap_or("unknown");
    let version = package
        .and_then(|pkg| pkg.get("version"))
        .and_then(|version| version.as_str())
        .unwrap_or("unknown");
    let deps = value
        .get("dependencies")
        .and_then(|deps| deps.as_table())
        .map(|deps| deps.len())
        .unwrap_or(0);

    Ok(format!("package {name} {version}, {deps} dependencies"))
}

fn summarize_tauri_config(path: &Path) -> Result<String, AgentError> {
    let value = read_json(path)?;
    let product_name = value
        .get("productName")
        .and_then(|name| name.as_str())
        .unwrap_or("unknown");
    let identifier = value
        .get("identifier")
        .and_then(|id| id.as_str())
        .unwrap_or("unknown");
    Ok(format!("product {product_name}, identifier {identifier}"))
}

fn summarize_package_manifest(
    path: &Path,
    snapshot: &mut ProjectSnapshot,
) -> Result<String, AgentError> {
    let value = read_json(path)?;
    let name = value
        .get("name")
        .and_then(|name| name.as_str())
        .unwrap_or("unknown");
    let version = value
        .get("version")
        .and_then(|version| version.as_str())
        .unwrap_or("unknown");

    let dependencies = value
        .get("dependencies")
        .and_then(|deps| deps.as_object())
        .cloned()
        .unwrap_or_default();
    if dependencies.contains_key("react") {
        snapshot.tech_stack.push("React".to_string());
    }
    if dependencies.contains_key("zustand") {
        snapshot.tech_stack.push("Zustand".to_string());
    }
    if dependencies.contains_key("@tauri-apps/api") {
        snapshot.tech_stack.push("Tauri JS API".to_string());
    }

    let dev_dependencies = value
        .get("devDependencies")
        .and_then(|deps| deps.as_object())
        .cloned()
        .unwrap_or_default();
    if dev_dependencies.contains_key("vitest") {
        snapshot.tech_stack.push("Vitest".to_string());
    }
    if dev_dependencies.contains_key("tailwindcss") {
        snapshot.tech_stack.push("Tailwind CSS".to_string());
    }

    Ok(format!(
        "package {name} {version}, {} dependencies, {} devDependencies",
        dependencies.len(),
        dev_dependencies.len()
    ))
}

fn read_json(path: &Path) -> Result<serde_json::Value, AgentError> {
    let raw = fs::read_to_string(path).map_err(|e| {
        AgentError::Internal(format!("读取 JSON 文件失败 [{}]: {e}", path.display()))
    })?;
    serde_json::from_str(&raw)
        .map_err(|e| AgentError::Internal(format!("解析 JSON 文件失败 [{}]: {e}", path.display())))
}

fn read_toml(path: &Path) -> Result<toml::Value, AgentError> {
    let raw = fs::read_to_string(path).map_err(|e| {
        AgentError::Internal(format!("读取 TOML 文件失败 [{}]: {e}", path.display()))
    })?;
    raw.parse::<toml::Value>()
        .map_err(|e| AgentError::Internal(format!("解析 TOML 文件失败 [{}]: {e}", path.display())))
}

fn select_search_query(goal: &str) -> String {
    let lower = goal.to_lowercase();
    let mappings = [
        (["任务", "task", "看板"].as_slice(), "Task"),
        (["项目", "project"].as_slice(), "Project"),
        (["文件", "读取", "file"].as_slice(), "readProjectFile"),
        (["搜索", "查找", "search"].as_slice(), "search"),
        (["agent", "智能体"].as_slice(), "Agent"),
        (["设置", "settings"].as_slice(), "SettingsPanel"),
        (["对话", "聊天", "chat"].as_slice(), "ChatWindow"),
        (["规划", "planner"].as_slice(), "Planner"),
        (["调度", "orchestrator"].as_slice(), "Orchestrator"),
        (["测试", "test"].as_slice(), "test"),
        (["前端", "react", "页面"].as_slice(), "React"),
        (["后端", "rust"].as_slice(), "Rust"),
    ];

    for (needles, query) in mappings {
        if needles.iter().any(|needle| lower.contains(needle)) {
            return query.to_string();
        }
    }

    lower
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .find(|token| token.len() >= 3)
        .map(str::to_string)
        .unwrap_or_else(|| "Agent".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_detects_current_project_stack() {
        let snapshot = scan_project().unwrap();
        assert!(snapshot.tech_stack.contains(&"Rust".to_string()));
        assert!(snapshot.tech_stack.contains(&"Tauri v2".to_string()));
        assert!(snapshot.tech_stack.contains(&"React".to_string()));
        assert!(snapshot.tech_stack.contains(&"Vite".to_string()));
    }

    #[test]
    fn scan_includes_recommended_commands() {
        let snapshot = scan_project().unwrap();
        assert!(snapshot
            .recommended_commands
            .iter()
            .any(|command| command.command == "cargo test"));
        assert!(snapshot
            .recommended_commands
            .iter()
            .any(|command| command.command == "npm test -- --run"));
    }

    #[test]
    fn planning_context_includes_snapshot_and_search() {
        let context = build_planning_context("优化项目任务看板").unwrap();
        assert!(context.snapshot.tech_stack.contains(&"Rust".to_string()));
        assert_eq!(context.search_query, "Task");
    }
}
