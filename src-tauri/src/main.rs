//! 多 Agent 协同智能体 — 应用入口
//!
//! 负责初始化 Tauri 桌面应用，启动 Agent 运行时，注册 Tauri 命令，
//! 并管理全局共享状态（Orchestrator、MessageBus 等）。

mod agent;
mod bus;
mod commands;
mod error;
mod llm;
mod memory;
mod orchestrator;
mod project;
mod task;
mod tool;
mod workspace;

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::bus::MessageBus;
use crate::orchestrator::Orchestrator;

/// 全局应用状态
pub struct AppState {
    pub bus: Arc<MessageBus>,
    pub orchestrator: Arc<Mutex<Orchestrator>>,
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
    let orchestrator = Arc::new(Mutex::new(Orchestrator::new(bus.clone())));

    {
        let mut orch = orchestrator.lock().await;
        orch.register_builtin_agents().await;
    }
    tracing::info!("Agent 运行时初始化完成");

    tauri::Builder::default()
        .manage(AppState { bus, orchestrator })
        .invoke_handler(tauri::generate_handler![
            commands::send_message,
            commands::create_task,
            commands::get_agent_status,
            commands::list_agents,
            commands::get_task_result,
            commands::get_task,
            commands::list_tasks,
            commands::get_task_events,
            commands::get_project_snapshot,
            commands::list_project_files,
            commands::read_project_file,
            commands::search_project_text,
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
