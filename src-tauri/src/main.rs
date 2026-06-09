//! 多 Agent 协同智能体 — 应用入口
//!
//! 负责初始化 Tauri 桌面应用，启动 Agent 运行时，注册 Tauri 命令，
//! 并管理全局共享状态（Orchestrator、MessageBus 等）。

mod agent;
mod approval;
mod bus;
mod chat;
mod commands;
mod error;
mod llm;
mod memory;
mod orchestrator;
mod project;
mod runtime;
mod task;
mod tool;
mod workspace;

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::approval::ApprovalStore;
use crate::bus::MessageBus;
use crate::chat::ChatStore;
use crate::orchestrator::Orchestrator;
use crate::runtime::CommandRunStore;
use crate::workspace::PatchProposalStore;

const DEFAULT_TASK_DB_PATH: &str = "rust-mutil-agent-tasks.sqlite3";
const DEFAULT_CHAT_DB_PATH: &str = "rust-mutil-agent-chat.sqlite3";
const DEFAULT_COMMAND_DB_PATH: &str = "rust-mutil-agent-commands.sqlite3";
const DEFAULT_APPROVAL_DB_PATH: &str = "rust-mutil-agent-approvals.sqlite3";
const DEFAULT_PATCH_DB_PATH: &str = "rust-mutil-agent-patches.sqlite3";

/// 全局应用状态
pub struct AppState {
    pub bus: Arc<MessageBus>,
    pub orchestrator: Arc<Mutex<Orchestrator>>,
    pub chat_store: Arc<ChatStore>,
    pub command_store: Arc<CommandRunStore>,
    pub approval_store: Arc<ApprovalStore>,
    pub patch_store: Arc<PatchProposalStore>,
}

/// Tauri 应用主入口
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("正在启动多 Agent 协同智能体系统...");

    if let Err(e) = dotenvy::dotenv() {
        tracing::warn!("未找到 .env 文件，使用系统环境变量: {e}");
    }

    let bus = Arc::new(MessageBus::new());
    let chat_db_path =
        std::env::var("CHAT_DB_PATH").unwrap_or_else(|_| DEFAULT_CHAT_DB_PATH.to_string());
    let chat_store = match ChatStore::open(&chat_db_path) {
        Ok(store) => {
            tracing::info!("聊天历史数据库已启用: {}", chat_db_path);
            Arc::new(store)
        }
        Err(err) => {
            tracing::warn!("聊天历史数据库初始化失败，将使用内存聊天历史: {}", err);
            Arc::new(ChatStore::open(":memory:").expect("内存聊天历史初始化失败"))
        }
    };
    let task_db_path =
        std::env::var("TASK_DB_PATH").unwrap_or_else(|_| DEFAULT_TASK_DB_PATH.to_string());
    let orchestrator = match Orchestrator::with_task_store(bus.clone(), &task_db_path) {
        Ok(orch) => {
            tracing::info!("任务持久化数据库已启用: {}", task_db_path);
            Arc::new(Mutex::new(orch))
        }
        Err(err) => {
            tracing::warn!("任务持久化数据库初始化失败，将使用内存任务运行时: {}", err);
            Arc::new(Mutex::new(Orchestrator::new(bus.clone())))
        }
    };
    let command_db_path =
        std::env::var("COMMAND_DB_PATH").unwrap_or_else(|_| DEFAULT_COMMAND_DB_PATH.to_string());
    let command_store = match CommandRunStore::open(&command_db_path) {
        Ok(store) => {
            tracing::info!("命令运行审计数据库已启用: {}", command_db_path);
            Arc::new(store)
        }
        Err(err) => {
            tracing::warn!("命令运行审计数据库初始化失败，将使用内存审计记录: {}", err);
            Arc::new(CommandRunStore::open(":memory:").expect("内存命令审计初始化失败"))
        }
    };
    let approval_db_path =
        std::env::var("APPROVAL_DB_PATH").unwrap_or_else(|_| DEFAULT_APPROVAL_DB_PATH.to_string());
    let approval_store = match ApprovalStore::open(&approval_db_path) {
        Ok(store) => {
            tracing::info!("审批请求数据库已启用: {}", approval_db_path);
            Arc::new(store)
        }
        Err(err) => {
            tracing::warn!("审批请求数据库初始化失败，将使用内存审批记录: {}", err);
            Arc::new(ApprovalStore::open(":memory:").expect("内存审批记录初始化失败"))
        }
    };
    let patch_db_path =
        std::env::var("PATCH_DB_PATH").unwrap_or_else(|_| DEFAULT_PATCH_DB_PATH.to_string());
    let patch_store = match PatchProposalStore::open(&patch_db_path) {
        Ok(store) => {
            tracing::info!("补丁提案数据库已启用: {}", patch_db_path);
            Arc::new(store)
        }
        Err(err) => {
            tracing::warn!("补丁提案数据库初始化失败，将使用内存补丁记录: {}", err);
            Arc::new(PatchProposalStore::open(":memory:").expect("内存补丁记录初始化失败"))
        }
    };

    {
        let mut orch = orchestrator.lock().await;
        orch.register_builtin_agents().await;
    }
    tracing::info!("Agent 运行时初始化完成");

    tauri::Builder::default()
        .manage(AppState {
            bus,
            orchestrator,
            chat_store,
            command_store,
            approval_store,
            patch_store,
        })
        .invoke_handler(tauri::generate_handler![
            commands::send_message,
            commands::create_task,
            commands::get_agent_status,
            commands::list_agents,
            commands::get_task_result,
            commands::get_task,
            commands::list_tasks,
            commands::get_task_events,
            commands::cancel_task,
            commands::retry_task,
            commands::skip_task_step,
            commands::get_project_snapshot,
            commands::list_project_files,
            commands::read_project_file,
            commands::search_project_text,
            commands::run_project_command,
            commands::request_project_command_approval,
            commands::request_tool_action_approval,
            commands::run_approved_project_command,
            commands::list_project_command_runs,
            commands::create_patch_proposal,
            commands::list_patch_proposals,
            commands::get_patch_proposal,
            commands::apply_approved_patch,
            commands::revert_applied_patch,
            commands::list_approval_requests,
            commands::approve_action,
            commands::health_check,
            commands::get_history,
            commands::clear_history,
        ])
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_process::init())
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用时发生错误");
}
