//! Tauri 命令 — 前端调用的 IPC 接口
//!
//! 定义所有暴露给前端（React）的 Tauri 命令，
//! 通过 invoke 机制实现前后端双向通信。

use crate::agent::action::RiskLevel;
use crate::agent::traits::Capability as AgentCapability;
use crate::approval::{
    parse_approval_status, ApprovalDecisionRequest, ApprovalListResponse, ApprovalRequest,
    ApprovalStatus, CreateApprovalRequest,
};
use crate::chat::ChatMessage as StoredChatMessage;
use crate::error::AgentError;
use crate::memory::{KnowledgeBase, KnowledgeItem};
use crate::project::ProjectSnapshot;
use crate::runtime::{
    CommandRunStore, ProjectCommandInspection, ProjectCommandRunListResponse,
    ProjectCommandRunRequest, ProjectCommandRunResponse, ToolInvocationListResponse,
};
use crate::task::{Task, TaskEvent};
use crate::workspace::{
    CreatePatchProposalRequest, FileReadResponse, PatchApplyResult, PatchAutoRollbackResult,
    PatchProposal, PatchProposalListResponse, PatchProposalStatus, PatchRevertResult,
    SearchResponse, WorkspaceEntry,
};
use crate::AppState;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

/// 前端传入的 LLM 设置。
///
/// API Key 只进入请求期临时上下文，不会写入任务上下文、事件或聊天历史。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendLlmSettingsRequest {
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub api_base_url: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

/// 前端发送消息请求。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    pub content: String,
    pub agent_id: Option<String>,
    pub route_mode: Option<String>,
    pub session_id: Option<String>,
    pub llm_settings: Option<FrontendLlmSettingsRequest>,
}

/// 创建软件工程任务请求。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskRequest {
    pub content: String,
    pub agent_id: Option<String>,
    pub llm_settings: Option<FrontendLlmSettingsRequest>,
}

/// 搜索项目文本请求。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchProjectTextRequest {
    pub query: String,
    pub max_results: Option<usize>,
}

/// 执行已审批项目命令请求。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunApprovedProjectCommandRequest {
    pub approval_id: String,
}

/// 创建通用工具动作审批请求。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolActionApprovalRequest {
    pub task_id: Option<String>,
    pub step_id: Option<String>,
    pub title: String,
    pub reason: String,
    pub action_type: String,
    #[serde(default)]
    pub action_payload: Value,
    pub risk: Option<RiskLevel>,
    pub requested_by: Option<String>,
}

/// 应用已审批补丁请求。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyApprovedPatchRequest {
    pub approval_id: String,
    #[serde(default)]
    pub auto_rollback_on_verification_failure: Option<bool>,
}

/// 回滚已应用补丁请求。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevertAppliedPatchRequest {
    pub patch_id: String,
}

/// 存储知识条目请求。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreKnowledgeRequest {
    pub title: String,
    pub content: String,
    pub source: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// 存储知识条目响应。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreKnowledgeResponse {
    pub id: String,
}

/// 搜索知识库请求。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchKnowledgeRequest {
    pub query: String,
    pub limit: Option<usize>,
}

/// 搜索知识库响应。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchKnowledgeResponse {
    pub query: String,
    pub items: Vec<KnowledgeItem>,
}

/// 创建补丁提案响应。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePatchProposalResponse {
    pub proposal: PatchProposal,
    pub approval: ApprovalRequest,
}

/// 结构化 API 错误。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub detail: Option<String>,
    pub retryable: bool,
    pub request_id: String,
}

impl ApiError {
    fn new(code: &str, message: &str, detail: Option<String>, retryable: bool) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
            detail,
            retryable,
            request_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    fn invalid_argument(message: &str) -> Self {
        Self::new("INVALID_ARGUMENT", message, None, false)
    }

    fn agent_not_found(agent_id: &str) -> Self {
        Self::new(
            "AGENT_NOT_FOUND",
            "找不到指定的 AI 成员，请刷新团队成员列表，或切换为“自动分配”后重试。",
            Some(format!("Agent [{}] 未注册", agent_id)),
            true,
        )
    }

    fn agent_not_selectable(meta: &AgentMeta) -> Self {
        Self::new(
            "AGENT_NOT_SELECTABLE",
            "这个 AI 成员主要作为内部能力使用，暂不支持直接对话。请切换为“自动分配”或选择协调员。",
            Some(format!(
                "Agent [{}] is internal/selectable=false",
                meta.runtime_name
            )),
            false,
        )
    }

    fn route_failed(detail: String) -> Self {
        Self::new(
            "ROUTE_FAILED",
            "消息已收到，但交给 AI 成员处理时失败。请稍后重试，或切换为“自动分配”。",
            Some(detail),
            true,
        )
    }

    fn workspace_failed(detail: String) -> Self {
        Self::new(
            "WORKSPACE_ERROR",
            "读取项目工作区时失败。请检查路径是否存在，或稍后重试。",
            Some(detail),
            false,
        )
    }

    fn history_failed(detail: String) -> Self {
        Self::new(
            "HISTORY_ERROR",
            "读写聊天历史时失败。当前操作没有完成，请稍后重试。",
            Some(detail),
            true,
        )
    }

    fn knowledge_failed(detail: String) -> Self {
        Self::new(
            "KNOWLEDGE_ERROR",
            "读写长期记忆时失败。请稍后重试，或检查记忆数据库路径。",
            Some(detail),
            true,
        )
    }

    fn command_failed(detail: String) -> Self {
        Self::new(
            "COMMAND_ERROR",
            "运行项目命令时失败。请检查本机开发环境是否可用。",
            Some(detail),
            true,
        )
    }

    fn tool_audit_failed(detail: String) -> Self {
        Self::new(
            "TOOL_AUDIT_ERROR",
            "读取工具调用审计时失败。请刷新项目面板后重试。",
            Some(detail),
            true,
        )
    }

    fn approval_failed(detail: String) -> Self {
        Self::new(
            "APPROVAL_ERROR",
            "处理审批请求时失败。请刷新审批列表后重试。",
            Some(detail),
            true,
        )
    }

    fn patch_failed(detail: String) -> Self {
        Self::new(
            "PATCH_ERROR",
            "处理补丁提案时失败。请刷新项目状态后重试。",
            Some(detail),
            true,
        )
    }
}

/// 前端消息结构。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageResponse {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
    pub sender_name: Option<String>,
}

/// 路由信息，告诉前端“这条消息到底交给谁处理”。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteInfoResponse {
    pub mode: String,
    pub requested_agent_id: Option<String>,
    pub target_agent_id: String,
    pub target_runtime_name: String,
    pub target_display_name: String,
    pub fallback: bool,
    pub reason: String,
}

/// 发送消息响应。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageResponse {
    pub message: ChatMessageResponse,
    pub handled_by: String,
    pub task_id: String,
    pub status: String,
    pub route: RouteInfoResponse,
    pub warnings: Vec<String>,
}

/// 创建任务响应。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskResponse {
    pub task_id: String,
    pub task: Option<Task>,
}

/// 取消任务请求。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelTaskRequest {
    pub task_id: String,
    pub reason: Option<String>,
}

/// 取消任务响应。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelTaskResponse {
    pub task_id: String,
    pub task: Option<Task>,
}

/// 重试任务请求。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryTaskRequest {
    pub task_id: String,
    pub reason: Option<String>,
}

/// 重试任务响应。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryTaskResponse {
    pub task_id: String,
    pub task: Option<Task>,
}

/// 跳过任务步骤请求。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkipTaskStepRequest {
    pub task_id: String,
    pub step_id: String,
    pub reason: Option<String>,
}

/// 跳过任务步骤响应。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkipTaskStepResponse {
    pub task_id: String,
    pub step_id: String,
    pub task: Option<Task>,
}

/// 任务列表响应。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskListResponse {
    pub tasks: Vec<Task>,
}

/// 项目文件列表响应。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileListResponse {
    pub files: Vec<WorkspaceEntry>,
}

/// 前端 Agent 能力结构。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityResponse {
    pub name: String,
    pub description: String,
    pub available: bool,
}

/// 前端 Agent 状态结构。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatusResponse {
    pub id: String,
    pub runtime_name: String,
    pub name: String,
    pub role: String,
    pub role_label: String,
    pub description: String,
    pub online: bool,
    pub status: String,
    pub status_label: String,
    pub current_task: Option<String>,
    pub capabilities: Vec<CapabilityResponse>,
    pub last_active: String,
    pub selectable: bool,
    pub recommended: bool,
    pub is_internal: bool,
}

/// Agent 列表响应。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentListResponse {
    pub agents: Vec<AgentStatusResponse>,
}

/// 健康检查响应。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckResponse {
    pub healthy: bool,
    pub version: String,
    pub agent_count: usize,
}

#[derive(Debug, Clone, Copy)]
struct AgentMeta {
    id: &'static str,
    runtime_name: &'static str,
    display_name: &'static str,
    role: &'static str,
    role_label: &'static str,
    description: &'static str,
    selectable: bool,
    recommended: bool,
    is_internal: bool,
}

const AGENT_CATALOG: &[AgentMeta] = &[
    AgentMeta {
        id: "coordinator",
        runtime_name: "Planner",
        display_name: "协调员/总控",
        role: "planner",
        role_label: "任务规划与调度",
        description: "理解你的需求，拆解任务，并安排合适的 AI 成员协同处理。",
        selectable: true,
        recommended: true,
        is_internal: false,
    },
    AgentMeta {
        id: "executor",
        runtime_name: "Executor",
        display_name: "执行工程师",
        role: "executor",
        role_label: "任务执行",
        description: "负责执行明确任务，例如代码处理、命令运行、文件操作和问题修复。",
        selectable: true,
        recommended: false,
        is_internal: false,
    },
    AgentMeta {
        id: "memory",
        runtime_name: "Memory",
        display_name: "记忆管理员",
        role: "memory",
        role_label: "记忆与检索",
        description: "负责保存、查找和整理历史上下文，通常由协调员自动调用。",
        selectable: false,
        recommended: false,
        is_internal: true,
    },
    AgentMeta {
        id: "tool",
        runtime_name: "Tool",
        display_name: "工具操作员",
        role: "tool",
        role_label: "工具调用",
        description: "负责调用工具、接口和系统能力，通常作为内部执行能力使用。",
        selectable: false,
        recommended: false,
        is_internal: true,
    },
    AgentMeta {
        id: "echo",
        runtime_name: "Echo",
        display_name: "回声测试员",
        role: "echo",
        role_label: "连接测试",
        description: "用于测试系统消息链路是否正常，不适合处理复杂正式任务。",
        selectable: true,
        recommended: false,
        is_internal: false,
    },
];

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn meta_for_runtime_name(name: &str) -> AgentMeta {
    AGENT_CATALOG
        .iter()
        .copied()
        .find(|meta| meta.runtime_name == name)
        .unwrap_or(AgentMeta {
            id: "custom",
            runtime_name: "Custom",
            display_name: "自定义成员",
            role: "custom",
            role_label: "自定义能力",
            description: "项目中注册的自定义 AI 成员。",
            selectable: true,
            recommended: false,
            is_internal: false,
        })
}

fn resolve_agent_meta(agent_id: Option<&str>) -> Result<(AgentMeta, String, bool), ApiError> {
    let raw = agent_id.map(str::trim).filter(|value| !value.is_empty());
    let Some(id) = raw else {
        let meta = meta_for_runtime_name("Planner");
        return Ok((
            meta,
            "未指定 AI 成员，默认由协调员/总控理解需求并自动分配。".to_string(),
            false,
        ));
    };

    let normalized = id.to_lowercase();
    let meta = AGENT_CATALOG
        .iter()
        .copied()
        .find(|meta| {
            meta.id.eq_ignore_ascii_case(id)
                || meta.runtime_name.eq_ignore_ascii_case(id)
                || meta.role.eq_ignore_ascii_case(id)
        })
        .ok_or_else(|| ApiError::agent_not_found(id))?;

    if !meta.selectable {
        return Err(ApiError::agent_not_selectable(&meta));
    }

    let reason = if normalized == "coordinator" || normalized == "planner" {
        "已交给协调员/总控处理，他会判断是否需要分派其他成员。".to_string()
    } else {
        format!("用户指定优先交给「{}」处理。", meta.display_name)
    };

    Ok((meta, reason, false))
}

fn capabilities_for_agent(name: &str) -> Vec<AgentCapability> {
    match name {
        "Echo" => vec![AgentCapability::chat()],
        "Planner" => vec![AgentCapability::planning(), AgentCapability::chat()],
        "Executor" => vec![
            AgentCapability::code_execution(),
            AgentCapability::tool_use(),
        ],
        "Memory" => vec![AgentCapability::memory(), AgentCapability::retrieval()],
        "Tool" => vec![AgentCapability::tool_use()],
        _ => vec![AgentCapability::chat()],
    }
}

fn capability_label(name: &str) -> String {
    match name {
        "chat" => "自然语言对话".to_string(),
        "planning" => "任务拆解".to_string(),
        "code_execution" => "代码/命令执行".to_string(),
        "retrieval" => "信息检索".to_string(),
        "tool_use" => "工具调用".to_string(),
        "memory" => "上下文记忆".to_string(),
        other => other.to_string(),
    }
}

fn build_agent_status(runtime_name: String) -> AgentStatusResponse {
    let meta = meta_for_runtime_name(&runtime_name);
    let capabilities = capabilities_for_agent(&runtime_name)
        .into_iter()
        .map(|cap| CapabilityResponse {
            name: capability_label(&cap.name),
            description: cap.description,
            available: true,
        })
        .collect();

    AgentStatusResponse {
        id: meta.id.to_string(),
        runtime_name,
        role: meta.role.to_string(),
        role_label: meta.role_label.to_string(),
        name: meta.display_name.to_string(),
        description: meta.description.to_string(),
        online: true,
        status: "idle".to_string(),
        status_label: "空闲".to_string(),
        current_task: None,
        capabilities,
        last_active: now_iso(),
        selectable: meta.selectable,
        recommended: meta.recommended,
        is_internal: meta.is_internal,
    }
}

fn clean_optional_string(value: Option<&String>) -> Option<String> {
    value
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
}

fn clean_required_text(value: &str, field_label: &str) -> Result<String, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ApiError::invalid_argument(&format!(
            "{field_label} 不能为空。"
        )));
    }
    Ok(value.to_string())
}

fn clean_tags(tags: Option<Vec<String>>) -> Vec<String> {
    tags.unwrap_or_default()
        .into_iter()
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty())
        .collect()
}

fn build_request_context(settings: Option<&FrontendLlmSettingsRequest>) -> Value {
    let Some(settings) = settings else {
        return Value::Null;
    };

    let has_api_key = settings
        .api_key
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());

    serde_json::json!({
        "frontendLlmSettings": {
            "model": clean_optional_string(settings.model.as_ref()),
            "apiBaseUrl": clean_optional_string(settings.api_base_url.as_ref()),
            "maxTokens": settings.max_tokens,
            "temperature": settings.temperature,
            "hasApiKey": has_api_key,
        }
    })
}

fn merge_context_values(base: Value, extra: Value) -> Value {
    match (base, extra) {
        (base, Value::Null) => base,
        (Value::Null, extra) => extra,
        (Value::Object(mut base), Value::Object(extra)) => {
            for (key, value) in extra {
                base.insert(key, value);
            }
            Value::Object(base)
        }
        (base, extra) => serde_json::json!({
            "base": base,
            "extra": extra,
        }),
    }
}

fn push_unique_query(queries: &mut Vec<String>, query: impl Into<String>) {
    let query = query.into();
    let query = query.trim();
    if !query.is_empty()
        && !queries
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(query))
    {
        queries.push(query.to_string());
    }
}

fn knowledge_search_queries(goal: &str) -> Vec<String> {
    let mut queries = Vec::new();
    push_unique_query(&mut queries, goal);
    push_unique_query(&mut queries, "ProjectFact");
    push_unique_query(&mut queries, "FailureCase");
    queries
}

fn collect_relevant_knowledge(
    knowledge_base: &KnowledgeBase,
    queries: &[String],
    limit: usize,
) -> Result<Vec<KnowledgeItem>, AgentError> {
    let mut items: Vec<KnowledgeItem> = Vec::new();
    for query in queries {
        for item in knowledge_base.search_knowledge(query, limit)? {
            if !items.iter().any(|existing| existing.id == item.id) {
                items.push(item);
            }
            if items.len() >= limit {
                return Ok(items);
            }
        }
    }
    Ok(items)
}

fn knowledge_item_context(item: KnowledgeItem) -> Value {
    serde_json::json!({
        "id": item.id,
        "title": item.title,
        "contentPreview": preview_for_knowledge(&item.content),
        "source": item.source,
        "tags": item.tags,
        "createdAt": item.created_at,
    })
}

fn build_knowledge_context(
    knowledge_base: &KnowledgeBase,
    goal: &str,
) -> Result<Value, AgentError> {
    const KNOWLEDGE_CONTEXT_LIMIT: usize = 6;

    let queries = knowledge_search_queries(goal);
    let items = collect_relevant_knowledge(knowledge_base, &queries, KNOWLEDGE_CONTEXT_LIMIT)?;
    if items.is_empty() {
        return Ok(Value::Null);
    }

    Ok(serde_json::json!({
        "knowledgeContext": {
            "query": goal.trim(),
            "queries": queries,
            "items": items.into_iter().map(knowledge_item_context).collect::<Vec<_>>(),
        }
    }))
}

fn build_request_context_with_knowledge(
    settings: Option<&FrontendLlmSettingsRequest>,
    knowledge_base: &KnowledgeBase,
    goal: &str,
) -> Value {
    let context = build_request_context(settings);
    match build_knowledge_context(knowledge_base, goal) {
        Ok(knowledge_context) => merge_context_values(context, knowledge_context),
        Err(err) => {
            tracing::warn!("检索规划长期经验失败: {}", err);
            context
        }
    }
}

fn build_transient_request_context(settings: Option<&FrontendLlmSettingsRequest>) -> Value {
    let Some(settings) = settings else {
        return Value::Null;
    };

    let api_key = clean_optional_string(settings.api_key.as_ref());
    let Some(api_key) = api_key else {
        return Value::Null;
    };

    serde_json::json!({
        "plannerLlmSettings": {
            "model": clean_optional_string(settings.model.as_ref()),
            "apiKey": api_key,
            "apiBaseUrl": clean_optional_string(settings.api_base_url.as_ref()),
            "maxTokens": settings.max_tokens,
            "temperature": settings.temperature,
        }
    })
}

fn clean_session_id(session_id: Option<&String>) -> String {
    session_id
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("default")
        .to_string()
}

fn stored_message_from_response(
    session_id: &str,
    message: &ChatMessageResponse,
) -> StoredChatMessage {
    StoredChatMessage {
        id: message.id.clone(),
        session_id: session_id.to_string(),
        role: message.role.clone(),
        content: message.content.clone(),
        timestamp: message.timestamp.clone(),
        sender_name: message.sender_name.clone(),
    }
}

fn response_from_stored_message(message: StoredChatMessage) -> ChatMessageResponse {
    ChatMessageResponse {
        id: message.id,
        role: message.role,
        content: message.content,
        timestamp: message.timestamp,
        sender_name: message.sender_name,
    }
}

/// 发送用户消息给 Agent 系统。
///
/// 前端调用：`invoke('send_message', { request: { content, agentId, llmSettings } })`
#[tauri::command]
pub async fn send_message(
    request: SendMessageRequest,
    state: State<'_, AppState>,
) -> Result<SendMessageResponse, ApiError> {
    let content = request.content.trim();
    if content.is_empty() {
        return Err(ApiError::invalid_argument("请输入要发送给 AI 成员的内容。"));
    }

    let session_id = clean_session_id(request.session_id.as_ref());
    let (target_meta, reason, fallback) = resolve_agent_meta(request.agent_id.as_deref())?;
    let msg_type = if target_meta.runtime_name == "Planner" {
        "plan_request"
    } else {
        "direct_message"
    };

    let request_context = if target_meta.runtime_name == "Planner" {
        build_request_context_with_knowledge(
            request.llm_settings.as_ref(),
            state.knowledge_base.as_ref(),
            content,
        )
    } else {
        build_request_context(request.llm_settings.as_ref())
    };
    let transient_context = build_transient_request_context(request.llm_settings.as_ref());
    let mut orch = state.orchestrator.lock().await;
    let task_id = orch
        .submit_task_to_agent_with_context_and_transient(
            target_meta.runtime_name,
            content,
            msg_type,
            request_context,
            transient_context,
        )
        .await
        .map_err(|e| ApiError::route_failed(format!("{}", e)))?;
    drop(orch);

    tracing::info!(
        "[Commands] 任务已提交: {}，处理 Agent: {}({})，route_mode={:?}, session_id={:?}",
        task_id,
        target_meta.display_name,
        target_meta.runtime_name,
        request.route_mode,
        request.session_id
    );

    let content = if target_meta.runtime_name == "Planner" {
        format!(
            "✅ 任务已收到\n\n已交给「{}」处理。他会先理解需求，再安排合适成员协同完成。\n\n任务编号：{}",
            target_meta.display_name, task_id
        )
    } else {
        format!(
            "✅ 任务已发送给「{}」\n\n系统正在处理，请稍等。\n\n任务编号：{}",
            target_meta.display_name, task_id
        )
    };

    let assistant_message = ChatMessageResponse {
        id: task_id.clone(),
        role: "assistant".to_string(),
        content,
        timestamp: now_iso(),
        sender_name: Some(target_meta.display_name.to_string()),
    };
    let user_message = StoredChatMessage {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.clone(),
        role: "user".to_string(),
        content: request.content.trim().to_string(),
        timestamp: now_iso(),
        sender_name: None,
    };
    let mut warnings = Vec::new();
    if let Err(err) = state.chat_store.append_message(&user_message) {
        warnings.push(format!("保存用户消息失败: {err}"));
    }
    if let Err(err) = state
        .chat_store
        .append_message(&stored_message_from_response(
            &session_id,
            &assistant_message,
        ))
    {
        warnings.push(format!("保存回复消息失败: {err}"));
    }

    Ok(SendMessageResponse {
        message: assistant_message,
        handled_by: target_meta.id.to_string(),
        task_id: task_id.clone(),
        status: "accepted".to_string(),
        route: RouteInfoResponse {
            mode: if target_meta.runtime_name == "Planner" {
                "auto".to_string()
            } else {
                "direct".to_string()
            },
            requested_agent_id: request.agent_id,
            target_agent_id: target_meta.id.to_string(),
            target_runtime_name: target_meta.runtime_name.to_string(),
            target_display_name: target_meta.display_name.to_string(),
            fallback,
            reason,
        },
        warnings,
    })
}

/// 创建软件工程任务。
///
/// 前端调用：`invoke('create_task', { request: { content, agentId, llmSettings } })`
#[tauri::command]
pub async fn create_task(
    request: CreateTaskRequest,
    state: State<'_, AppState>,
) -> Result<CreateTaskResponse, ApiError> {
    let content = request.content.trim();
    if content.is_empty() {
        return Err(ApiError::invalid_argument("请输入任务目标。"));
    }

    let (target_meta, _, _) = resolve_agent_meta(request.agent_id.as_deref())?;
    let msg_type = if target_meta.runtime_name == "Planner" {
        "plan_request"
    } else {
        "direct_message"
    };

    let request_context = if target_meta.runtime_name == "Planner" {
        build_request_context_with_knowledge(
            request.llm_settings.as_ref(),
            state.knowledge_base.as_ref(),
            content,
        )
    } else {
        build_request_context(request.llm_settings.as_ref())
    };
    let transient_context = build_transient_request_context(request.llm_settings.as_ref());
    let mut orch = state.orchestrator.lock().await;
    let task_id = orch
        .submit_task_to_agent_with_context_and_transient(
            target_meta.runtime_name,
            content,
            msg_type,
            request_context,
            transient_context,
        )
        .await
        .map_err(|e| ApiError::route_failed(format!("{}", e)))?;
    let task = orch.get_task(&task_id).await;

    Ok(CreateTaskResponse { task_id, task })
}

/// 获取单个 Agent 的当前状态。
///
/// 前端调用：`invoke('get_agent_status', { agentId })`
#[tauri::command]
pub async fn get_agent_status(
    agent_id: String,
    state: State<'_, AppState>,
) -> Result<AgentStatusResponse, ApiError> {
    let requested = resolve_agent_meta(Some(&agent_id))?.0;
    let orch = state.orchestrator.lock().await;
    let agents = orch.list_agents();
    let runtime_name = agents
        .into_iter()
        .find(|name| name == requested.runtime_name)
        .ok_or_else(|| ApiError::agent_not_found(&agent_id))?;

    Ok(build_agent_status(runtime_name))
}

/// 列出所有已注册的 Agent 及其状态。
///
/// 前端调用：`invoke('list_agents')`
#[tauri::command]
pub async fn list_agents(state: State<'_, AppState>) -> Result<AgentListResponse, ApiError> {
    let orch = state.orchestrator.lock().await;
    let mut agents = orch.list_agents();
    agents.sort_by_key(|name| {
        let meta = meta_for_runtime_name(name);
        AGENT_CATALOG
            .iter()
            .position(|item| item.runtime_name == meta.runtime_name)
            .unwrap_or(usize::MAX)
    });

    Ok(AgentListResponse {
        agents: agents.into_iter().map(build_agent_status).collect(),
    })
}

/// 获取指定任务的状态和结果。
///
/// 前端调用：`invoke('get_task_result', { taskId })`
#[tauri::command]
pub async fn get_task_result(
    task_id: String,
    state: State<'_, AppState>,
) -> Result<Option<serde_json::Value>, ApiError> {
    let orch = state.orchestrator.lock().await;

    let result = orch.get_task_result(&task_id).await.map(|r| {
        serde_json::json!({
            "taskId": r.task_id,
            "status": r.status,
            "steps": r.steps,
            "output": r.output,
        })
    });

    Ok(result)
}

/// 获取任务详情。
///
/// 前端调用：`invoke('get_task', { taskId })`
#[tauri::command]
pub async fn get_task(
    task_id: String,
    state: State<'_, AppState>,
) -> Result<Option<Task>, ApiError> {
    let orch = state.orchestrator.lock().await;
    Ok(orch.get_task(&task_id).await)
}

/// 获取任务列表。
///
/// 前端调用：`invoke('list_tasks')`
#[tauri::command]
pub async fn list_tasks(state: State<'_, AppState>) -> Result<TaskListResponse, ApiError> {
    let orch = state.orchestrator.lock().await;
    Ok(TaskListResponse {
        tasks: orch.list_tasks().await,
    })
}

/// 获取任务事件流。
///
/// 前端调用：`invoke('get_task_events', { taskId })`
#[tauri::command]
pub async fn get_task_events(
    task_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<TaskEvent>, ApiError> {
    let orch = state.orchestrator.lock().await;
    Ok(orch.get_task_events(&task_id).await)
}

/// 取消任务。
///
/// 前端调用：`invoke('cancel_task', { request: { taskId, reason } })`
#[tauri::command]
pub async fn cancel_task(
    request: CancelTaskRequest,
    state: State<'_, AppState>,
) -> Result<CancelTaskResponse, ApiError> {
    let task_id = request.task_id.trim();
    if task_id.is_empty() {
        return Err(ApiError::invalid_argument("缺少要取消的任务 ID。"));
    }

    let reason = request
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("用户取消任务。");

    let orch = state.orchestrator.lock().await;
    let task = orch.cancel_task(task_id, reason).await;
    Ok(CancelTaskResponse {
        task_id: task_id.to_string(),
        task,
    })
}

/// 重试失败或已取消的任务。
///
/// 前端调用：`invoke('retry_task', { request: { taskId, reason } })`
#[tauri::command]
pub async fn retry_task(
    request: RetryTaskRequest,
    state: State<'_, AppState>,
) -> Result<RetryTaskResponse, ApiError> {
    let task_id = request.task_id.trim();
    if task_id.is_empty() {
        return Err(ApiError::invalid_argument("缺少要重试的任务 ID。"));
    }

    let reason = request
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("用户重试任务。");

    let orch = state.orchestrator.lock().await;
    let task = orch.retry_task(task_id, reason).await;
    Ok(RetryTaskResponse {
        task_id: task_id.to_string(),
        task,
    })
}

/// 跳过单个任务步骤。
///
/// 前端调用：`invoke('skip_task_step', { request: { taskId, stepId, reason } })`
#[tauri::command]
pub async fn skip_task_step(
    request: SkipTaskStepRequest,
    state: State<'_, AppState>,
) -> Result<SkipTaskStepResponse, ApiError> {
    let task_id = request.task_id.trim();
    if task_id.is_empty() {
        return Err(ApiError::invalid_argument("缺少要操作的任务 ID。"));
    }
    let step_id = request.step_id.trim();
    if step_id.is_empty() {
        return Err(ApiError::invalid_argument("缺少要跳过的步骤 ID。"));
    }

    let reason = request
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("用户跳过步骤。");

    let orch = state.orchestrator.lock().await;
    let task = orch.skip_task_step(task_id, step_id, reason).await;
    Ok(SkipTaskStepResponse {
        task_id: task_id.to_string(),
        step_id: step_id.to_string(),
        task,
    })
}

/// 获取当前项目快照。
///
/// 前端调用：`invoke('get_project_snapshot')`
#[tauri::command]
pub async fn get_project_snapshot() -> Result<ProjectSnapshot, ApiError> {
    crate::project::scan_project().map_err(|e| ApiError::workspace_failed(format!("{}", e)))
}

/// 列出项目文件。
///
/// 前端调用：`invoke('list_project_files', { maxFiles })`
#[tauri::command]
pub async fn list_project_files(
    max_files: Option<usize>,
) -> Result<ProjectFileListResponse, ApiError> {
    crate::workspace::list_files(max_files)
        .map(|files| ProjectFileListResponse { files })
        .map_err(|e| ApiError::workspace_failed(format!("{}", e)))
}

/// 读取项目内文本文件。
///
/// 前端调用：`invoke('read_project_file', { path })`
#[tauri::command]
pub async fn read_project_file(path: String) -> Result<FileReadResponse, ApiError> {
    crate::workspace::read_file(&path).map_err(|e| ApiError::workspace_failed(format!("{}", e)))
}

/// 搜索项目内文本。
///
/// 前端调用：`invoke('search_project_text', { request: { query, maxResults } })`
#[tauri::command]
pub async fn search_project_text(
    request: SearchProjectTextRequest,
) -> Result<SearchResponse, ApiError> {
    crate::workspace::search_text(&request.query, request.max_results)
        .map_err(|e| ApiError::workspace_failed(format!("{}", e)))
}

/// 存储长期知识条目。
///
/// 前端调用：`invoke('store_knowledge', { request: { title, content, source?, tags? } })`
#[tauri::command]
pub async fn store_knowledge(
    request: StoreKnowledgeRequest,
    state: State<'_, AppState>,
) -> Result<StoreKnowledgeResponse, ApiError> {
    let title = clean_required_text(&request.title, "知识标题")?;
    let content = clean_required_text(&request.content, "知识内容")?;
    let source = clean_optional_string(request.source.as_ref());
    let tags = clean_tags(request.tags);

    let id = state
        .knowledge_base
        .store_knowledge(&title, &content, source.as_deref(), Some(&tags))
        .map_err(|err| ApiError::knowledge_failed(format!("{}", err)))?;
    Ok(StoreKnowledgeResponse { id })
}

/// 搜索长期知识条目。
///
/// 前端调用：`invoke('search_knowledge', { request: { query, limit } })`
#[tauri::command]
pub async fn search_knowledge(
    request: SearchKnowledgeRequest,
    state: State<'_, AppState>,
) -> Result<SearchKnowledgeResponse, ApiError> {
    let query = clean_required_text(&request.query, "搜索关键词")?;
    let limit = request.limit.unwrap_or(20).clamp(1, 100);
    let items = state
        .knowledge_base
        .search_knowledge(&query, limit)
        .map_err(|err| ApiError::knowledge_failed(format!("{}", err)))?;
    Ok(SearchKnowledgeResponse { query, items })
}

/// 运行受控项目命令。
///
/// 前端调用：`invoke('run_project_command', { request: { command, workingDir } })`
#[tauri::command]
pub async fn run_project_command(
    request: ProjectCommandRunRequest,
    state: State<'_, AppState>,
) -> Result<ProjectCommandRunResponse, ApiError> {
    let result = crate::runtime::run_project_command(request)
        .await
        .map_err(|err| match err {
            AgentError::MessageFormat(message) => ApiError::invalid_argument(&message),
            other => ApiError::command_failed(format!("{}", other)),
        })?;
    state
        .command_store
        .append_run(&result)
        .map_err(|err| ApiError::command_failed(format!("{}", err)))?;
    Ok(result)
}

async fn run_patch_auto_verification(
    command_store: &CommandRunStore,
) -> (Vec<ProjectCommandRunResponse>, Vec<Value>) {
    let snapshot = match crate::project::scan_project() {
        Ok(snapshot) => snapshot,
        Err(err) => {
            return (
                Vec::new(),
                vec![serde_json::json!({
                    "phase": "scanProject",
                    "error": format!("{}", err),
                })],
            );
        }
    };

    let mut runs = Vec::new();
    let mut errors = Vec::new();
    for command in snapshot.recommended_commands {
        let request = ProjectCommandRunRequest {
            command: command.command.clone(),
            working_dir: command.working_dir.clone(),
            task_id: None,
            step_id: None,
        };
        match crate::runtime::run_project_command(request).await {
            Ok(run) => {
                if let Err(err) = command_store.append_run(&run) {
                    errors.push(serde_json::json!({
                        "phase": "audit",
                        "runId": &run.id,
                        "command": &run.command,
                        "workingDir": &run.working_dir,
                        "error": format!("{}", err),
                    }));
                }
                runs.push(run);
            }
            Err(err) => errors.push(serde_json::json!({
                "phase": "runCommand",
                "command": command.command,
                "workingDir": command.working_dir,
                "error": format!("{}", err),
            })),
        }
    }

    (runs, errors)
}

fn patch_verification_failed(runs: &[ProjectCommandRunResponse], errors: &[Value]) -> bool {
    !errors.is_empty() || runs.iter().any(|run| !run.success)
}

fn preview_for_knowledge(value: &str) -> String {
    const PREVIEW_CHARS: usize = 600;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut preview = trimmed.chars().take(PREVIEW_CHARS).collect::<String>();
    if trimmed.chars().nth(PREVIEW_CHARS).is_some() {
        preview.push_str("\n[preview truncated]");
    }
    preview
}

fn failed_command_summary(run: &ProjectCommandRunResponse) -> String {
    let mut lines = vec![format!(
        "- `{}` in `{}` failed: exitCode={:?}, timedOut={}",
        run.command, run.working_dir, run.exit_code, run.timed_out
    )];
    let stderr = preview_for_knowledge(&run.stderr);
    if !stderr.is_empty() {
        lines.push(format!("  stderr:\n{}", stderr));
    }
    let stdout = preview_for_knowledge(&run.stdout);
    if !stdout.is_empty() {
        lines.push(format!("  stdout:\n{}", stdout));
    }
    lines.join("\n")
}

fn verification_error_summary(error: &Value) -> String {
    preview_for_knowledge(&serde_json::to_string(error).unwrap_or_else(|_| error.to_string()))
}

fn auto_rollback_summary(auto_rollback: Option<&PatchAutoRollbackResult>) -> &'static str {
    match auto_rollback {
        Some(result) if result.reverted => "验证失败后已自动回滚补丁。",
        Some(result) if !result.reverted => "验证失败后尝试自动回滚，但回滚失败，需要人工处理。",
        None => "验证失败后未自动回滚，补丁仍可能保留在工作区。",
        _ => "验证失败后的回滚状态未知。",
    }
}

fn patch_verification_failure_knowledge(
    proposal: &PatchProposal,
    approval_id: &str,
    runs: &[ProjectCommandRunResponse],
    errors: &[Value],
    auto_rollback: Option<&PatchAutoRollbackResult>,
) -> Option<(String, String, Option<String>, Vec<String>)> {
    if !patch_verification_failed(runs, errors) {
        return None;
    }

    let failed_runs = runs
        .iter()
        .filter(|run| !run.success)
        .map(failed_command_summary)
        .collect::<Vec<_>>();
    let command_section = if failed_runs.is_empty() {
        "无失败命令记录。".to_string()
    } else {
        failed_runs.join("\n")
    };
    let error_section = if errors.is_empty() {
        "无额外验证错误。".to_string()
    } else {
        errors
            .iter()
            .map(verification_error_summary)
            .collect::<Vec<_>>()
            .join("\n")
    };
    let files = proposal
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let task_id = proposal.task_id.as_deref().unwrap_or("unlinked");
    let step_id = proposal.step_id.as_deref().unwrap_or("unlinked");
    let rollback = auto_rollback_summary(auto_rollback);
    let title = format!("FailureCase: {}", proposal.summary);
    let content = format!(
        "补丁验证失败经验\n\n任务: {task_id}\n步骤: {step_id}\n补丁: {}\n审批: {approval_id}\n摘要: {}\n文件: {}\n\n失败命令:\n{}\n\n验证错误:\n{}\n\n回滚状态: {}\n\n后续处理: 先修复失败命令输出中暴露的问题，再通过 retry_task 重新调度；如果补丁仍在工作区且风险较高，优先手动回滚。",
        proposal.id, proposal.summary, files, command_section, error_section, rollback
    );
    let mut tags = vec![
        "FailureCase".to_string(),
        "patchVerification".to_string(),
        "autoExperience".to_string(),
    ];
    if auto_rollback.is_some_and(|result| result.reverted) {
        tags.push("autoRollback".to_string());
    }
    Some((
        title,
        content,
        Some(format!("patch:{};approval:{}", proposal.id, approval_id)),
        tags,
    ))
}

fn store_patch_verification_failure_knowledge(
    knowledge_base: &KnowledgeBase,
    proposal: &PatchProposal,
    approval_id: &str,
    runs: &[ProjectCommandRunResponse],
    errors: &[Value],
    auto_rollback: Option<&PatchAutoRollbackResult>,
) -> Result<Option<String>, AgentError> {
    let Some((title, content, source, tags)) =
        patch_verification_failure_knowledge(proposal, approval_id, runs, errors, auto_rollback)
    else {
        return Ok(None);
    };

    knowledge_base
        .store_knowledge(&title, &content, source.as_deref(), Some(&tags))
        .map(Some)
}

const PATCH_AUTO_ROLLBACK_ON_VERIFICATION_FAILURE_ENV: &str =
    "PATCH_AUTO_ROLLBACK_ON_VERIFICATION_FAILURE";

fn parse_bool_flag(value: Option<&str>) -> bool {
    value
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .is_some_and(|value| matches!(value.as_str(), "true" | "1" | "yes" | "on"))
}

fn default_auto_rollback_on_verification_failure() -> bool {
    parse_bool_flag(
        std::env::var(PATCH_AUTO_ROLLBACK_ON_VERIFICATION_FAILURE_ENV)
            .ok()
            .as_deref(),
    )
}

fn resolve_auto_rollback_on_verification_failure(request_value: Option<bool>) -> bool {
    request_value.unwrap_or_else(default_auto_rollback_on_verification_failure)
}

fn successful_auto_rollback_result(result: PatchRevertResult) -> PatchAutoRollbackResult {
    PatchAutoRollbackResult {
        triggered_by: "verificationFailure".to_string(),
        reverted: true,
        error: None,
        result: Some(result),
    }
}

fn failed_auto_rollback_result(error: AgentError) -> PatchAutoRollbackResult {
    PatchAutoRollbackResult {
        triggered_by: "verificationFailure".to_string(),
        reverted: false,
        error: Some(error.to_string()),
        result: None,
    }
}

fn build_project_command_approval_input(
    inspection: &ProjectCommandInspection,
    request: &ProjectCommandRunRequest,
) -> CreateApprovalRequest {
    CreateApprovalRequest {
        task_id: request.task_id.clone(),
        step_id: request.step_id.clone(),
        title: format!("运行项目命令：{}", inspection.command),
        reason: format!(
            "命令 [{}] 不在受控允许列表中，需要用户确认后再进入后续执行流程。",
            inspection.command
        ),
        risk: RiskLevel::High,
        action_type: "runtime.runProjectCommand".to_string(),
        action_payload: serde_json::json!({
            "command": inspection.command.clone(),
            "workingDir": inspection.working_dir.clone(),
            "allowedByDefault": inspection.allowed,
            "taskId": request.task_id.clone(),
            "stepId": request.step_id.clone(),
        }),
        requested_by: Some("ProjectPanel".to_string()),
    }
}

fn build_tool_action_approval_input(
    request: ToolActionApprovalRequest,
) -> Result<CreateApprovalRequest, ApiError> {
    let action_type = request.action_type.trim();
    if !action_type.starts_with("tool.") {
        return Err(ApiError::invalid_argument(
            "通用工具审批动作类型必须以 tool. 开头。",
        ));
    }
    if request.title.trim().is_empty() {
        return Err(ApiError::invalid_argument("缺少审批标题。"));
    }
    if request.reason.trim().is_empty() {
        return Err(ApiError::invalid_argument("缺少审批原因。"));
    }

    Ok(CreateApprovalRequest {
        task_id: clean_optional_text(request.task_id),
        step_id: clean_optional_text(request.step_id),
        title: request.title.trim().to_string(),
        reason: request.reason.trim().to_string(),
        risk: request.risk.unwrap_or(RiskLevel::Medium),
        action_type: action_type.to_string(),
        action_payload: request.action_payload,
        requested_by: clean_optional_text(request.requested_by)
            .or_else(|| Some("ToolAgent".to_string())),
    })
}

fn clean_optional_text(value: Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

/// 为非 allowlist 项目命令创建审批请求。
///
/// 前端调用：`invoke('request_project_command_approval', { request: { command, workingDir } })`
#[tauri::command]
pub async fn request_project_command_approval(
    request: ProjectCommandRunRequest,
    state: State<'_, AppState>,
) -> Result<ApprovalRequest, ApiError> {
    let inspection =
        crate::runtime::inspect_project_command_request(&request).map_err(|err| match err {
            AgentError::MessageFormat(message) => ApiError::invalid_argument(&message),
            other => ApiError::command_failed(format!("{}", other)),
        })?;

    if inspection.allowed {
        return Err(ApiError::invalid_argument(
            "该命令已在受控允许列表中，可以直接运行。",
        ));
    }

    let approval = state
        .approval_store
        .create_request(build_project_command_approval_input(&inspection, &request))
        .map_err(|err| ApiError::approval_failed(format!("{}", err)))?;

    {
        let orch = state.orchestrator.lock().await;
        orch.record_command_approval_requested(&approval).await;
    }

    Ok(approval)
}

/// 创建通用工具动作审批请求，并让关联任务/步骤进入等待审批。
///
/// 前端调用：`invoke('request_tool_action_approval', { request })`
#[tauri::command]
pub async fn request_tool_action_approval(
    request: ToolActionApprovalRequest,
    state: State<'_, AppState>,
) -> Result<ApprovalRequest, ApiError> {
    let approval_input = build_tool_action_approval_input(request)?;
    let approval = state
        .approval_store
        .create_request(approval_input)
        .map_err(|err| ApiError::approval_failed(format!("{}", err)))?;

    {
        let orch = state.orchestrator.lock().await;
        orch.record_tool_approval_requested(&approval).await;
    }

    Ok(approval)
}

fn project_command_request_from_approval(
    approval: &ApprovalRequest,
) -> Result<ProjectCommandRunRequest, ApiError> {
    if approval.action_type != "runtime.runProjectCommand" {
        return Err(ApiError::invalid_argument(
            "这条审批不是项目命令执行请求，不能作为命令运行。",
        ));
    }
    if approval.status != ApprovalStatus::Approved {
        return Err(ApiError::invalid_argument(
            "审批请求尚未通过，不能执行对应命令。",
        ));
    }

    let command = approval
        .action_payload
        .get("command")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::invalid_argument("审批 payload 缺少命令内容。"))?;
    let working_dir = approval
        .action_payload
        .get("workingDir")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::invalid_argument("审批 payload 缺少工作目录。"))?;

    Ok(ProjectCommandRunRequest {
        command: command.to_string(),
        working_dir: working_dir.to_string(),
        task_id: approval.task_id.clone(),
        step_id: approval.step_id.clone(),
    })
}

/// 执行已通过审批的项目命令。
///
/// 前端调用：`invoke('run_approved_project_command', { request: { approvalId } })`
#[tauri::command]
pub async fn run_approved_project_command(
    request: RunApprovedProjectCommandRequest,
    state: State<'_, AppState>,
) -> Result<ProjectCommandRunResponse, ApiError> {
    let approval_id = request.approval_id.trim();
    if approval_id.is_empty() {
        return Err(ApiError::invalid_argument("缺少审批请求 ID。"));
    }

    let Some(approval) = state
        .approval_store
        .get_request(approval_id)
        .map_err(|err| ApiError::approval_failed(format!("{}", err)))?
    else {
        return Err(ApiError::invalid_argument("找不到对应的审批请求。"));
    };
    let command_request = project_command_request_from_approval(&approval)?;

    if let Some(existing) = state
        .command_store
        .get_run_by_approval_id(&approval.id)
        .map_err(|err| ApiError::command_failed(format!("{}", err)))?
    {
        {
            let orch = state.orchestrator.lock().await;
            orch.record_command_run(&approval, &existing).await;
        }
        return Ok(existing);
    }

    let result = crate::runtime::run_approved_project_command(command_request, approval.id.clone())
        .await
        .map_err(|err| match err {
            AgentError::MessageFormat(message) => ApiError::invalid_argument(&message),
            other => ApiError::command_failed(format!("{}", other)),
        })?;
    state
        .command_store
        .append_run(&result)
        .map_err(|err| ApiError::command_failed(format!("{}", err)))?;
    {
        let orch = state.orchestrator.lock().await;
        orch.record_command_run(&approval, &result).await;
    }

    Ok(result)
}

/// 列出最近的受控项目命令运行记录。
///
/// 前端调用：`invoke('list_project_command_runs', { limit })`
#[tauri::command]
pub async fn list_project_command_runs(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<ProjectCommandRunListResponse, ApiError> {
    state
        .command_store
        .list_runs(limit)
        .map(|runs| ProjectCommandRunListResponse { runs })
        .map_err(|err| ApiError::command_failed(format!("{}", err)))
}

/// 列出最近的 ToolAgent 工具调用审计记录。
///
/// 前端调用：`invoke('list_tool_invocations', { limit })`
#[tauri::command]
pub async fn list_tool_invocations(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<ToolInvocationListResponse, ApiError> {
    state
        .tool_invocation_store
        .list_invocations(limit)
        .map(|invocations| ToolInvocationListResponse { invocations })
        .map_err(|err| ApiError::tool_audit_failed(format!("{}", err)))
}

fn map_patch_error(err: AgentError) -> ApiError {
    match err {
        AgentError::MessageFormat(message) => ApiError::invalid_argument(&message),
        other => ApiError::patch_failed(format!("{}", other)),
    }
}

fn build_patch_approval_input(proposal: &PatchProposal) -> CreateApprovalRequest {
    let files = proposal
        .files
        .iter()
        .map(|file| {
            serde_json::json!({
                "path": &file.path,
                "changeType": &file.change_type,
                "diff": &file.diff,
            })
        })
        .collect::<Vec<_>>();

    CreateApprovalRequest {
        task_id: proposal.task_id.clone(),
        step_id: proposal.step_id.clone(),
        title: format!("应用补丁：{}", proposal.summary),
        reason: format!(
            "补丁提案 [{}] 将修改 {} 个文件，需要用户确认 diff 后再进入应用流程。",
            proposal.summary,
            proposal.files.len()
        ),
        risk: RiskLevel::High,
        action_type: "workspace.applyPatch".to_string(),
        action_payload: serde_json::json!({
            "patchId": &proposal.id,
            "summary": &proposal.summary,
            "files": files,
            "unifiedDiff": &proposal.unified_diff,
            "defaultAutoRollbackOnVerificationFailure": default_auto_rollback_on_verification_failure(),
        }),
        requested_by: Some(proposal.requested_by.clone()),
    }
}

fn patch_id_from_approval(approval: &ApprovalRequest) -> Option<String> {
    if approval.action_type != "workspace.applyPatch" {
        return None;
    }
    approval
        .action_payload
        .get("patchId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn approved_patch_id_from_approval(approval: &ApprovalRequest) -> Result<String, ApiError> {
    if approval.action_type != "workspace.applyPatch" {
        return Err(ApiError::invalid_argument(
            "这条审批不是补丁应用请求，不能作为补丁应用。",
        ));
    }
    if approval.status != ApprovalStatus::Approved {
        return Err(ApiError::invalid_argument(
            "审批请求尚未通过，不能应用对应补丁。",
        ));
    }

    patch_id_from_approval(approval)
        .ok_or_else(|| ApiError::invalid_argument("审批 payload 缺少补丁提案 ID。"))
}

fn patch_status_from_approval(status: &ApprovalStatus) -> Option<PatchProposalStatus> {
    match status {
        ApprovalStatus::Pending => Some(PatchProposalStatus::PendingApproval),
        ApprovalStatus::Approved => Some(PatchProposalStatus::Approved),
        ApprovalStatus::Rejected => Some(PatchProposalStatus::Rejected),
        ApprovalStatus::Cancelled => None,
    }
}

/// 创建补丁提案，并为 `workspace.applyPatch` 动作生成审批请求。
///
/// 前端调用：`invoke('create_patch_proposal', { request: { summary, files } })`
#[tauri::command]
pub async fn create_patch_proposal(
    request: CreatePatchProposalRequest,
    state: State<'_, AppState>,
) -> Result<CreatePatchProposalResponse, ApiError> {
    let mut proposal = crate::workspace::build_patch_proposal(request).map_err(map_patch_error)?;
    state
        .patch_store
        .save_proposal(&proposal)
        .map_err(|err| ApiError::patch_failed(format!("{}", err)))?;

    let approval = state
        .approval_store
        .create_request(build_patch_approval_input(&proposal))
        .map_err(|err| ApiError::approval_failed(format!("{}", err)))?;

    proposal.attach_approval(approval.id.clone());
    state
        .patch_store
        .save_proposal(&proposal)
        .map_err(|err| ApiError::patch_failed(format!("{}", err)))?;
    {
        let orch = state.orchestrator.lock().await;
        orch.record_patch_approval_requested(&proposal, &approval)
            .await;
    }

    Ok(CreatePatchProposalResponse { proposal, approval })
}

/// 列出最近的补丁提案。
///
/// 前端调用：`invoke('list_patch_proposals', { limit })`
#[tauri::command]
pub async fn list_patch_proposals(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<PatchProposalListResponse, ApiError> {
    state
        .patch_store
        .list_proposals(limit)
        .map(|proposals| PatchProposalListResponse { proposals })
        .map_err(|err| ApiError::patch_failed(format!("{}", err)))
}

/// 获取单个补丁提案。
///
/// 前端调用：`invoke('get_patch_proposal', { patchId })`
#[tauri::command]
pub async fn get_patch_proposal(
    patch_id: String,
    state: State<'_, AppState>,
) -> Result<Option<PatchProposal>, ApiError> {
    let patch_id = patch_id.trim();
    if patch_id.is_empty() {
        return Err(ApiError::invalid_argument("缺少补丁提案 ID。"));
    }
    state
        .patch_store
        .get_proposal(patch_id)
        .map_err(|err| ApiError::patch_failed(format!("{}", err)))
}

/// 应用已通过审批的补丁提案。
///
/// 前端调用：`invoke('apply_approved_patch', { request: { approvalId } })`
#[tauri::command]
pub async fn apply_approved_patch(
    request: ApplyApprovedPatchRequest,
    state: State<'_, AppState>,
) -> Result<PatchApplyResult, ApiError> {
    let approval_id = request.approval_id.trim();
    if approval_id.is_empty() {
        return Err(ApiError::invalid_argument("缺少审批请求 ID。"));
    }

    let Some(approval) = state
        .approval_store
        .get_request(approval_id)
        .map_err(|err| ApiError::approval_failed(format!("{}", err)))?
    else {
        return Err(ApiError::invalid_argument("找不到对应的审批请求。"));
    };
    let patch_id = approved_patch_id_from_approval(&approval)?;

    let Some(mut proposal) = state
        .patch_store
        .get_proposal(&patch_id)
        .map_err(|err| ApiError::patch_failed(format!("{}", err)))?
    else {
        return Err(ApiError::invalid_argument("找不到对应的补丁提案。"));
    };

    if proposal.approval_id.as_deref() != Some(approval.id.as_str()) {
        return Err(ApiError::invalid_argument(
            "补丁提案与审批请求不匹配，已拒绝应用。",
        ));
    }

    let mut result = crate::workspace::apply_patch_proposal(&mut proposal, Some("user"))
        .map_err(map_patch_error)?;
    state
        .patch_store
        .save_proposal(&proposal)
        .map_err(|err| ApiError::patch_failed(format!("{}", err)))?;
    {
        let orch = state.orchestrator.lock().await;
        orch.record_patch_applied(&proposal, &result, &approval.id)
            .await;
    }
    if !result.already_applied {
        let command_store = state.command_store.clone();
        let (verification_runs, verification_errors) =
            run_patch_auto_verification(command_store.as_ref()).await;
        let mut auto_rollback = None;
        let mut reverted = None;

        let auto_rollback_on_verification_failure = resolve_auto_rollback_on_verification_failure(
            request.auto_rollback_on_verification_failure,
        );

        if auto_rollback_on_verification_failure
            && patch_verification_failed(&verification_runs, &verification_errors)
        {
            match crate::workspace::revert_patch_proposal(&mut proposal, Some("auto-verification"))
            {
                Ok(revert_result) => {
                    state
                        .patch_store
                        .save_proposal(&proposal)
                        .map_err(|err| ApiError::patch_failed(format!("{}", err)))?;
                    result.status = proposal.status.clone();
                    reverted = Some(revert_result.clone());
                    auto_rollback = Some(successful_auto_rollback_result(revert_result));
                }
                Err(err) => {
                    auto_rollback = Some(failed_auto_rollback_result(err));
                }
            }
        }

        let orch = state.orchestrator.lock().await;
        orch.record_patch_verification(
            &proposal,
            &approval.id,
            &verification_runs,
            &verification_errors,
            auto_rollback.as_ref(),
        )
        .await;
        if let Some(revert_result) = reverted.as_ref() {
            orch.record_patch_reverted(&proposal, revert_result).await;
        }
        if let Err(err) = store_patch_verification_failure_knowledge(
            state.knowledge_base.as_ref(),
            &proposal,
            &approval.id,
            &verification_runs,
            &verification_errors,
            auto_rollback.as_ref(),
        ) {
            tracing::warn!("写入补丁验证失败经验失败: {}", err);
        }
        result.auto_rollback = auto_rollback;
    }

    Ok(result)
}

/// 回滚已应用的补丁提案。
///
/// 前端调用：`invoke('revert_applied_patch', { request: { patchId } })`
#[tauri::command]
pub async fn revert_applied_patch(
    request: RevertAppliedPatchRequest,
    state: State<'_, AppState>,
) -> Result<PatchRevertResult, ApiError> {
    let patch_id = request.patch_id.trim();
    if patch_id.is_empty() {
        return Err(ApiError::invalid_argument("缺少补丁提案 ID。"));
    }

    let Some(mut proposal) = state
        .patch_store
        .get_proposal(patch_id)
        .map_err(|err| ApiError::patch_failed(format!("{}", err)))?
    else {
        return Err(ApiError::invalid_argument("找不到对应的补丁提案。"));
    };

    let result = crate::workspace::revert_patch_proposal(&mut proposal, Some("user"))
        .map_err(map_patch_error)?;
    state
        .patch_store
        .save_proposal(&proposal)
        .map_err(|err| ApiError::patch_failed(format!("{}", err)))?;
    {
        let orch = state.orchestrator.lock().await;
        orch.record_patch_reverted(&proposal, &result).await;
    }

    Ok(result)
}

/// 列出审批请求。
///
/// 前端调用：`invoke('list_approval_requests', { status, limit })`
#[tauri::command]
pub async fn list_approval_requests(
    status: Option<String>,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<ApprovalListResponse, ApiError> {
    let status = parse_approval_status(status.as_deref()).map_err(|err| match err {
        AgentError::MessageFormat(message) => ApiError::invalid_argument(&message),
        other => ApiError::approval_failed(format!("{}", other)),
    })?;
    state
        .approval_store
        .list_requests(status, limit)
        .map(|approvals| ApprovalListResponse { approvals })
        .map_err(|err| ApiError::approval_failed(format!("{}", err)))
}

/// 审批或拒绝一个高风险动作。
///
/// 前端调用：`invoke('approve_action', { request: { approvalId, approved, note? } })`
#[tauri::command]
pub async fn approve_action(
    request: ApprovalDecisionRequest,
    state: State<'_, AppState>,
) -> Result<Option<ApprovalRequest>, ApiError> {
    if request.approval_id.trim().is_empty() {
        return Err(ApiError::invalid_argument("缺少审批请求 ID。"));
    }
    let updated = state
        .approval_store
        .decide(request)
        .map_err(|err| match err {
            AgentError::MessageFormat(message) => ApiError::invalid_argument(&message),
            other => ApiError::approval_failed(format!("{}", other)),
        })?;

    if let Some(approval) = &updated {
        if approval.action_type == "runtime.runProjectCommand" {
            let orch = state.orchestrator.lock().await;
            orch.record_command_approval_resolved(approval).await;
        }

        if approval.action_type.starts_with("tool.") {
            let orch = state.orchestrator.lock().await;
            orch.resolve_tool_approval(approval).await;
        }

        if let (Some(patch_id), Some(status)) = (
            patch_id_from_approval(approval),
            patch_status_from_approval(&approval.status),
        ) {
            let proposal = state
                .patch_store
                .update_status(&patch_id, status)
                .map_err(|err| ApiError::patch_failed(format!("{}", err)))?;
            if let Some(proposal) = proposal {
                let orch = state.orchestrator.lock().await;
                orch.record_patch_approval_resolved(&proposal, approval)
                    .await;
            }
        }
    }

    Ok(updated)
}

/// 获取系统健康状态。
///
/// 前端调用：`invoke('health_check')`
#[tauri::command]
pub async fn health_check(state: State<'_, AppState>) -> Result<HealthCheckResponse, ApiError> {
    let orch = state.orchestrator.lock().await;
    Ok(HealthCheckResponse {
        healthy: true,
        version: env!("CARGO_PKG_VERSION").to_string(),
        agent_count: orch.list_agents().len(),
    })
}

/// 获取对话历史。
///
#[tauri::command]
pub async fn get_history(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessageResponse>, ApiError> {
    let session_id = clean_session_id(Some(&session_id));
    let messages = state
        .chat_store
        .list_messages(&session_id)
        .map_err(|err| ApiError::history_failed(format!("{}", err)))?;
    Ok(messages
        .into_iter()
        .map(response_from_stored_message)
        .collect())
}

/// 清空对话历史。
#[tauri::command]
pub async fn clear_history(session_id: String, state: State<'_, AppState>) -> Result<(), ApiError> {
    let session_id = clean_session_id(Some(&session_id));
    state
        .chat_store
        .clear_session(&session_id)
        .map_err(|err| ApiError::history_failed(format!("{}", err)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_patch_proposal(summary: &str) -> PatchProposal {
        use crate::workspace::patch::{PatchChangeType, PatchFileChange};

        let now = chrono::Utc::now();
        PatchProposal {
            id: "patch-test-1".to_string(),
            task_id: Some("task-test-1".to_string()),
            step_id: Some("task-test-1-1".to_string()),
            approval_id: Some("approval-1".to_string()),
            summary: summary.to_string(),
            status: PatchProposalStatus::Applied,
            files: vec![PatchFileChange {
                path: "README.md".to_string(),
                change_type: PatchChangeType::Modify,
                old_content: "old".to_string(),
                new_content: "new".to_string(),
                diff: crate::workspace::patch::build_unified_diff("README.md", "old", "new"),
            }],
            unified_diff: crate::workspace::patch::build_unified_diff("README.md", "old", "new"),
            requested_by: "ProjectPanel".to_string(),
            created_at: now,
            updated_at: now,
            applied_at: Some(now),
            applied_by: Some("user".to_string()),
            reverted_at: None,
            reverted_by: None,
        }
    }

    fn test_command_run(command: &str, success: bool) -> ProjectCommandRunResponse {
        ProjectCommandRunResponse {
            id: format!("run-{command}"),
            approval_id: None,
            command: command.to_string(),
            working_dir: "src-tauri".to_string(),
            exit_code: Some(if success { 0 } else { 101 }),
            success,
            stdout: if success {
                "ok".to_string()
            } else {
                "build started".to_string()
            },
            stderr: if success {
                String::new()
            } else {
                "assertion failed in verification".to_string()
            },
            duration_ms: 42,
            timed_out: false,
            stdout_truncated: false,
            stderr_truncated: false,
            created_at: "2026-06-09T12:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_build_request_context_redacts_api_key() {
        let settings = FrontendLlmSettingsRequest {
            model: Some(" deepseek-chat ".to_string()),
            api_key: Some("sk-secret".to_string()),
            api_base_url: Some(" https://api.deepseek.com/v1 ".to_string()),
            max_tokens: Some(2048),
            temperature: Some(0.3),
        };

        let context = build_request_context(Some(&settings));
        let serialized = serde_json::to_string(&context).unwrap();

        assert_eq!(
            context["frontendLlmSettings"]["model"],
            serde_json::json!("deepseek-chat")
        );
        assert_eq!(context["frontendLlmSettings"]["hasApiKey"], true);
        assert!(!serialized.contains("sk-secret"));
        assert!(serialized.contains("hasApiKey"));
    }

    #[test]
    fn test_build_request_context_omits_blank_settings() {
        let settings = FrontendLlmSettingsRequest {
            model: Some("  ".to_string()),
            api_key: Some("  ".to_string()),
            api_base_url: None,
            max_tokens: None,
            temperature: None,
        };

        let context = build_request_context(Some(&settings));
        assert!(context["frontendLlmSettings"]["model"].is_null());
        assert_eq!(context["frontendLlmSettings"]["hasApiKey"], false);
    }

    #[test]
    fn test_build_knowledge_context_collects_project_fact_and_failure_case() {
        let kb = KnowledgeBase::new(":memory:");
        kb.initialize().unwrap();
        kb.store_knowledge(
            "ProjectFact: 前端任务面板",
            "TaskBoard 负责展示任务、事件和补丁验证状态。",
            Some("manual"),
            Some(&["ProjectFact".to_string(), "frontend".to_string()]),
        )
        .unwrap();
        kb.store_knowledge(
            "FailureCase: 补丁验证失败",
            &format!(
                "{}\n{}",
                "cargo test 失败时先查看 stderr。",
                "x".repeat(1_000)
            ),
            Some("patch:1"),
            Some(&["FailureCase".to_string(), "patchVerification".to_string()]),
        )
        .unwrap();

        let context = build_knowledge_context(&kb, "优化任务面板").unwrap();
        let items = context["knowledgeContext"]["items"].as_array().unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(context["knowledgeContext"]["queries"][1], "ProjectFact");
        assert!(items
            .iter()
            .any(|item| item["title"].as_str().unwrap().contains("ProjectFact")));
        let failure = items
            .iter()
            .find(|item| item["title"].as_str().unwrap().contains("FailureCase"))
            .unwrap();
        assert!(failure["contentPreview"]
            .as_str()
            .unwrap()
            .contains("[preview truncated]"));
    }

    #[test]
    fn test_build_request_context_with_knowledge_merges_settings() {
        let kb = KnowledgeBase::new(":memory:");
        kb.initialize().unwrap();
        kb.store_knowledge(
            "FailureCase: Rust 测试失败",
            "cargo test 失败后先看失败用例。",
            None,
            Some(&["FailureCase".to_string()]),
        )
        .unwrap();
        let settings = FrontendLlmSettingsRequest {
            model: Some("deepseek-chat".to_string()),
            api_key: None,
            api_base_url: None,
            max_tokens: None,
            temperature: None,
        };

        let context =
            build_request_context_with_knowledge(Some(&settings), &kb, "修复 Rust 测试失败");

        assert_eq!(
            context["frontendLlmSettings"]["model"],
            serde_json::json!("deepseek-chat")
        );
        assert_eq!(
            context["knowledgeContext"]["items"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn test_build_transient_request_context_keeps_api_key_out_of_safe_context() {
        let settings = FrontendLlmSettingsRequest {
            model: Some("deepseek-chat".to_string()),
            api_key: Some("sk-request".to_string()),
            api_base_url: Some("https://api.deepseek.com/v1".to_string()),
            max_tokens: Some(1024),
            temperature: Some(0.2),
        };

        let safe = build_request_context(Some(&settings));
        let transient = build_transient_request_context(Some(&settings));

        assert!(!safe.to_string().contains("sk-request"));
        assert_eq!(
            transient["plannerLlmSettings"]["apiKey"],
            serde_json::json!("sk-request")
        );
    }

    #[test]
    fn test_clean_session_id_defaults_blank_values() {
        assert_eq!(clean_session_id(None), "default");
        assert_eq!(clean_session_id(Some(&"  ".to_string())), "default");
        assert_eq!(
            clean_session_id(Some(&" session-1 ".to_string())),
            "session-1"
        );
    }

    #[test]
    fn test_store_knowledge_request_deserializes_tags() {
        let request: StoreKnowledgeRequest = serde_json::from_value(serde_json::json!({
            "title": "失败经验",
            "content": "cargo test 失败时先看 stderr 摘要",
            "source": "task-1",
            "tags": ["FailureCase", " cargo "]
        }))
        .unwrap();

        assert_eq!(request.title, "失败经验");
        assert_eq!(request.source.as_deref(), Some("task-1"));
        assert_eq!(
            clean_tags(request.tags),
            vec!["FailureCase".to_string(), "cargo".to_string()]
        );
    }

    #[test]
    fn test_clean_required_text_rejects_blank() {
        let err = clean_required_text("  ", "知识标题").unwrap_err();
        assert_eq!(err.code, "INVALID_ARGUMENT");
        assert!(err.message.contains("知识标题"));
        assert_eq!(clean_required_text("  abc  ", "知识标题").unwrap(), "abc");
    }

    #[test]
    fn test_build_project_command_approval_input_records_payload() {
        let inspection = ProjectCommandInspection {
            command: "cargo clippy".to_string(),
            working_dir: "src-tauri".to_string(),
            allowed: false,
        };
        let request = ProjectCommandRunRequest {
            command: "cargo clippy".to_string(),
            working_dir: "src-tauri".to_string(),
            task_id: Some("task-1".to_string()),
            step_id: Some("step-1".to_string()),
        };

        let input = build_project_command_approval_input(&inspection, &request);

        assert_eq!(input.risk, RiskLevel::High);
        assert_eq!(input.action_type, "runtime.runProjectCommand");
        assert_eq!(input.task_id.as_deref(), Some("task-1"));
        assert_eq!(input.step_id.as_deref(), Some("step-1"));
        assert_eq!(input.requested_by.as_deref(), Some("ProjectPanel"));
        assert_eq!(
            input.action_payload["command"],
            serde_json::json!("cargo clippy")
        );
        assert_eq!(
            input.action_payload["workingDir"],
            serde_json::json!("src-tauri")
        );
        assert_eq!(
            input.action_payload["allowedByDefault"],
            serde_json::json!(false)
        );
        assert_eq!(input.action_payload["taskId"], serde_json::json!("task-1"));
        assert_eq!(input.action_payload["stepId"], serde_json::json!("step-1"));
    }

    #[test]
    fn test_project_command_request_from_approval_requires_approved_status() {
        let mut approval = ApprovalRequest::new(CreateApprovalRequest {
            task_id: None,
            step_id: None,
            title: "运行命令".to_string(),
            reason: "需要确认。".to_string(),
            risk: RiskLevel::High,
            action_type: "runtime.runProjectCommand".to_string(),
            action_payload: serde_json::json!({
                "command": "cargo clippy",
                "workingDir": "src-tauri",
            }),
            requested_by: Some("ProjectPanel".to_string()),
        })
        .unwrap();

        let err = project_command_request_from_approval(&approval).unwrap_err();
        assert_eq!(err.code, "INVALID_ARGUMENT");
        assert!(err.message.contains("尚未通过"));

        approval.status = ApprovalStatus::Approved;
        let request = project_command_request_from_approval(&approval).unwrap();
        assert_eq!(request.command, "cargo clippy");
        assert_eq!(request.working_dir, "src-tauri");
    }

    #[test]
    fn test_project_command_request_from_approval_rejects_wrong_action_type() {
        let mut approval = ApprovalRequest::new(CreateApprovalRequest {
            task_id: None,
            step_id: None,
            title: "应用 patch".to_string(),
            reason: "需要确认。".to_string(),
            risk: RiskLevel::High,
            action_type: "workspace.applyPatch".to_string(),
            action_payload: serde_json::json!({
                "command": "cargo clippy",
                "workingDir": "src-tauri",
            }),
            requested_by: Some("ProjectPanel".to_string()),
        })
        .unwrap();
        approval.status = ApprovalStatus::Approved;

        let err = project_command_request_from_approval(&approval).unwrap_err();
        assert_eq!(err.code, "INVALID_ARGUMENT");
        assert!(err.message.contains("不是项目命令"));
    }

    #[test]
    fn test_build_tool_action_approval_input_defaults_and_validates_prefix() {
        let input = build_tool_action_approval_input(ToolActionApprovalRequest {
            task_id: Some(" task-1 ".to_string()),
            step_id: Some(" step-1 ".to_string()),
            title: " 读取受限文件 ".to_string(),
            reason: " 需要确认工具动作。 ".to_string(),
            action_type: " tool.fileRead ".to_string(),
            action_payload: serde_json::json!({ "tool": "file_read", "path": ".env" }),
            risk: None,
            requested_by: None,
        })
        .unwrap();

        assert_eq!(input.task_id.as_deref(), Some("task-1"));
        assert_eq!(input.step_id.as_deref(), Some("step-1"));
        assert_eq!(input.title, "读取受限文件");
        assert_eq!(input.reason, "需要确认工具动作。");
        assert_eq!(input.action_type, "tool.fileRead");
        assert_eq!(input.risk, RiskLevel::Medium);
        assert_eq!(input.requested_by.as_deref(), Some("ToolAgent"));
        assert_eq!(input.action_payload["tool"], serde_json::json!("file_read"));

        let err = build_tool_action_approval_input(ToolActionApprovalRequest {
            task_id: None,
            step_id: None,
            title: "运行命令".to_string(),
            reason: "需要确认。".to_string(),
            action_type: "runtime.runProjectCommand".to_string(),
            action_payload: serde_json::json!({}),
            risk: Some(RiskLevel::High),
            requested_by: Some("test".to_string()),
        })
        .unwrap_err();
        assert_eq!(err.code, "INVALID_ARGUMENT");
        assert!(err.message.contains("tool."));
    }

    #[test]
    fn test_build_patch_approval_input_records_diff_payload() {
        use crate::workspace::patch::{PatchChangeType, PatchFileChange};

        let now = chrono::Utc::now();
        let diff = crate::workspace::patch::build_unified_diff("README.md", "old line", "new line");
        let proposal = PatchProposal {
            id: "patch-1".to_string(),
            task_id: Some("task-1".to_string()),
            step_id: Some("step-1".to_string()),
            approval_id: None,
            summary: "更新 README".to_string(),
            status: PatchProposalStatus::Draft,
            files: vec![PatchFileChange {
                path: "README.md".to_string(),
                change_type: PatchChangeType::Modify,
                old_content: "old line".to_string(),
                new_content: "new line".to_string(),
                diff: diff.clone(),
            }],
            unified_diff: diff.clone(),
            requested_by: "ProjectPanel".to_string(),
            created_at: now,
            updated_at: now,
            applied_at: None,
            applied_by: None,
            reverted_at: None,
            reverted_by: None,
        };

        let input = build_patch_approval_input(&proposal);

        assert_eq!(input.risk, RiskLevel::High);
        assert_eq!(input.action_type, "workspace.applyPatch");
        assert_eq!(input.task_id.as_deref(), Some("task-1"));
        assert_eq!(input.step_id.as_deref(), Some("step-1"));
        assert_eq!(input.requested_by.as_deref(), Some("ProjectPanel"));
        assert_eq!(
            input.action_payload["patchId"],
            serde_json::json!("patch-1")
        );
        assert_eq!(
            input.action_payload["summary"],
            serde_json::json!("更新 README")
        );
        assert_eq!(input.action_payload["unifiedDiff"], serde_json::json!(diff));
        assert_eq!(
            input.action_payload["defaultAutoRollbackOnVerificationFailure"],
            serde_json::json!(default_auto_rollback_on_verification_failure())
        );
        assert_eq!(
            input.action_payload["files"][0]["path"],
            serde_json::json!("README.md")
        );
    }

    #[test]
    fn test_apply_patch_request_keeps_auto_rollback_optional() {
        let request: ApplyApprovedPatchRequest =
            serde_json::from_value(serde_json::json!({ "approvalId": "approval-1" })).unwrap();

        assert_eq!(request.approval_id, "approval-1");
        assert_eq!(request.auto_rollback_on_verification_failure, None);

        let request: ApplyApprovedPatchRequest = serde_json::from_value(serde_json::json!({
            "approvalId": "approval-1",
            "autoRollbackOnVerificationFailure": false
        }))
        .unwrap();

        assert_eq!(request.auto_rollback_on_verification_failure, Some(false));
    }

    #[test]
    fn test_auto_rollback_policy_parses_truthy_flags_and_respects_overrides() {
        for value in ["true", "1", "yes", "on", " TRUE "] {
            assert!(parse_bool_flag(Some(value)));
        }
        for value in ["false", "0", "no", "off", "", "maybe"] {
            assert!(!parse_bool_flag(Some(value)));
        }
        assert!(!parse_bool_flag(None));
        assert!(resolve_auto_rollback_on_verification_failure(Some(true)));
        assert!(!resolve_auto_rollback_on_verification_failure(Some(false)));
    }

    #[test]
    fn test_patch_verification_failed_detects_run_failure_or_errors() {
        let success = ProjectCommandRunResponse {
            id: "run-1".to_string(),
            approval_id: None,
            command: "cargo test".to_string(),
            working_dir: "src-tauri".to_string(),
            exit_code: Some(0),
            success: true,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 42,
            timed_out: false,
            stdout_truncated: false,
            stderr_truncated: false,
            created_at: "2026-06-09T12:00:00Z".to_string(),
        };
        let failure = ProjectCommandRunResponse {
            success: false,
            exit_code: Some(101),
            ..success.clone()
        };

        assert!(!patch_verification_failed(&[success], &[]));
        assert!(patch_verification_failed(&[failure], &[]));
        assert!(patch_verification_failed(
            &[],
            &[serde_json::json!({ "phase": "runCommand", "error": "boom" })]
        ));
    }

    #[test]
    fn test_patch_verification_failure_knowledge_skips_success() {
        let proposal = test_patch_proposal("更新 README");
        let runs = vec![test_command_run("cargo test", true)];

        let note = patch_verification_failure_knowledge(&proposal, "approval-1", &runs, &[], None);

        assert!(note.is_none());
    }

    #[test]
    fn test_store_patch_verification_failure_knowledge_writes_failure_case() {
        let kb = KnowledgeBase::new(":memory:");
        kb.initialize().unwrap();
        let proposal = test_patch_proposal("更新 README");
        let runs = vec![test_command_run("cargo test", false)];

        let id = store_patch_verification_failure_knowledge(
            &kb,
            &proposal,
            "approval-1",
            &runs,
            &[serde_json::json!({ "phase": "audit", "error": "audit failed" })],
            None,
        )
        .unwrap()
        .unwrap();

        assert!(!id.is_empty());
        let results = kb.search_knowledge("FailureCase", 10).unwrap();
        assert_eq!(results.len(), 1);
        let item = &results[0];
        assert!(item.title.contains("更新 README"));
        assert_eq!(
            item.source.as_deref(),
            Some("patch:patch-test-1;approval:approval-1")
        );
        assert!(item.tags.contains(&"FailureCase".to_string()));
        assert!(item.tags.contains(&"patchVerification".to_string()));
        assert!(item.tags.contains(&"autoExperience".to_string()));
        assert!(item.content.contains("cargo test"));
        assert!(item.content.contains("assertion failed in verification"));
        assert!(item.content.contains("audit failed"));
        assert!(item.content.contains("retry_task"));
    }

    #[test]
    fn test_approved_patch_id_from_approval_requires_patch_action_and_approved_status() {
        let mut approval = ApprovalRequest::new(CreateApprovalRequest {
            task_id: None,
            step_id: None,
            title: "应用补丁".to_string(),
            reason: "需要确认。".to_string(),
            risk: RiskLevel::High,
            action_type: "workspace.applyPatch".to_string(),
            action_payload: serde_json::json!({
                "patchId": "patch-1",
                "unifiedDiff": "diff --git a/README.md b/README.md",
            }),
            requested_by: Some("ProjectPanel".to_string()),
        })
        .unwrap();

        let err = approved_patch_id_from_approval(&approval).unwrap_err();
        assert_eq!(err.code, "INVALID_ARGUMENT");
        assert!(err.message.contains("尚未通过"));

        approval.status = ApprovalStatus::Approved;
        assert_eq!(
            approved_patch_id_from_approval(&approval).unwrap(),
            "patch-1"
        );

        approval.action_type = "runtime.runProjectCommand".to_string();
        let err = approved_patch_id_from_approval(&approval).unwrap_err();
        assert_eq!(err.code, "INVALID_ARGUMENT");
        assert!(err.message.contains("不是补丁应用请求"));
    }
}
