//! 调度器模块 — 多 Agent 协同任务调度中心
//!
//! Orchestrator 是系统的核心调度组件，负责：
//! 1. 管理所有已注册的 Agent
//! 2. 接收用户输入，创建可追踪 Task
//! 3. 监听 Agent 回复并更新任务状态
//! 4. 在任务执行闭环 v1 中用只读工具观察和模拟执行推进计划步骤

use crate::agent::echo_agent::EchoAgent;
use crate::agent::executor_agent::ExecutorAgent;
use crate::agent::memory_agent::MemoryAgent;
use crate::agent::planner_agent::{PlannerAgent, TaskPlan};
use crate::agent::tool_agent::ToolAgent;
use crate::agent::traits::{Agent, AgentMessage};
use crate::bus::message_bus::MessageBus;
use crate::error::AgentError;
use crate::task::{StepStatus, Task, TaskEvent, TaskEventKind, TaskStatus, TaskStep};
use crate::workspace;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

const READONLY_TOOL_SEARCH_LIMIT: usize = 8;
const READONLY_TOOL_READ_LIMIT: usize = 3;
const READONLY_TOOL_PREVIEW_CHARS: usize = 2_000;

/// Agent 运行时信息
struct AgentRuntime {
    /// 向该 Agent 发送消息的通道
    sender: tokio::sync::mpsc::UnboundedSender<AgentMessage>,
    /// Agent 名称
    name: String,
}

/// 任务执行结果（旧接口兼容结构）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskResult {
    /// 任务 ID
    pub task_id: String,
    /// 执行状态
    pub status: String,
    /// 执行步骤
    pub steps: Vec<serde_json::Value>,
    /// 最终输出
    pub output: String,
}

impl From<&Task> for TaskResult {
    fn from(task: &Task) -> Self {
        Self {
            task_id: task.id.clone(),
            status: status_label(&task.status).to_string(),
            steps: task
                .steps
                .iter()
                .filter_map(|step| serde_json::to_value(step).ok())
                .collect(),
            output: task.output.clone().unwrap_or_default(),
        }
    }
}

#[derive(Default)]
struct TaskRuntime {
    tasks: HashMap<String, Task>,
    events: HashMap<String, Vec<TaskEvent>>,
}

impl TaskRuntime {
    fn insert_task(&mut self, task: Task) {
        let task_id = task.id.clone();
        self.tasks.insert(task_id.clone(), task);
        self.push_event(TaskEvent::new(
            task_id,
            None,
            TaskEventKind::Created,
            "任务已创建，等待规划。",
            serde_json::Value::Null,
        ));
    }

    fn prepare_direct_step(&mut self, task_id: &str, agent_name: &str, instruction: &str) {
        let mut emitted = Vec::new();

        if let Some(task) = self.tasks.get_mut(task_id) {
            let mut step = TaskStep::new(
                format!("{task_id}-1"),
                task_id.to_string(),
                1,
                agent_name.to_string(),
                instruction.to_string(),
                Vec::new(),
            );
            step.start();

            emitted.push(TaskEvent::new(
                task_id.to_string(),
                Some(step.id.clone()),
                TaskEventKind::StepStarted,
                format!("{} 开始处理任务。", step.agent_id),
                serde_json::json!({
                    "agentId": step.agent_id,
                    "instruction": step.instruction,
                }),
            ));

            task.steps = vec![step];
            task.status = TaskStatus::Running;
            task.touch();
        }

        self.extend_events(emitted);
    }

    fn apply_plan(&mut self, task_id: &str, plan: TaskPlan) {
        let steps = plan
            .steps
            .into_iter()
            .map(|step| {
                TaskStep::new(
                    step.step_id,
                    task_id.to_string(),
                    step.order,
                    step.agent,
                    step.instruction,
                    step.depends_on,
                )
            })
            .collect::<Vec<_>>();

        let step_count = steps.len();
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.set_steps(steps);
            task.user_goal = plan.goal;
        }

        self.push_event(TaskEvent::new(
            task_id.to_string(),
            None,
            TaskEventKind::Planned,
            format!("Planner 已生成 {step_count} 个步骤。"),
            serde_json::json!({ "stepCount": step_count }),
        ));
    }

    fn run_task_v1(&mut self, task_id: &str) {
        let mut emitted = Vec::new();
        let mut completed_count = 0usize;

        if let Some(task) = self.tasks.get_mut(task_id) {
            task.status = TaskStatus::Running;
            task.touch();

            for step in &mut task.steps {
                if step.status != StepStatus::Pending {
                    continue;
                }

                step.start();
                emitted.push(TaskEvent::new(
                    task_id.to_string(),
                    Some(step.id.clone()),
                    TaskEventKind::StepStarted,
                    format!("{} 开始执行：{}", step.agent_id, step.title),
                    serde_json::json!({
                        "agentId": step.agent_id,
                        "instruction": step.instruction,
                    }),
                ));

                let result = execute_task_step_v1(step);
                step.complete(result.clone());
                completed_count += 1;

                emitted.push(TaskEvent::new(
                    task_id.to_string(),
                    Some(step.id.clone()),
                    TaskEventKind::StepCompleted,
                    format!("{} 已完成步骤。", step.agent_id),
                    result,
                ));
            }

            if task
                .steps
                .iter()
                .all(|step| step.status == StepStatus::Completed)
            {
                let output = format!(
                    "任务执行闭环 v1 已完成，共完成 {} 个步骤。",
                    task.steps.len()
                );
                task.complete(output.clone());
                emitted.push(TaskEvent::new(
                    task_id.to_string(),
                    None,
                    TaskEventKind::Completed,
                    output,
                    serde_json::json!({ "completedSteps": completed_count }),
                ));
            }
        }

        self.extend_events(emitted);
    }

    fn complete_running_step(
        &mut self,
        task_id: &str,
        agent_name: &str,
        content: &str,
        context: serde_json::Value,
    ) {
        let mut emitted = Vec::new();
        let mut task_completed = None;

        if let Some(task) = self.tasks.get_mut(task_id) {
            if let Some(step) = task
                .steps
                .iter_mut()
                .find(|step| step.status == StepStatus::Running && step.agent_id == agent_name)
            {
                let result = serde_json::json!({
                    "agentId": agent_name,
                    "content": content,
                    "context": context,
                });
                step.complete(result.clone());
                emitted.push(TaskEvent::new(
                    task_id.to_string(),
                    Some(step.id.clone()),
                    TaskEventKind::StepCompleted,
                    format!("{} 已返回执行结果。", agent_name),
                    result,
                ));
            }

            if !task.steps.is_empty()
                && task
                    .steps
                    .iter()
                    .all(|step| step.status == StepStatus::Completed)
            {
                let output = content.to_string();
                task.complete(output.clone());
                task_completed = Some(output);
            }
        }

        if let Some(output) = task_completed {
            emitted.push(TaskEvent::new(
                task_id.to_string(),
                None,
                TaskEventKind::Completed,
                output,
                serde_json::Value::Null,
            ));
        }

        self.extend_events(emitted);
    }

    fn fail_running_step(&mut self, task_id: &str, agent_name: &str, error: &str) {
        let mut emitted = Vec::new();

        if let Some(task) = self.tasks.get_mut(task_id) {
            if let Some(step) = task
                .steps
                .iter_mut()
                .find(|step| step.status == StepStatus::Running && step.agent_id == agent_name)
            {
                step.fail(error);
                emitted.push(TaskEvent::new(
                    task_id.to_string(),
                    Some(step.id.clone()),
                    TaskEventKind::StepFailed,
                    error.to_string(),
                    serde_json::json!({ "agentId": agent_name }),
                ));
            }

            task.fail(error);
            emitted.push(TaskEvent::new(
                task_id.to_string(),
                None,
                TaskEventKind::Failed,
                error.to_string(),
                serde_json::Value::Null,
            ));
        }

        self.extend_events(emitted);
    }

    fn get_task(&self, task_id: &str) -> Option<Task> {
        self.tasks.get(task_id).cloned()
    }

    fn list_tasks(&self) -> Vec<Task> {
        let mut tasks = self.tasks.values().cloned().collect::<Vec<_>>();
        tasks.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        tasks
    }

    fn get_events(&self, task_id: &str) -> Vec<TaskEvent> {
        self.events.get(task_id).cloned().unwrap_or_default()
    }

    fn push_event(&mut self, event: TaskEvent) {
        self.events
            .entry(event.task_id.clone())
            .or_default()
            .push(event);
    }

    fn extend_events(&mut self, events: Vec<TaskEvent>) {
        for event in events {
            self.push_event(event);
        }
    }
}

/// 调度器 — 多 Agent 系统的中央调度组件
pub struct Orchestrator {
    /// 已注册的 Agent 运行时信息
    agents: HashMap<String, AgentRuntime>,
    /// 消息总线
    bus: Arc<MessageBus>,
    /// 任务运行状态
    runtime: Arc<Mutex<TaskRuntime>>,
    /// 是否已启动后台事件监听器
    event_loop_started: bool,
}

impl Orchestrator {
    /// 创建新的调度器
    pub fn new(bus: Arc<MessageBus>) -> Self {
        Self {
            agents: HashMap::new(),
            bus,
            runtime: Arc::new(Mutex::new(TaskRuntime::default())),
            event_loop_started: false,
        }
    }

    /// 注册所有内置 Agent 并启动其运行循环
    ///
    /// 内置 Agent 包括：Echo、Planner、Executor、Memory、Tool。
    pub async fn register_builtin_agents(&mut self) {
        tracing::info!("[Orchestrator] 正在注册内置 Agent...");

        self.register_and_spawn(Box::new(EchoAgent::new())).await;
        self.register_and_spawn(Box::new(PlannerAgent::new())).await;
        self.register_and_spawn(Box::new(ExecutorAgent::new()))
            .await;
        self.register_and_spawn(Box::new(MemoryAgent::new())).await;
        self.register_and_spawn(Box::new(ToolAgent::default()))
            .await;
        self.start_task_event_loop();

        tracing::info!(
            "[Orchestrator] 已注册 {} 个 Agent: {:?}",
            self.agents.len(),
            self.list_agents()
        );
    }

    /// 注册单个 Agent 并启动其运行循环
    async fn register_and_spawn(&mut self, mut agent: Box<dyn Agent>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<AgentMessage>();
        let name = agent.name().to_string();

        // 启动 Agent 的主循环
        let bus_tx = self.bus.sender();
        let agent_name = name.clone();

        tokio::spawn(async move {
            tracing::info!("[{}] Agent 启动", agent_name);
            agent.run(rx, bus_tx).await;
        });

        self.agents
            .insert(name.clone(), AgentRuntime { sender: tx, name });
    }

    fn start_task_event_loop(&mut self) {
        if self.event_loop_started {
            return;
        }

        let mut rx = self.bus.subscribe();
        let runtime = self.runtime.clone();

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(msg) => handle_agent_message(runtime.clone(), msg).await,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!("[Orchestrator] 任务事件监听落后，跳过 {skipped} 条消息");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        tracing::warn!("[Orchestrator] 消息总线已关闭，停止任务事件监听");
                        break;
                    }
                }
            }
        });

        self.event_loop_started = true;
    }

    /// 提交用户任务。
    ///
    /// 默认入口交给 PlannerAgent（产品侧展示为“协调员/总控”）。
    pub async fn submit_task(&mut self, user_input: &str) -> Result<String, AgentError> {
        self.submit_task_to_agent("Planner", user_input, "plan_request")
            .await
    }

    /// 提交用户任务到指定 Agent。
    ///
    /// 注意：Agent 的主循环通过 mpsc 接收消息，回复再发回 MessageBus。
    /// 因此用户入口必须使用 `send_to_agent`，不能直接 `bus.publish`。
    pub async fn submit_task_to_agent(
        &mut self,
        agent_name: &str,
        user_input: &str,
        msg_type: &str,
    ) -> Result<String, AgentError> {
        let task_id = uuid::Uuid::new_v4().to_string();
        tracing::info!(
            "[Orchestrator] 接收任务 [{}]，目标 Agent [{}]: {}",
            task_id,
            agent_name,
            user_input
        );

        {
            let mut runtime = self.runtime.lock().await;
            runtime.insert_task(Task::new(task_id.clone(), user_input));
            if agent_name != "Planner" {
                runtime.prepare_direct_step(&task_id, agent_name, user_input);
            }
        }

        let context = if agent_name == "Planner" {
            build_planner_message_context(user_input)
        } else {
            serde_json::Value::Null
        };

        let msg = AgentMessage::new("Orchestrator", agent_name, user_input)
            .with_task_id(&task_id)
            .with_type(msg_type)
            .with_context(context);

        self.send_to_agent(agent_name, msg)?;

        tracing::info!(
            "[Orchestrator] 任务 [{}] 已发送给 Agent [{}]",
            task_id,
            agent_name
        );

        Ok(task_id)
    }

    /// 向指定 Agent 发送消息（通过 mpsc 通道）
    pub fn send_to_agent(&self, agent_name: &str, msg: AgentMessage) -> Result<(), AgentError> {
        match self.agents.get(agent_name) {
            Some(runtime) => runtime.sender.send(msg).map_err(|e| {
                AgentError::BusError(format!("无法向 Agent [{}] 发送消息: {}", agent_name, e))
            }),
            None => Err(AgentError::BusError(format!(
                "Agent [{}] 未注册",
                agent_name
            ))),
        }
    }

    /// 查询任务执行结果（旧接口兼容）。
    pub async fn get_task_result(&self, task_id: &str) -> Option<TaskResult> {
        let runtime = self.runtime.lock().await;
        runtime.get_task(task_id).as_ref().map(TaskResult::from)
    }

    /// 查询单个任务。
    pub async fn get_task(&self, task_id: &str) -> Option<Task> {
        let runtime = self.runtime.lock().await;
        runtime.get_task(task_id)
    }

    /// 列出任务。
    pub async fn list_tasks(&self) -> Vec<Task> {
        let runtime = self.runtime.lock().await;
        runtime.list_tasks()
    }

    /// 获取任务事件。
    pub async fn get_task_events(&self, task_id: &str) -> Vec<TaskEvent> {
        let runtime = self.runtime.lock().await;
        runtime.get_events(task_id)
    }

    /// 列出所有已注册的 Agent 名称
    pub fn list_agents(&self) -> Vec<String> {
        self.agents
            .values()
            .map(|runtime| runtime.name.clone())
            .collect()
    }

    /// 获取消息总线的引用
    pub fn bus(&self) -> &Arc<MessageBus> {
        &self.bus
    }
}

async fn handle_agent_message(runtime: Arc<Mutex<TaskRuntime>>, msg: AgentMessage) {
    let Some(task_id) = msg.task_id.clone() else {
        return;
    };

    match msg.msg_type.as_str() {
        "plan_created" => match serde_json::from_value::<TaskPlan>(msg.context.clone()) {
            Ok(plan) => {
                let mut runtime = runtime.lock().await;
                runtime.apply_plan(&task_id, plan);
                runtime.run_task_v1(&task_id);
            }
            Err(err) => {
                let mut runtime = runtime.lock().await;
                runtime.fail_running_step(&task_id, "Planner", &format!("计划解析失败: {err}"));
            }
        },
        "execution_result" | "tool_result" | "memory_ack" | "echo_reply" => {
            let mut runtime = runtime.lock().await;
            runtime.complete_running_step(&task_id, &msg.from, &msg.content, msg.context);
        }
        "execution_error" => {
            let mut runtime = runtime.lock().await;
            runtime.fail_running_step(&task_id, &msg.from, &msg.content);
        }
        _ => {}
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReadonlyToolInstruction {
    query: String,
    focus_paths: Vec<String>,
}

fn execute_task_step_v1(step: &TaskStep) -> serde_json::Value {
    if step.agent_id == "Tool" {
        if let Some(result) = execute_readonly_tool_step(&step.instruction) {
            return result;
        }
    }

    serde_json::json!({
        "mode": "simulated",
        "agentId": step.agent_id,
        "summary": format!("{} 已完成模拟执行。", step.agent_id),
        "instruction": step.instruction,
    })
}

fn execute_readonly_tool_step(instruction: &str) -> Option<serde_json::Value> {
    let parsed = parse_readonly_tool_instruction(instruction)?;
    let mut candidate_paths = parsed.focus_paths.clone();

    let (matches, matches_truncated, search_error) =
        match workspace::search_text(&parsed.query, Some(READONLY_TOOL_SEARCH_LIMIT)) {
            Ok(response) => {
                for item in &response.matches {
                    push_unique_path(&mut candidate_paths, &item.path);
                }
                (response.matches, response.truncated, None)
            }
            Err(err) => (Vec::new(), false, Some(format!("{}", err))),
        };

    let mut read_files = Vec::new();
    let mut read_errors = Vec::new();
    for path in candidate_paths {
        if read_files.len() >= READONLY_TOOL_READ_LIMIT {
            break;
        }

        match workspace::read_file(&path) {
            Ok(file) => {
                let (preview, truncated) = text_preview(&file.content, READONLY_TOOL_PREVIEW_CHARS);
                read_files.push(serde_json::json!({
                    "path": file.path,
                    "sizeBytes": file.size_bytes,
                    "preview": preview,
                    "truncated": truncated,
                }));
            }
            Err(err) => {
                read_errors.push(serde_json::json!({
                    "path": path,
                    "error": format!("{}", err),
                }));
            }
        }
    }

    let match_count = matches.len();
    let read_count = read_files.len();
    let summary = match &search_error {
        Some(err) => format!("只读检索失败，已记录错误: {err}"),
        None => format!("只读检索完成，命中 {match_count} 条，读取 {read_count} 个文件预览。"),
    };

    Some(serde_json::json!({
        "mode": "workspace.readonly",
        "tool": "workspace.search_text/read_file",
        "query": parsed.query,
        "focusPaths": parsed.focus_paths,
        "matches": matches,
        "matchesTruncated": matches_truncated,
        "readFiles": read_files,
        "readErrors": read_errors,
        "searchError": search_error,
        "summary": summary,
    }))
}

fn parse_readonly_tool_instruction(instruction: &str) -> Option<ReadonlyToolInstruction> {
    if !(instruction.contains("只读")
        && (instruction.contains("检索") || instruction.contains("搜索")))
    {
        return None;
    }

    let query = extract_labeled_segment(instruction, &["搜索词:", "搜索词："])?;
    if query.is_empty() {
        return None;
    }

    let focus_paths = extract_labeled_segment(instruction, &["优先关注:", "优先关注："])
        .map(|raw| parse_focus_paths(&raw))
        .unwrap_or_default();

    Some(ReadonlyToolInstruction { query, focus_paths })
}

fn extract_labeled_segment(text: &str, labels: &[&str]) -> Option<String> {
    for label in labels {
        let Some(start) = text.find(label) else {
            continue;
        };
        let value = &text[start + label.len()..];
        let end = value
            .find(|ch| matches!(ch, '。' | '\n' | '\r'))
            .unwrap_or(value.len());
        let segment = value[..end]
            .trim()
            .trim_matches(|ch| matches!(ch, ',' | '，' | ';' | '；'))
            .trim()
            .to_string();
        if !segment.is_empty() {
            return Some(segment);
        }
    }

    None
}

fn parse_focus_paths(raw: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for item in raw.split(|ch| matches!(ch, ',' | '，' | ';' | '；' | '\n' | '\r')) {
        let path = normalize_focus_path(item);
        if looks_like_workspace_path(&path) {
            push_unique_path(&mut paths, &path);
        }
    }

    paths
}

fn normalize_focus_path(path: &str) -> String {
    path.trim()
        .trim_matches(|ch| matches!(ch, '"' | '\'' | '`' | '“' | '”' | '‘' | '’'))
        .replace('\\', "/")
}

fn looks_like_workspace_path(path: &str) -> bool {
    if path.is_empty() || path.contains(' ') {
        return false;
    }

    path.contains('/') || path.rsplit_once('.').is_some()
}

fn push_unique_path(paths: &mut Vec<String>, path: &str) {
    if !paths.iter().any(|existing| existing == path) {
        paths.push(path.to_string());
    }
}

fn text_preview(text: &str, max_chars: usize) -> (String, bool) {
    let mut chars = text.chars();
    let preview = chars.by_ref().take(max_chars).collect::<String>();
    let truncated = chars.next().is_some();
    (preview, truncated)
}

fn status_label(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Draft => "草稿",
        TaskStatus::Planning | TaskStatus::Running | TaskStatus::Reviewing => "执行中",
        TaskStatus::WaitingApproval => "等待审批",
        TaskStatus::Failed => "失败",
        TaskStatus::Completed => "完成",
        TaskStatus::Cancelled => "已取消",
    }
}

fn build_planner_message_context(user_input: &str) -> serde_json::Value {
    match crate::project::build_planning_context(user_input) {
        Ok(context) => serde_json::json!({
            "projectPlanningContext": context,
        }),
        Err(err) => serde_json::json!({
            "projectPlanningError": format!("{}", err),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_orchestrator_create_and_register() {
        let bus = Arc::new(MessageBus::new());
        let mut orch = Orchestrator::new(bus);
        orch.register_builtin_agents().await;

        let agents = orch.list_agents();
        assert!(!agents.is_empty());
        assert!(agents.contains(&"Echo".to_string()));
        assert!(agents.contains(&"Planner".to_string()));
    }

    #[tokio::test]
    async fn test_submit_task() {
        let bus = Arc::new(MessageBus::new());
        let mut orch = Orchestrator::new(bus.clone());

        orch.register_builtin_agents().await;

        let task_id = orch.submit_task("帮我整理今日新闻").await.unwrap();
        assert!(!task_id.is_empty());

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        assert!(orch.get_task_result(&task_id).await.is_some());
    }

    /// 测试 — send_to_agent 向注册的 Agent 发送消息
    /// 验证：通过 mpsc 通道可以将消息发送给已注册的 Agent
    #[tokio::test]
    async fn test_send_to_registered_agent() {
        let bus = Arc::new(MessageBus::new());
        let mut orch = Orchestrator::new(bus);
        orch.register_builtin_agents().await;

        // 向 Echo Agent 发送消息
        let msg = AgentMessage::new("Test", "Echo", "测试直连消息");
        let result = orch.send_to_agent("Echo", msg);

        assert!(result.is_ok(), "应向已注册的 Agent 发送成功");
    }

    /// 测试 — send_to_agent 向未注册 Agent 发送应失败
    /// 验证：向不存在的 Agent 名称发送消息会返回错误
    #[tokio::test]
    async fn test_send_to_unregistered_agent_errors() {
        let bus = Arc::new(MessageBus::new());
        let orch = Orchestrator::new(bus);

        // 向不存在的 Agent 发送
        let msg = AgentMessage::new("Test", "GhostAgent", "幽灵消息");
        let result = orch.send_to_agent("GhostAgent", msg);

        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("未注册"));
    }

    /// 测试 — 获取不存在的任务结果返回 None
    /// 验证：查询未提交的任务 ID 返回 None
    #[tokio::test]
    async fn test_get_nonexistent_task() {
        let bus = Arc::new(MessageBus::new());
        let orch = Orchestrator::new(bus);

        let result = orch.get_task_result("non-existent-id").await;
        assert!(result.is_none());
    }

    /// 测试 — 注册所有内置 Agent 后列表完整
    /// 验证：register_builtin_agents 注册了全部 5 个内置 Agent
    #[tokio::test]
    async fn test_all_builtin_agents_registered() {
        let bus = Arc::new(MessageBus::new());
        let mut orch = Orchestrator::new(bus);
        orch.register_builtin_agents().await;

        let agents = orch.list_agents();
        assert_eq!(agents.len(), 5);
        assert!(agents.contains(&"Echo".to_string()));
        assert!(agents.contains(&"Planner".to_string()));
        assert!(agents.contains(&"Executor".to_string()));
        assert!(agents.contains(&"Memory".to_string()));
        assert!(agents.contains(&"Tool".to_string()));
    }

    /// 测试 — 任务提交后状态为"执行中"
    /// 验证：submit_task 创建的任务初始兼容状态正确
    #[tokio::test]
    async fn test_task_initial_status() {
        let bus = Arc::new(MessageBus::new());
        let mut orch = Orchestrator::new(bus);
        orch.register_builtin_agents().await;

        let task_id = orch.submit_task("测试任务状态").await.unwrap();
        let result = orch.get_task_result(&task_id).await.unwrap();

        assert_eq!(result.status, "执行中");
        assert_eq!(result.task_id, task_id);
    }

    /// 测试 — Planner 计划会写入 Task.steps 并完成 v1 闭环
    #[tokio::test]
    async fn test_planner_task_updates_steps_and_completes() {
        let bus = Arc::new(MessageBus::new());
        let mut orch = Orchestrator::new(bus);
        orch.register_builtin_agents().await;

        let task_id = orch.submit_task("帮我搜索今日新闻").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let task = orch.get_task(&task_id).await.unwrap();
        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(task.steps.len(), 3);
        assert!(task
            .steps
            .iter()
            .all(|step| step.status == StepStatus::Completed));

        let events = orch.get_task_events(&task_id).await;
        assert!(events
            .iter()
            .any(|event| event.kind == TaskEventKind::Planned));
        assert!(events
            .iter()
            .any(|event| event.kind == TaskEventKind::Completed));
    }

    /// 测试 — 软件工程任务会带项目上下文生成项目感知计划
    #[tokio::test]
    async fn test_project_aware_planner_task_uses_project_context() {
        let bus = Arc::new(MessageBus::new());
        let mut orch = Orchestrator::new(bus);
        orch.register_builtin_agents().await;

        let task_id = orch.submit_task("优化项目任务看板").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let task = orch.get_task(&task_id).await.unwrap();
        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(task.steps.len(), 4);
        assert!(task.steps[0].instruction.contains("技术栈"));
        assert!(task
            .steps
            .iter()
            .any(|step| step.instruction.contains("Task")));

        let tool_step = task
            .steps
            .iter()
            .find(|step| step.agent_id == "Tool")
            .unwrap();
        let result = tool_step.result.as_ref().unwrap();
        assert_eq!(result["mode"], "workspace.readonly");
        assert_eq!(result["query"], "Task");
        assert!(!result["matches"].as_array().unwrap().is_empty());
        assert!(!result["readFiles"].as_array().unwrap().is_empty());

        let planner_step = task
            .steps
            .iter()
            .find(|step| step.agent_id == "Planner")
            .unwrap();
        assert_eq!(planner_step.result.as_ref().unwrap()["mode"], "simulated");
    }

    /// 测试 — 只读 Tool 指令解析出搜索词和关注文件
    #[test]
    fn test_parse_readonly_tool_instruction() {
        let parsed = parse_readonly_tool_instruction(
            "只读检索相关源码。搜索词: Task。优先关注: src-web/src/components/TaskBoard.tsx, src-tauri/src/orchestrator/mod.rs。",
        )
        .unwrap();

        assert_eq!(parsed.query, "Task");
        assert_eq!(
            parsed.focus_paths,
            vec![
                "src-web/src/components/TaskBoard.tsx".to_string(),
                "src-tauri/src/orchestrator/mod.rs".to_string(),
            ]
        );
    }

    /// 测试 — 非只读 Tool 指令仍走模拟结果
    #[test]
    fn test_execute_task_step_keeps_generic_tool_simulated() {
        let step = TaskStep::new(
            "task-1-1",
            "task-1",
            1,
            "Tool",
            "搜索相关信息: 帮我搜索今日新闻",
            vec![],
        );

        let result = execute_task_step_v1(&step);
        assert_eq!(result["mode"], "simulated");
    }

    /// 测试 — 直连 Echo 任务可以通过 Agent 回复完成
    #[tokio::test]
    async fn test_direct_agent_task_completes_from_reply() {
        let bus = Arc::new(MessageBus::new());
        let mut orch = Orchestrator::new(bus);
        orch.register_builtin_agents().await;

        let task_id = orch
            .submit_task_to_agent("Echo", "ping", "direct_message")
            .await
            .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let task = orch.get_task(&task_id).await.unwrap();
        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(task.steps.len(), 1);
        assert_eq!(task.steps[0].status, StepStatus::Completed);
    }

    /// 测试 — bus() 方法返回正确的引用
    /// 验证：可以通过 orchestrator.bus() 获取消息总线引用
    #[tokio::test]
    async fn test_bus_reference() {
        let bus = Arc::new(MessageBus::new());
        let orch = Orchestrator::new(bus.clone());
        let bus_ref = orch.bus();

        // 验证返回的是同一实例
        assert_eq!(Arc::strong_count(&bus), 2); // bus + bus_ref
        assert_eq!(bus_ref.subscriber_count(), 0);
    }

    /// 测试 — TaskResult 序列化
    /// 验证：TaskResult 可以正确序列化/反序列化
    #[test]
    fn test_task_result_serialization() {
        let result = TaskResult {
            task_id: "t-1".to_string(),
            status: "完成".to_string(),
            steps: vec![serde_json::json!({"step": 1})],
            output: "任务已成功完成".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        let restored: TaskResult = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.task_id, "t-1");
        assert_eq!(restored.status, "完成");
    }
}
