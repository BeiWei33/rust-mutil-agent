//! 调度器模块 — 多 Agent 协同任务调度中心
//!
//! Orchestrator 是系统的核心调度组件，负责：
//! 1. 管理所有已注册的 Agent
//! 2. 接收用户输入，创建可追踪 Task
//! 3. 监听 Agent 回复并更新任务状态
//! 4. 按计划依赖调度步骤，并用只读工具观察项目状态

use crate::agent::echo_agent::EchoAgent;
use crate::agent::executor_agent::ExecutorAgent;
use crate::agent::memory_agent::MemoryAgent;
use crate::agent::planner_agent::{PlannerAgent, TaskPlan};
use crate::agent::tool_agent::ToolAgent;
use crate::agent::traits::{Agent, AgentMessage};
use crate::approval::{ApprovalRequest, ApprovalStatus};
use crate::bus::message_bus::MessageBus;
use crate::error::AgentError;
use crate::runtime::ProjectCommandRunResponse;
use crate::task::{StepStatus, Task, TaskEvent, TaskEventKind, TaskStatus, TaskStep, TaskStore};
use crate::workspace::{
    self, PatchApplyResult, PatchAutoRollbackResult, PatchProposal, PatchRevertResult,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc::UnboundedSender, Mutex};

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

#[derive(Debug, Clone)]
struct StepDispatch {
    task_id: String,
    step_id: String,
    agent_name: String,
    instruction: String,
    context: serde_json::Value,
}

#[derive(Debug, Clone)]
struct RetryTaskResult {
    task: Task,
    dispatches: Vec<StepDispatch>,
    needs_planner: bool,
}

#[derive(Debug, Clone)]
struct SkipStepResult {
    task: Task,
    dispatches: Vec<StepDispatch>,
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
    store: Option<Arc<TaskStore>>,
}

impl TaskRuntime {
    fn with_store(store: Arc<TaskStore>) -> Self {
        let mut runtime = Self {
            tasks: HashMap::new(),
            events: HashMap::new(),
            store: Some(store.clone()),
        };

        match store.load_tasks() {
            Ok(tasks) => {
                for task in tasks {
                    runtime.tasks.insert(task.id.clone(), task);
                }
            }
            Err(err) => tracing::warn!("[TaskRuntime] 加载持久化任务失败: {err}"),
        }

        match store.load_events() {
            Ok(events) => {
                for event in events {
                    runtime
                        .events
                        .entry(event.task_id.clone())
                        .or_default()
                        .push(event);
                }
            }
            Err(err) => tracing::warn!("[TaskRuntime] 加载持久化任务事件失败: {err}"),
        }

        runtime
    }

    fn insert_task(&mut self, task: Task) {
        let task_id = task.id.clone();
        self.tasks.insert(task_id.clone(), task);
        self.persist_task_by_id(&task_id);
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

        self.persist_task_by_id(task_id);
        self.extend_events(emitted);
    }

    fn apply_plan(&mut self, task_id: &str, plan: TaskPlan) -> bool {
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
            if task.status == TaskStatus::Cancelled {
                return false;
            }

            task.set_steps(steps);
            task.user_goal = plan.goal;
            task.touch();
        } else {
            return false;
        }

        self.persist_task_by_id(task_id);
        self.push_event(TaskEvent::new(
            task_id.to_string(),
            None,
            TaskEventKind::Planned,
            format!("Planner 已生成 {step_count} 个步骤。"),
            serde_json::json!({ "stepCount": step_count }),
        ));

        true
    }

    fn advance_task(&mut self, task_id: &str) -> Vec<StepDispatch> {
        let mut dispatches = Vec::new();
        let mut emitted = Vec::new();

        loop {
            let Some(task) = self.tasks.get_mut(task_id) else {
                break;
            };

            if matches!(
                task.status,
                TaskStatus::Failed | TaskStatus::Completed | TaskStatus::Cancelled
            ) {
                break;
            }

            task.status = TaskStatus::Running;
            task.touch();

            let ready_indices = ready_step_indices(task);
            if ready_indices.is_empty() {
                break;
            }

            let mut completed_inside_runtime = false;
            for index in ready_indices {
                {
                    let step = &mut task.steps[index];
                    step.start();

                    emitted.push(TaskEvent::new(
                        task_id.to_string(),
                        Some(step.id.clone()),
                        TaskEventKind::StepStarted,
                        format!("{} 开始执行：{}", step.agent_id, step.title),
                        serde_json::json!({
                            "agentId": step.agent_id,
                            "instruction": step.instruction,
                            "attempt": step.attempts,
                        }),
                    ));
                }

                let context = build_step_dispatch_context(task, index);
                let step = &mut task.steps[index];

                if let Some(result) = execute_runtime_step(step) {
                    step.complete(result.clone());
                    completed_inside_runtime = true;

                    emitted.push(TaskEvent::new(
                        task_id.to_string(),
                        Some(step.id.clone()),
                        TaskEventKind::StepCompleted,
                        format!("{} 已完成步骤。", step.agent_id),
                        result,
                    ));
                } else {
                    dispatches.push(StepDispatch {
                        task_id: task_id.to_string(),
                        step_id: step.id.clone(),
                        agent_name: step.agent_id.clone(),
                        instruction: step.instruction.clone(),
                        context,
                    });
                }
            }

            if !completed_inside_runtime {
                break;
            }
        }

        self.complete_task_if_ready(task_id, &mut emitted);
        self.extend_events(emitted);
        self.persist_task_by_id(task_id);
        dispatches
    }

    fn complete_running_step(
        &mut self,
        task_id: &str,
        agent_name: &str,
        content: &str,
        context: serde_json::Value,
    ) -> Vec<StepDispatch> {
        let mut emitted = Vec::new();

        if let Some(task) = self.tasks.get_mut(task_id) {
            if task.status == TaskStatus::Cancelled {
                return Vec::new();
            }

            let context_step_id = context_step_id(&context);
            let context_step_attempt = context_step_attempt(&context);
            if let Some(step) =
                find_running_step_mut(task, agent_name, context_step_id, context_step_attempt)
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
        }

        self.complete_task_if_ready(task_id, &mut emitted);

        self.extend_events(emitted);
        self.persist_task_by_id(task_id);
        self.advance_task(task_id)
    }

    fn cancel_task(&mut self, task_id: &str, reason: &str) -> Option<Task> {
        let mut emitted = Vec::new();
        let task = self.tasks.get_mut(task_id)?;

        if task.status == TaskStatus::Completed
            || task.status == TaskStatus::Failed
            || task.status == TaskStatus::Cancelled
        {
            return Some(task.clone());
        }

        for step in &mut task.steps {
            if matches!(step.status, StepStatus::Pending | StepStatus::Running) {
                step.status = StepStatus::Skipped;
                step.error = Some(reason.to_string());
                step.completed_at = Some(chrono::Utc::now());
                emitted.push(TaskEvent::new(
                    task_id.to_string(),
                    Some(step.id.clone()),
                    TaskEventKind::StepFailed,
                    format!("{} 已跳过：任务取消。", step.agent_id),
                    serde_json::json!({
                        "agentId": step.agent_id,
                        "reason": reason,
                    }),
                ));
            }
        }

        task.cancel(reason);
        let cancelled = task.clone();

        emitted.push(TaskEvent::new(
            task_id.to_string(),
            None,
            TaskEventKind::Cancelled,
            format!("任务已取消：{reason}"),
            serde_json::json!({ "reason": reason }),
        ));

        self.extend_events(emitted);
        self.persist_task(&cancelled);
        Some(cancelled)
    }

    fn retry_task(&mut self, task_id: &str, reason: &str) -> Option<RetryTaskResult> {
        let needs_planner;
        {
            let task = self.tasks.get_mut(task_id)?;

            if !matches!(task.status, TaskStatus::Failed | TaskStatus::Cancelled) {
                return Some(RetryTaskResult {
                    task: task.clone(),
                    dispatches: Vec::new(),
                    needs_planner: false,
                });
            }

            for step in &mut task.steps {
                if matches!(
                    step.status,
                    StepStatus::Pending
                        | StepStatus::WaitingApproval
                        | StepStatus::Running
                        | StepStatus::Failed
                        | StepStatus::Skipped
                ) {
                    step.reset_for_retry();
                }
            }

            task.retry();
            needs_planner = task.steps.is_empty();
        }

        self.persist_task_by_id(task_id);
        self.push_event(TaskEvent::new(
            task_id.to_string(),
            None,
            TaskEventKind::Retried,
            format!("任务已重新进入调度：{reason}"),
            serde_json::json!({ "reason": reason }),
        ));

        let dispatches = if needs_planner {
            Vec::new()
        } else {
            self.advance_task(task_id)
        };
        let task = self.get_task(task_id)?;

        Some(RetryTaskResult {
            task,
            dispatches,
            needs_planner,
        })
    }

    fn skip_step(&mut self, task_id: &str, step_id: &str, reason: &str) -> Option<SkipStepResult> {
        {
            let mut emitted = Vec::new();
            let task = self.tasks.get_mut(task_id)?;

            if task.status == TaskStatus::Cancelled {
                return Some(SkipStepResult {
                    task: task.clone(),
                    dispatches: Vec::new(),
                });
            }

            let skipped_step_id = {
                let step = task.steps.iter_mut().find(|step| step.id == step_id)?;
                if matches!(step.status, StepStatus::Completed | StepStatus::Skipped) {
                    return Some(SkipStepResult {
                        task: task.clone(),
                        dispatches: Vec::new(),
                    });
                }

                step.status = StepStatus::Skipped;
                step.error = Some(reason.to_string());
                step.completed_at = Some(chrono::Utc::now());
                step.result = Some(serde_json::json!({
                    "skipped": true,
                    "reason": reason,
                }));
                step.id.clone()
            };

            if matches!(
                task.status,
                TaskStatus::Failed | TaskStatus::WaitingApproval
            ) {
                task.status = TaskStatus::Running;
                task.error = None;
            }
            task.touch();

            emitted.push(TaskEvent::new(
                task_id.to_string(),
                Some(skipped_step_id.clone()),
                TaskEventKind::StepSkipped,
                format!("步骤已跳过：{reason}"),
                serde_json::json!({
                    "stepId": skipped_step_id,
                    "reason": reason,
                }),
            ));

            self.complete_task_if_ready(task_id, &mut emitted);
            self.extend_events(emitted);
            self.persist_task_by_id(task_id);
        }

        let dispatches = self.advance_task(task_id);
        let task = self.get_task(task_id)?;

        Some(SkipStepResult { task, dispatches })
    }

    fn record_patch_applied(
        &mut self,
        proposal: &PatchProposal,
        result: &PatchApplyResult,
        approval_id: &str,
    ) -> Option<Task> {
        let task_id = proposal.task_id.as_deref()?.trim();
        if task_id.is_empty() {
            return None;
        }

        let artifact = patch_apply_artifact(proposal, result, approval_id);
        let mut emitted = Vec::new();
        let task = self.tasks.get_mut(task_id)?;

        if has_patch_artifact(task, &proposal.id) {
            return Some(task.clone());
        }

        task.add_artifact(artifact.clone());
        let updated = task.clone();

        emitted.push(TaskEvent::new(
            task_id.to_string(),
            proposal.step_id.clone(),
            TaskEventKind::ArtifactCreated,
            format!("补丁已应用：{}", proposal.summary),
            artifact,
        ));

        self.extend_events(emitted);
        self.persist_task(&updated);
        Some(updated)
    }

    fn record_patch_approval_requested(
        &mut self,
        proposal: &PatchProposal,
        approval: &ApprovalRequest,
    ) -> Option<Task> {
        let task_id = proposal.task_id.as_deref()?.trim();
        if task_id.is_empty() {
            return None;
        }

        let artifact = patch_approval_artifact(proposal, approval);
        let task = self.tasks.get_mut(task_id)?;

        if has_patch_kind_artifact(task, "patchApproval", &proposal.id) {
            return Some(task.clone());
        }

        task.add_artifact(artifact.clone());
        if !matches!(
            task.status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        ) {
            task.status = TaskStatus::WaitingApproval;
            task.touch();
        }
        if let Some(step_id) = proposal.step_id.as_deref() {
            if let Some(step) = task.steps.iter_mut().find(|step| step.id == step_id) {
                if matches!(
                    step.status,
                    StepStatus::Pending | StepStatus::Running | StepStatus::WaitingApproval
                ) {
                    step.status = StepStatus::WaitingApproval;
                    step.error = None;
                }
            }
        }
        let updated = task.clone();

        self.push_event(TaskEvent::new(
            task_id.to_string(),
            proposal.step_id.clone(),
            TaskEventKind::ApprovalRequested,
            format!("补丁等待审批：{}", proposal.summary),
            artifact,
        ));
        self.persist_task(&updated);
        Some(updated)
    }

    fn record_patch_approval_resolved(
        &mut self,
        proposal: &PatchProposal,
        approval: &ApprovalRequest,
    ) -> Option<Task> {
        let task_id = proposal.task_id.as_deref()?.trim();
        if task_id.is_empty() {
            return None;
        }

        let artifact = patch_approval_resolved_artifact(proposal, approval);
        let mut emitted = Vec::new();
        let task = self.tasks.get_mut(task_id)?;

        if has_patch_kind_artifact(task, "patchApprovalResolved", &proposal.id) {
            return Some(task.clone());
        }

        task.add_artifact(artifact.clone());
        match approval.status {
            ApprovalStatus::Approved => {
                if task.status == TaskStatus::WaitingApproval {
                    task.status = TaskStatus::Running;
                    task.error = None;
                    task.touch();
                }
                if let Some(step_id) = proposal.step_id.as_deref() {
                    if let Some(step) = task.steps.iter_mut().find(|step| step.id == step_id) {
                        if step.status == StepStatus::WaitingApproval {
                            step.status = StepStatus::Running;
                            step.error = None;
                        }
                    }
                }
            }
            ApprovalStatus::Rejected => {
                let message = format!("补丁审批已拒绝：{}", proposal.summary);
                if let Some(step_id) = proposal.step_id.as_deref() {
                    if let Some(step) = task.steps.iter_mut().find(|step| step.id == step_id) {
                        if matches!(
                            step.status,
                            StepStatus::Pending | StepStatus::WaitingApproval | StepStatus::Running
                        ) {
                            step.fail(message.clone());
                            emitted.push(TaskEvent::new(
                                task_id.to_string(),
                                Some(step.id.clone()),
                                TaskEventKind::StepFailed,
                                message.clone(),
                                artifact.clone(),
                            ));
                        }
                    }
                }
                if !matches!(
                    task.status,
                    TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
                ) {
                    task.fail(message.clone());
                    emitted.push(TaskEvent::new(
                        task_id.to_string(),
                        None,
                        TaskEventKind::Failed,
                        message,
                        artifact.clone(),
                    ));
                }
            }
            ApprovalStatus::Pending | ApprovalStatus::Cancelled => {}
        }
        let updated = task.clone();

        emitted.push(TaskEvent::new(
            task_id.to_string(),
            proposal.step_id.clone(),
            TaskEventKind::ApprovalResolved,
            format!(
                "补丁审批{}：{}",
                approval_status_label(&approval.status),
                proposal.summary
            ),
            artifact,
        ));
        self.extend_events(emitted);
        self.persist_task(&updated);
        Some(updated)
    }

    fn record_command_approval_requested(&mut self, approval: &ApprovalRequest) -> Option<Task> {
        if approval.action_type != "runtime.runProjectCommand" {
            return None;
        }
        let task_id = approval_task_id(approval)?;
        let step_id = approval_step_id(approval);
        let artifact = command_approval_artifact(approval);
        let task = self.tasks.get_mut(task_id)?;

        if has_command_kind_artifact(task, "commandApproval", &approval.id) {
            return Some(task.clone());
        }

        task.add_artifact(artifact.clone());
        if !matches!(
            task.status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        ) {
            task.status = TaskStatus::WaitingApproval;
            task.touch();
        }
        if let Some(step_id) = step_id {
            if let Some(step) = task.steps.iter_mut().find(|step| step.id == step_id) {
                if matches!(
                    step.status,
                    StepStatus::Pending | StepStatus::Running | StepStatus::WaitingApproval
                ) {
                    step.status = StepStatus::WaitingApproval;
                    step.error = None;
                }
            }
        }
        let updated = task.clone();

        self.push_event(TaskEvent::new(
            task_id.to_string(),
            step_id.map(ToString::to_string),
            TaskEventKind::ApprovalRequested,
            format!(
                "项目命令等待审批：{}",
                command_label_from_approval(approval)
            ),
            artifact,
        ));
        self.persist_task(&updated);
        Some(updated)
    }

    fn record_command_approval_resolved(&mut self, approval: &ApprovalRequest) -> Option<Task> {
        if approval.action_type != "runtime.runProjectCommand" {
            return None;
        }
        let task_id = approval_task_id(approval)?;
        let step_id = approval_step_id(approval);
        let artifact = command_approval_resolved_artifact(approval);
        let mut emitted = Vec::new();
        let task = self.tasks.get_mut(task_id)?;

        if has_command_kind_artifact(task, "commandApprovalResolved", &approval.id) {
            return Some(task.clone());
        }

        task.add_artifact(artifact.clone());
        match approval.status {
            ApprovalStatus::Approved => {
                if task.status == TaskStatus::WaitingApproval {
                    task.status = TaskStatus::Running;
                    task.error = None;
                    task.touch();
                }
                if let Some(step_id) = step_id {
                    if let Some(step) = task.steps.iter_mut().find(|step| step.id == step_id) {
                        if step.status == StepStatus::WaitingApproval {
                            step.status = StepStatus::Running;
                            step.error = None;
                        }
                    }
                }
            }
            ApprovalStatus::Rejected => {
                let message = format!(
                    "项目命令审批已拒绝：{}",
                    command_label_from_approval(approval)
                );
                if let Some(step_id) = step_id {
                    if let Some(step) = task.steps.iter_mut().find(|step| step.id == step_id) {
                        if matches!(
                            step.status,
                            StepStatus::Pending | StepStatus::WaitingApproval | StepStatus::Running
                        ) {
                            step.fail(message.clone());
                            emitted.push(TaskEvent::new(
                                task_id.to_string(),
                                Some(step.id.clone()),
                                TaskEventKind::StepFailed,
                                message.clone(),
                                artifact.clone(),
                            ));
                        }
                    }
                }
                if !matches!(
                    task.status,
                    TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
                ) {
                    task.fail(message.clone());
                    emitted.push(TaskEvent::new(
                        task_id.to_string(),
                        None,
                        TaskEventKind::Failed,
                        message,
                        artifact.clone(),
                    ));
                }
            }
            ApprovalStatus::Pending | ApprovalStatus::Cancelled => {}
        }
        let updated = task.clone();

        emitted.push(TaskEvent::new(
            task_id.to_string(),
            step_id.map(ToString::to_string),
            TaskEventKind::ApprovalResolved,
            format!(
                "项目命令审批{}：{}",
                approval_status_label(&approval.status),
                command_label_from_approval(approval)
            ),
            artifact,
        ));
        self.extend_events(emitted);
        self.persist_task(&updated);
        Some(updated)
    }

    fn record_command_run(
        &mut self,
        approval: &ApprovalRequest,
        run: &ProjectCommandRunResponse,
    ) -> Option<Task> {
        if approval.action_type != "runtime.runProjectCommand" {
            return None;
        }
        let task_id = approval_task_id(approval)?;
        let step_id = approval_step_id(approval);
        let artifact = command_run_artifact(approval, run);
        let mut emitted = Vec::new();
        let task = self.tasks.get_mut(task_id)?;

        if has_command_kind_artifact(task, "commandRun", &approval.id) {
            return Some(task.clone());
        }

        task.add_artifact(artifact.clone());
        if !run.success && task.status != TaskStatus::Cancelled {
            let message = format!("项目命令执行失败：{}", run.command);
            if let Some(step_id) = step_id {
                if let Some(step) = task.steps.iter_mut().find(|step| step.id == step_id) {
                    if step.status != StepStatus::Failed {
                        step.fail(message.clone());
                        emitted.push(TaskEvent::new(
                            task_id.to_string(),
                            Some(step.id.clone()),
                            TaskEventKind::StepFailed,
                            message.clone(),
                            artifact.clone(),
                        ));
                    }
                }
            }
            if task.status != TaskStatus::Failed {
                task.fail(message.clone());
                emitted.push(TaskEvent::new(
                    task_id.to_string(),
                    None,
                    TaskEventKind::Failed,
                    message,
                    artifact.clone(),
                ));
            }
        } else if run.success
            && !matches!(
                task.status,
                TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
            )
        {
            if let Some(step_id) = step_id {
                if let Some(step) = task.steps.iter_mut().find(|step| step.id == step_id) {
                    if matches!(
                        step.status,
                        StepStatus::Pending | StepStatus::WaitingApproval | StepStatus::Running
                    ) {
                        let result = serde_json::json!({
                            "approvalId": approval.id,
                            "runId": run.id,
                            "command": run.command,
                            "workingDir": run.working_dir,
                            "success": run.success,
                            "exitCode": run.exit_code,
                        });
                        step.complete(result.clone());
                        emitted.push(TaskEvent::new(
                            task_id.to_string(),
                            Some(step.id.clone()),
                            TaskEventKind::StepCompleted,
                            "已审批项目命令执行通过，步骤已完成。",
                            result,
                        ));
                    }
                }
            }

            if !task.steps.is_empty()
                && task
                    .steps
                    .iter()
                    .all(|step| matches!(step.status, StepStatus::Completed | StepStatus::Skipped))
            {
                let completed_steps = task
                    .steps
                    .iter()
                    .filter(|step| step.status == StepStatus::Completed)
                    .count();
                let skipped_steps = task
                    .steps
                    .iter()
                    .filter(|step| step.status == StepStatus::Skipped)
                    .count();
                let output = completion_output(completed_steps, skipped_steps);
                task.complete(output.clone());
                emitted.push(TaskEvent::new(
                    task_id.to_string(),
                    None,
                    TaskEventKind::Completed,
                    output,
                    serde_json::json!({
                        "completedSteps": completed_steps,
                        "skippedSteps": skipped_steps,
                    }),
                ));
            }
        }
        let updated = task.clone();

        emitted.push(TaskEvent::new(
            task_id.to_string(),
            step_id.map(ToString::to_string),
            TaskEventKind::ArtifactCreated,
            format!(
                "项目命令执行{}：{}",
                command_run_status_label(run),
                run.command
            ),
            artifact,
        ));
        self.extend_events(emitted);
        self.persist_task(&updated);
        Some(updated)
    }

    fn record_patch_verification(
        &mut self,
        proposal: &PatchProposal,
        approval_id: &str,
        runs: &[ProjectCommandRunResponse],
        errors: &[serde_json::Value],
        auto_rollback: Option<&PatchAutoRollbackResult>,
    ) -> Option<Task> {
        let task_id = proposal.task_id.as_deref()?.trim();
        if task_id.is_empty() {
            return None;
        }

        let artifact =
            patch_verification_artifact(proposal, approval_id, runs, errors, auto_rollback);
        let mut emitted = Vec::new();
        let task = self.tasks.get_mut(task_id)?;

        if has_patch_kind_artifact(task, "patchVerification", &proposal.id) {
            return Some(task.clone());
        }

        let status = artifact
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("skipped");
        task.add_artifact(artifact.clone());
        if status == "failed" && task.status != TaskStatus::Cancelled {
            let message = patch_verification_failure_message(proposal, auto_rollback);
            if let Some(step_id) = proposal.step_id.as_deref() {
                if let Some(step) = task.steps.iter_mut().find(|step| step.id == step_id) {
                    if step.status != StepStatus::Failed {
                        step.fail(message.clone());
                        emitted.push(TaskEvent::new(
                            task_id.to_string(),
                            Some(step.id.clone()),
                            TaskEventKind::StepFailed,
                            message.clone(),
                            patch_verification_failure_payload(proposal, status, auto_rollback),
                        ));
                    }
                }
            }
            if task.status != TaskStatus::Failed {
                task.fail(message.clone());
                emitted.push(TaskEvent::new(
                    task_id.to_string(),
                    None,
                    TaskEventKind::Failed,
                    message,
                    patch_verification_failure_payload(proposal, status, auto_rollback),
                ));
            }
        } else if !matches!(
            task.status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        ) {
            if let Some(step_id) = proposal.step_id.as_deref() {
                if let Some(step) = task.steps.iter_mut().find(|step| step.id == step_id) {
                    if matches!(
                        step.status,
                        StepStatus::Pending | StepStatus::WaitingApproval | StepStatus::Running
                    ) {
                        let result = serde_json::json!({
                            "patchId": proposal.id,
                            "approvalId": approval_id,
                            "summary": proposal.summary,
                            "verificationStatus": status,
                        });
                        step.complete(result.clone());
                        emitted.push(TaskEvent::new(
                            task_id.to_string(),
                            Some(step.id.clone()),
                            TaskEventKind::StepCompleted,
                            format!(
                                "补丁验证{}，步骤已恢复并完成。",
                                patch_verification_status_label(status)
                            ),
                            result,
                        ));
                    }
                }
            }

            if !task.steps.is_empty()
                && task
                    .steps
                    .iter()
                    .all(|step| matches!(step.status, StepStatus::Completed | StepStatus::Skipped))
            {
                let completed_steps = task
                    .steps
                    .iter()
                    .filter(|step| step.status == StepStatus::Completed)
                    .count();
                let skipped_steps = task
                    .steps
                    .iter()
                    .filter(|step| step.status == StepStatus::Skipped)
                    .count();
                let output = completion_output(completed_steps, skipped_steps);
                task.complete(output.clone());
                emitted.push(TaskEvent::new(
                    task_id.to_string(),
                    None,
                    TaskEventKind::Completed,
                    output,
                    serde_json::json!({
                        "completedSteps": completed_steps,
                        "skippedSteps": skipped_steps,
                    }),
                ));
            }
        }
        let updated = task.clone();

        emitted.push(TaskEvent::new(
            task_id.to_string(),
            proposal.step_id.clone(),
            TaskEventKind::ArtifactCreated,
            format!(
                "补丁验证{}：{}",
                patch_verification_status_label(status),
                proposal.summary
            ),
            artifact,
        ));

        self.extend_events(emitted);
        self.persist_task(&updated);
        Some(updated)
    }

    fn record_patch_reverted(
        &mut self,
        proposal: &PatchProposal,
        result: &PatchRevertResult,
    ) -> Option<Task> {
        let task_id = proposal.task_id.as_deref()?.trim();
        if task_id.is_empty() {
            return None;
        }

        let artifact = patch_revert_artifact(proposal, result);
        let mut emitted = Vec::new();
        let task = self.tasks.get_mut(task_id)?;

        if has_patch_kind_artifact(task, "patchReverted", &proposal.id) {
            return Some(task.clone());
        }

        task.add_artifact(artifact.clone());
        let updated = task.clone();

        emitted.push(TaskEvent::new(
            task_id.to_string(),
            proposal.step_id.clone(),
            TaskEventKind::ArtifactCreated,
            format!("补丁已回滚：{}", proposal.summary),
            artifact,
        ));

        self.extend_events(emitted);
        self.persist_task(&updated);
        Some(updated)
    }

    fn fail_running_step(
        &mut self,
        task_id: &str,
        agent_name: &str,
        error: &str,
        context: Option<&serde_json::Value>,
    ) {
        let mut emitted = Vec::new();

        if let Some(task) = self.tasks.get_mut(task_id) {
            if task.status == TaskStatus::Cancelled {
                return;
            }

            let context_step_id = context.and_then(context_step_id);
            let context_step_attempt = context.and_then(context_step_attempt);
            let mut matched_step = false;
            if let Some(step) =
                find_running_step_mut(task, agent_name, context_step_id, context_step_attempt)
            {
                matched_step = true;
                step.fail(error);
                emitted.push(TaskEvent::new(
                    task_id.to_string(),
                    Some(step.id.clone()),
                    TaskEventKind::StepFailed,
                    error.to_string(),
                    serde_json::json!({ "agentId": agent_name }),
                ));
            }

            let should_fail_task =
                matched_step || (context_step_id.is_none() && task.steps.is_empty());
            if should_fail_task {
                task.fail(error);
                emitted.push(TaskEvent::new(
                    task_id.to_string(),
                    None,
                    TaskEventKind::Failed,
                    error.to_string(),
                    serde_json::Value::Null,
                ));
            }
        }

        self.extend_events(emitted);
        self.persist_task_by_id(task_id);
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
        self.persist_event(&event);
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

    fn persist_task_by_id(&self, task_id: &str) {
        if let Some(task) = self.tasks.get(task_id) {
            self.persist_task(task);
        }
    }

    fn persist_task(&self, task: &Task) {
        let Some(store) = &self.store else {
            return;
        };

        if let Err(err) = store.save_task(task) {
            tracing::warn!("[TaskRuntime] 保存任务 [{}] 失败: {err}", task.id);
        }
    }

    fn persist_event(&self, event: &TaskEvent) {
        let Some(store) = &self.store else {
            return;
        };

        if let Err(err) = store.append_event(event) {
            tracing::warn!("[TaskRuntime] 保存任务事件 [{}] 失败: {err}", event.id);
        }
    }

    fn complete_task_if_ready(&mut self, task_id: &str, emitted: &mut Vec<TaskEvent>) {
        let Some(task) = self.tasks.get_mut(task_id) else {
            return;
        };

        if task.steps.is_empty()
            || task.status == TaskStatus::Completed
            || task.status == TaskStatus::Failed
            || task.status == TaskStatus::Cancelled
        {
            return;
        }

        if task
            .steps
            .iter()
            .all(|step| matches!(step.status, StepStatus::Completed | StepStatus::Skipped))
        {
            let completed_steps = task
                .steps
                .iter()
                .filter(|step| step.status == StepStatus::Completed)
                .count();
            let skipped_steps = task
                .steps
                .iter()
                .filter(|step| step.status == StepStatus::Skipped)
                .count();
            let output = completion_output(completed_steps, skipped_steps);
            task.complete(output.clone());
            emitted.push(TaskEvent::new(
                task_id.to_string(),
                None,
                TaskEventKind::Completed,
                output,
                serde_json::json!({
                    "completedSteps": completed_steps,
                    "skippedSteps": skipped_steps,
                }),
            ));
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

    /// 创建启用任务持久化的调度器。
    pub fn with_task_store(
        bus: Arc<MessageBus>,
        db_path: impl Into<String>,
    ) -> Result<Self, AgentError> {
        let store = Arc::new(TaskStore::open(db_path)?);
        Ok(Self {
            agents: HashMap::new(),
            bus,
            runtime: Arc::new(Mutex::new(TaskRuntime::with_store(store))),
            event_loop_started: false,
        })
    }

    /// 注册所有内置 Agent 并启动其运行循环
    ///
    /// 内置 Agent 包括：Echo、Planner、Executor、Memory、Tool。
    pub async fn register_builtin_agents(&mut self) {
        tracing::info!("[Orchestrator] 正在注册内置 Agent...");

        self.register_and_spawn(Box::new(EchoAgent::new())).await;
        self.register_and_spawn(Box::new(PlannerAgent::from_env()))
            .await;
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
        let agent_senders = Arc::new(
            self.agents
                .iter()
                .map(|(name, runtime)| (name.clone(), runtime.sender.clone()))
                .collect::<HashMap<_, _>>(),
        );

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        handle_agent_message(runtime.clone(), agent_senders.clone(), msg).await
                    }
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
        self.submit_task_to_agent_with_context(
            agent_name,
            user_input,
            msg_type,
            serde_json::Value::Null,
        )
        .await
    }

    /// 提交用户任务到指定 Agent，并附带请求级上下文。
    pub async fn submit_task_to_agent_with_context(
        &mut self,
        agent_name: &str,
        user_input: &str,
        msg_type: &str,
        extra_context: serde_json::Value,
    ) -> Result<String, AgentError> {
        self.submit_task_to_agent_with_context_and_transient(
            agent_name,
            user_input,
            msg_type,
            extra_context,
            serde_json::Value::Null,
        )
        .await
    }

    /// 提交用户任务到指定 Agent，并附带请求级上下文和进程内临时上下文。
    pub async fn submit_task_to_agent_with_context_and_transient(
        &mut self,
        agent_name: &str,
        user_input: &str,
        msg_type: &str,
        extra_context: serde_json::Value,
        transient_context: serde_json::Value,
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
            merge_message_context(build_planner_message_context(user_input), extra_context)
        } else {
            extra_context
        };

        let msg = AgentMessage::new("Orchestrator", agent_name, user_input)
            .with_task_id(&task_id)
            .with_type(msg_type)
            .with_context(context)
            .with_transient_context(transient_context);

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

    /// 记录已应用补丁为任务 artifact 和事件。
    pub async fn record_patch_applied(
        &self,
        proposal: &PatchProposal,
        result: &PatchApplyResult,
        approval_id: &str,
    ) -> Option<Task> {
        let mut runtime = self.runtime.lock().await;
        runtime.record_patch_applied(proposal, result, approval_id)
    }

    /// 记录补丁审批请求，并让关联任务/步骤进入等待审批。
    pub async fn record_patch_approval_requested(
        &self,
        proposal: &PatchProposal,
        approval: &ApprovalRequest,
    ) -> Option<Task> {
        let mut runtime = self.runtime.lock().await;
        runtime.record_patch_approval_requested(proposal, approval)
    }

    /// 记录补丁审批决策，并恢复或失败关联任务/步骤。
    pub async fn record_patch_approval_resolved(
        &self,
        proposal: &PatchProposal,
        approval: &ApprovalRequest,
    ) -> Option<Task> {
        let mut runtime = self.runtime.lock().await;
        runtime.record_patch_approval_resolved(proposal, approval)
    }

    /// 记录项目命令审批请求，并让关联任务/步骤进入等待审批。
    pub async fn record_command_approval_requested(
        &self,
        approval: &ApprovalRequest,
    ) -> Option<Task> {
        let mut runtime = self.runtime.lock().await;
        runtime.record_command_approval_requested(approval)
    }

    /// 记录项目命令审批决策，并恢复或失败关联任务/步骤。
    pub async fn record_command_approval_resolved(
        &self,
        approval: &ApprovalRequest,
    ) -> Option<Task> {
        let mut runtime = self.runtime.lock().await;
        runtime.record_command_approval_resolved(approval)
    }

    /// 记录已审批项目命令运行结果为任务 artifact 和事件。
    pub async fn record_command_run(
        &self,
        approval: &ApprovalRequest,
        run: &ProjectCommandRunResponse,
    ) -> Option<Task> {
        let mut runtime = self.runtime.lock().await;
        runtime.record_command_run(approval, run)
    }

    /// 记录补丁应用后的自动验证结果为任务 artifact 和事件。
    pub async fn record_patch_verification(
        &self,
        proposal: &PatchProposal,
        approval_id: &str,
        runs: &[ProjectCommandRunResponse],
        errors: &[serde_json::Value],
        auto_rollback: Option<&PatchAutoRollbackResult>,
    ) -> Option<Task> {
        let mut runtime = self.runtime.lock().await;
        runtime.record_patch_verification(proposal, approval_id, runs, errors, auto_rollback)
    }

    /// 记录已回滚补丁为任务 artifact 和事件。
    pub async fn record_patch_reverted(
        &self,
        proposal: &PatchProposal,
        result: &PatchRevertResult,
    ) -> Option<Task> {
        let mut runtime = self.runtime.lock().await;
        runtime.record_patch_reverted(proposal, result)
    }

    /// 取消任务。
    pub async fn cancel_task(&self, task_id: &str, reason: &str) -> Option<Task> {
        let mut runtime = self.runtime.lock().await;
        runtime.cancel_task(task_id, reason)
    }

    /// 重试失败或已取消的任务。
    pub async fn retry_task(&self, task_id: &str, reason: &str) -> Option<Task> {
        let retry = {
            let mut runtime = self.runtime.lock().await;
            runtime.retry_task(task_id, reason)
        }?;

        if retry.needs_planner {
            let context = build_planner_message_context(&retry.task.user_goal);
            let msg = AgentMessage::new("Orchestrator", "Planner", &retry.task.user_goal)
                .with_task_id(task_id)
                .with_type("plan_request")
                .with_context(context);

            if let Err(err) = self.send_to_agent("Planner", msg) {
                let mut runtime = self.runtime.lock().await;
                runtime.fail_running_step(task_id, "Planner", &format!("{}", err), None);
            }
        } else {
            dispatch_steps(self.runtime.clone(), self.agent_senders(), retry.dispatches).await;
        }

        let runtime = self.runtime.lock().await;
        runtime.get_task(task_id)
    }

    /// 跳过单个步骤，并继续调度依赖已满足的后续步骤。
    pub async fn skip_task_step(&self, task_id: &str, step_id: &str, reason: &str) -> Option<Task> {
        let skipped = {
            let mut runtime = self.runtime.lock().await;
            runtime.skip_step(task_id, step_id, reason)
        }?;

        dispatch_steps(
            self.runtime.clone(),
            self.agent_senders(),
            skipped.dispatches,
        )
        .await;

        let runtime = self.runtime.lock().await;
        runtime.get_task(&skipped.task.id)
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

    fn agent_senders(&self) -> Arc<HashMap<String, UnboundedSender<AgentMessage>>> {
        Arc::new(
            self.agents
                .iter()
                .map(|(name, runtime)| (name.clone(), runtime.sender.clone()))
                .collect(),
        )
    }
}

fn merge_message_context(base: serde_json::Value, extra: serde_json::Value) -> serde_json::Value {
    match (base, extra) {
        (base, serde_json::Value::Null) => base,
        (serde_json::Value::Null, extra) => extra,
        (serde_json::Value::Object(mut base), serde_json::Value::Object(extra)) => {
            for (key, value) in extra {
                base.insert(key, value);
            }
            serde_json::Value::Object(base)
        }
        (base, extra) => serde_json::json!({
            "base": base,
            "extra": extra,
        }),
    }
}

async fn handle_agent_message(
    runtime: Arc<Mutex<TaskRuntime>>,
    agent_senders: Arc<HashMap<String, UnboundedSender<AgentMessage>>>,
    msg: AgentMessage,
) {
    let Some(task_id) = msg.task_id.clone() else {
        return;
    };

    match msg.msg_type.as_str() {
        "plan_created" => match serde_json::from_value::<TaskPlan>(msg.context.clone()) {
            Ok(plan) => {
                let dispatches = {
                    let mut runtime = runtime.lock().await;
                    if runtime.apply_plan(&task_id, plan) {
                        runtime.advance_task(&task_id)
                    } else {
                        Vec::new()
                    }
                };
                dispatch_steps(runtime, agent_senders, dispatches).await;
            }
            Err(err) => {
                let mut runtime = runtime.lock().await;
                runtime.fail_running_step(
                    &task_id,
                    "Planner",
                    &format!("计划解析失败: {err}"),
                    Some(&msg.context),
                );
            }
        },
        "execution_result" | "tool_result" | "memory_ack" | "memory_stored"
        | "memory_retrieved" | "echo_reply" => {
            let dispatches = {
                let mut runtime = runtime.lock().await;
                runtime.complete_running_step(&task_id, &msg.from, &msg.content, msg.context)
            };
            dispatch_steps(runtime, agent_senders, dispatches).await;
        }
        "execution_error" | "tool_error" => {
            let mut runtime = runtime.lock().await;
            runtime.fail_running_step(&task_id, &msg.from, &msg.content, Some(&msg.context));
        }
        _ => {}
    }
}

async fn dispatch_steps(
    runtime: Arc<Mutex<TaskRuntime>>,
    agent_senders: Arc<HashMap<String, UnboundedSender<AgentMessage>>>,
    dispatches: Vec<StepDispatch>,
) {
    for dispatch in dispatches {
        let msg = AgentMessage::new("Orchestrator", &dispatch.agent_name, &dispatch.instruction)
            .with_task_id(&dispatch.task_id)
            .with_type("plan_step")
            .with_context(dispatch.context.clone());

        let result = match agent_senders.get(&dispatch.agent_name) {
            Some(sender) => sender.send(msg).map_err(|err| {
                format!(
                    "无法向 Agent [{}] 分派步骤 [{}]: {}",
                    dispatch.agent_name, dispatch.step_id, err
                )
            }),
            None => Err(format!(
                "无法分派步骤 [{}]，Agent [{}] 未注册",
                dispatch.step_id, dispatch.agent_name
            )),
        };

        if let Err(error) = result {
            let mut runtime = runtime.lock().await;
            runtime.fail_running_step(
                &dispatch.task_id,
                &dispatch.agent_name,
                &error,
                Some(&dispatch.context),
            );
        }
    }
}

fn context_step_id(context: &serde_json::Value) -> Option<&str> {
    context.get("stepId").and_then(|value| value.as_str())
}

fn context_step_attempt(context: &serde_json::Value) -> Option<u32> {
    context
        .get("stepAttempt")
        .and_then(|value| value.as_u64())
        .and_then(|value| u32::try_from(value).ok())
}

fn find_running_step_mut<'a>(
    task: &'a mut Task,
    agent_name: &str,
    step_id: Option<&str>,
    step_attempt: Option<u32>,
) -> Option<&'a mut TaskStep> {
    if let Some(step_id) = step_id {
        return task.steps.iter_mut().find(|step| {
            step.status == StepStatus::Running
                && step.id == step_id
                && step_attempt.is_none_or(|attempt| step.attempts == attempt)
        });
    }

    task.steps.iter_mut().find(|step| {
        step.status == StepStatus::Running
            && step.agent_id == agent_name
            && step_attempt.is_none_or(|attempt| step.attempts == attempt)
    })
}

fn ready_step_indices(task: &Task) -> Vec<usize> {
    let mut indices = task
        .steps
        .iter()
        .enumerate()
        .filter_map(|(index, step)| {
            if step.status != StepStatus::Pending {
                return None;
            }

            let dependencies_completed = step.depends_on.iter().all(|dep_id| {
                task.steps.iter().any(|candidate| {
                    candidate.id == *dep_id
                        && matches!(
                            candidate.status,
                            StepStatus::Completed | StepStatus::Skipped
                        )
                })
            });

            dependencies_completed.then_some(index)
        })
        .collect::<Vec<_>>();

    indices.sort_by_key(|index| task.steps[*index].order);
    indices
}

fn completion_output(completed_steps: usize, skipped_steps: usize) -> String {
    if skipped_steps == 0 {
        format!("任务执行调度器已完成，共完成 {completed_steps} 个步骤。")
    } else {
        format!(
            "任务执行调度器已完成，共完成 {completed_steps} 个步骤，跳过 {skipped_steps} 个步骤。"
        )
    }
}

fn build_step_dispatch_context(task: &Task, step_index: usize) -> serde_json::Value {
    let step = &task.steps[step_index];
    let dependency_results = step
        .depends_on
        .iter()
        .filter_map(|dep_id| {
            task.steps
                .iter()
                .find(|candidate| candidate.id == *dep_id)
                .map(|dep_step| {
                    serde_json::json!({
                        "stepId": dep_step.id,
                        "agentId": dep_step.agent_id,
                        "instruction": dep_step.instruction,
                        "result": dep_step.result,
                    })
                })
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "taskId": task.id,
        "taskGoal": task.user_goal,
        "stepId": step.id,
        "stepOrder": step.order,
        "stepAttempt": step.attempts,
        "dependsOn": step.depends_on,
        "dependencyResults": dependency_results,
    })
}

fn patch_apply_artifact(
    proposal: &PatchProposal,
    result: &PatchApplyResult,
    approval_id: &str,
) -> serde_json::Value {
    serde_json::json!({
        "kind": "patchApplied",
        "patchId": &proposal.id,
        "approvalId": approval_id,
        "summary": &proposal.summary,
        "status": &result.status,
        "files": &result.files,
        "appliedAt": result.applied_at,
        "appliedBy": &proposal.applied_by,
        "alreadyApplied": result.already_applied,
        "unifiedDiff": &proposal.unified_diff,
    })
}

fn patch_approval_artifact(
    proposal: &PatchProposal,
    approval: &ApprovalRequest,
) -> serde_json::Value {
    serde_json::json!({
        "kind": "patchApproval",
        "patchId": &proposal.id,
        "approvalId": &approval.id,
        "summary": &proposal.summary,
        "status": &approval.status,
        "requestedBy": &approval.requested_by,
        "createdAt": approval.created_at,
        "unifiedDiff": &proposal.unified_diff,
    })
}

fn patch_approval_resolved_artifact(
    proposal: &PatchProposal,
    approval: &ApprovalRequest,
) -> serde_json::Value {
    serde_json::json!({
        "kind": "patchApprovalResolved",
        "patchId": &proposal.id,
        "approvalId": &approval.id,
        "summary": &proposal.summary,
        "status": &approval.status,
        "decidedBy": &approval.decided_by,
        "decisionNote": &approval.decision_note,
        "decidedAt": approval.decided_at,
    })
}

fn approval_task_id(approval: &ApprovalRequest) -> Option<&str> {
    approval
        .task_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn approval_step_id(approval: &ApprovalRequest) -> Option<&str> {
    approval
        .step_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn command_label_from_approval(approval: &ApprovalRequest) -> String {
    approval
        .action_payload
        .get("command")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&approval.title)
        .to_string()
}

fn command_working_dir_from_approval(approval: &ApprovalRequest) -> Option<String> {
    approval
        .action_payload
        .get("workingDir")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn command_approval_artifact(approval: &ApprovalRequest) -> serde_json::Value {
    serde_json::json!({
        "kind": "commandApproval",
        "approvalId": &approval.id,
        "command": command_label_from_approval(approval),
        "workingDir": command_working_dir_from_approval(approval),
        "status": &approval.status,
        "requestedBy": &approval.requested_by,
        "createdAt": approval.created_at,
    })
}

fn command_approval_resolved_artifact(approval: &ApprovalRequest) -> serde_json::Value {
    serde_json::json!({
        "kind": "commandApprovalResolved",
        "approvalId": &approval.id,
        "command": command_label_from_approval(approval),
        "workingDir": command_working_dir_from_approval(approval),
        "status": &approval.status,
        "decidedBy": &approval.decided_by,
        "decisionNote": &approval.decision_note,
        "decidedAt": approval.decided_at,
    })
}

fn command_run_artifact(
    approval: &ApprovalRequest,
    run: &ProjectCommandRunResponse,
) -> serde_json::Value {
    serde_json::json!({
        "kind": "commandRun",
        "approvalId": &approval.id,
        "runId": &run.id,
        "command": &run.command,
        "workingDir": &run.working_dir,
        "success": run.success,
        "exitCode": run.exit_code,
        "durationMs": run.duration_ms,
        "timedOut": run.timed_out,
        "stdoutTruncated": run.stdout_truncated,
        "stderrTruncated": run.stderr_truncated,
        "stdoutPreview": output_preview(&run.stdout),
        "stderrPreview": output_preview(&run.stderr),
        "createdAt": &run.created_at,
    })
}

fn patch_verification_artifact(
    proposal: &PatchProposal,
    approval_id: &str,
    runs: &[ProjectCommandRunResponse],
    errors: &[serde_json::Value],
    auto_rollback: Option<&PatchAutoRollbackResult>,
) -> serde_json::Value {
    let status = patch_verification_status(runs, errors);
    let success_count = runs.iter().filter(|run| run.success).count();
    let failed_count = runs.iter().filter(|run| !run.success).count() + errors.len();
    let run_summaries = runs
        .iter()
        .map(|run| {
            serde_json::json!({
                "id": &run.id,
                "command": &run.command,
                "workingDir": &run.working_dir,
                "success": run.success,
                "exitCode": run.exit_code,
                "durationMs": run.duration_ms,
                "timedOut": run.timed_out,
                "createdAt": &run.created_at,
            })
        })
        .collect::<Vec<_>>();

    let mut artifact = serde_json::json!({
        "kind": "patchVerification",
        "patchId": &proposal.id,
        "approvalId": approval_id,
        "summary": &proposal.summary,
        "status": status,
        "verifiedAt": chrono::Utc::now(),
        "commandCount": runs.len() + errors.len(),
        "successCount": success_count,
        "failedCount": failed_count,
        "runs": run_summaries,
        "errors": errors,
    });
    if let Some(auto_rollback) = auto_rollback {
        artifact["autoRollback"] =
            serde_json::to_value(auto_rollback).unwrap_or(serde_json::Value::Null);
    }
    artifact
}

fn patch_revert_artifact(
    proposal: &PatchProposal,
    result: &PatchRevertResult,
) -> serde_json::Value {
    let rollback_diff = proposal
        .files
        .iter()
        .map(|file| {
            crate::workspace::patch::build_unified_diff(
                &file.path,
                &file.new_content,
                &file.old_content,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    serde_json::json!({
        "kind": "patchReverted",
        "patchId": &proposal.id,
        "approvalId": &proposal.approval_id,
        "summary": &proposal.summary,
        "status": &result.status,
        "files": &result.files,
        "revertedAt": result.reverted_at,
        "revertedBy": &proposal.reverted_by,
        "alreadyReverted": result.already_reverted,
        "rollbackDiff": rollback_diff,
    })
}

fn patch_verification_status(
    runs: &[ProjectCommandRunResponse],
    errors: &[serde_json::Value],
) -> &'static str {
    if !errors.is_empty() || runs.iter().any(|run| !run.success) {
        "failed"
    } else if runs.is_empty() {
        "skipped"
    } else {
        "passed"
    }
}

fn patch_verification_status_label(status: &str) -> &'static str {
    match status {
        "passed" => "通过",
        "failed" => "失败",
        _ => "跳过",
    }
}

fn approval_status_label(status: &ApprovalStatus) -> &'static str {
    match status {
        ApprovalStatus::Pending => "等待中",
        ApprovalStatus::Approved => "已通过",
        ApprovalStatus::Rejected => "已拒绝",
        ApprovalStatus::Cancelled => "已取消",
    }
}

fn command_run_status_label(run: &ProjectCommandRunResponse) -> &'static str {
    if run.timed_out {
        "超时"
    } else if run.success {
        "通过"
    } else {
        "失败"
    }
}

fn output_preview(output: &str) -> String {
    const PREVIEW_CHARS: usize = 2_000;
    let mut preview = output.chars().take(PREVIEW_CHARS).collect::<String>();
    if output.chars().nth(PREVIEW_CHARS).is_some() {
        preview.push_str("\n[preview truncated]");
    }
    preview
}

fn patch_verification_failure_message(
    proposal: &PatchProposal,
    auto_rollback: Option<&PatchAutoRollbackResult>,
) -> String {
    let rollback_summary = match auto_rollback {
        Some(result) if result.reverted => "已自动回滚补丁。",
        Some(result) if !result.reverted => "自动回滚失败，需要人工处理。",
        _ => "补丁仍保留在工作区，需要修复或手动回滚。",
    };
    format!("补丁验证失败：{}。{}", proposal.summary, rollback_summary)
}

fn patch_verification_failure_payload(
    proposal: &PatchProposal,
    status: &str,
    auto_rollback: Option<&PatchAutoRollbackResult>,
) -> serde_json::Value {
    serde_json::json!({
        "patchId": &proposal.id,
        "status": status,
        "autoRollback": auto_rollback,
    })
}

fn has_patch_artifact(task: &Task, patch_id: &str) -> bool {
    has_patch_kind_artifact(task, "patchApplied", patch_id)
}

fn has_patch_kind_artifact(task: &Task, kind: &str, patch_id: &str) -> bool {
    task.artifacts.iter().any(|artifact| {
        artifact.get("kind").and_then(serde_json::Value::as_str) == Some(kind)
            && artifact.get("patchId").and_then(serde_json::Value::as_str) == Some(patch_id)
    })
}

fn has_command_kind_artifact(task: &Task, kind: &str, approval_id: &str) -> bool {
    task.artifacts.iter().any(|artifact| {
        artifact.get("kind").and_then(serde_json::Value::as_str) == Some(kind)
            && artifact
                .get("approvalId")
                .and_then(serde_json::Value::as_str)
                == Some(approval_id)
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReadonlyToolInstruction {
    query: String,
    focus_paths: Vec<String>,
}

fn execute_runtime_step(step: &TaskStep) -> Option<serde_json::Value> {
    if step.agent_id == "Planner" {
        return Some(simulated_step_result(step));
    }

    if step.agent_id == "Tool" {
        if let Some(result) = execute_readonly_tool_step(&step.instruction) {
            return Some(result);
        }
    }

    None
}

fn simulated_step_result(step: &TaskStep) -> serde_json::Value {
    serde_json::json!({
        "mode": "simulated",
        "agentId": step.agent_id,
        "summary": format!("{} 已完成模拟执行。", step.agent_id),
        "instruction": step.instruction,
    })
}

#[cfg(test)]
fn execute_task_step_v1(step: &TaskStep) -> serde_json::Value {
    execute_runtime_step(step).unwrap_or_else(|| simulated_step_result(step))
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
        tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;

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
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

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

    #[test]
    fn test_ready_step_indices_waits_for_dependencies() {
        let mut task = Task::new("task-1", "测试依赖调度");
        let mut first = TaskStep::new("task-1-1", "task-1", 1, "Planner", "理解任务", vec![]);
        let second = TaskStep::new(
            "task-1-2",
            "task-1",
            2,
            "Executor",
            "执行任务",
            vec!["task-1-1".to_string()],
        );

        task.set_steps(vec![first.clone(), second]);
        assert_eq!(ready_step_indices(&task), vec![0]);

        first.complete(serde_json::json!({ "ok": true }));
        task.steps[0] = first;
        assert_eq!(ready_step_indices(&task), vec![1]);
    }

    #[test]
    fn test_ready_step_indices_treats_skipped_dependencies_as_ready() {
        let mut task = Task::new("task-skip-ready", "跳过依赖");
        let mut first = TaskStep::new(
            "task-skip-ready-1",
            "task-skip-ready",
            1,
            "Tool",
            "可跳过步骤",
            vec![],
        );
        first.status = StepStatus::Skipped;
        first.completed_at = Some(chrono::Utc::now());
        let second = TaskStep::new(
            "task-skip-ready-2",
            "task-skip-ready",
            2,
            "Executor",
            "继续执行",
            vec!["task-skip-ready-1".to_string()],
        );

        task.set_steps(vec![first, second]);

        assert_eq!(ready_step_indices(&task), vec![1]);
    }

    #[test]
    fn test_skip_step_dispatches_dependent_step() {
        let task_id = "task-skip-dispatch";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(Task::new(task_id, "跳过后继续"));
        runtime.apply_plan(
            task_id,
            TaskPlan {
                task_id: task_id.to_string(),
                goal: "跳过后继续".to_string(),
                steps: vec![
                    crate::agent::planner_agent::PlanStep {
                        step_id: format!("{task_id}-1"),
                        order: 1,
                        agent: "Tool".to_string(),
                        instruction: "等待外部信息".to_string(),
                        depends_on: vec![],
                        status: crate::agent::planner_agent::StepStatus::Pending,
                    },
                    crate::agent::planner_agent::PlanStep {
                        step_id: format!("{task_id}-2"),
                        order: 2,
                        agent: "Executor".to_string(),
                        instruction: "根据现有信息继续".to_string(),
                        depends_on: vec![format!("{task_id}-1")],
                        status: crate::agent::planner_agent::StepStatus::Pending,
                    },
                ],
                created_at: chrono::Utc::now(),
            },
        );

        let result = runtime
            .skip_step(task_id, &format!("{task_id}-1"), "信息不足，跳过")
            .unwrap();

        assert_eq!(result.dispatches.len(), 1);
        assert_eq!(result.dispatches[0].step_id, format!("{task_id}-2"));
        let task = runtime.get_task(task_id).unwrap();
        assert_eq!(task.steps[0].status, StepStatus::Skipped);
        assert_eq!(task.steps[1].status, StepStatus::Running);
        assert!(runtime
            .get_events(task_id)
            .iter()
            .any(|event| event.kind == TaskEventKind::StepSkipped));
    }

    #[test]
    fn test_skip_last_step_completes_task_with_skipped_count() {
        let task_id = "task-skip-complete";
        let mut task = Task::new(task_id, "跳过最后一步");
        let mut first = TaskStep::new(format!("{task_id}-1"), task_id, 1, "Tool", "检索", vec![]);
        first.complete(serde_json::json!({ "ok": true }));
        let mut second = TaskStep::new(
            format!("{task_id}-2"),
            task_id,
            2,
            "Executor",
            "整理输出",
            vec![format!("{task_id}-1")],
        );
        second.start();
        task.steps = vec![first, second];
        task.status = TaskStatus::Running;
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(task);

        let result = runtime
            .skip_step(task_id, &format!("{task_id}-2"), "无需整理")
            .unwrap();

        assert!(result.dispatches.is_empty());
        assert_eq!(result.task.status, TaskStatus::Completed);
        assert_eq!(result.task.steps[1].status, StepStatus::Skipped);
        let completed = runtime
            .get_events(task_id)
            .into_iter()
            .find(|event| event.kind == TaskEventKind::Completed)
            .unwrap();
        assert_eq!(completed.payload["completedSteps"], serde_json::json!(1));
        assert_eq!(completed.payload["skippedSteps"], serde_json::json!(1));
    }

    #[test]
    fn test_step_dispatch_context_includes_dependency_results() {
        let mut task = Task::new("task-1", "测试上下文");
        let mut first = TaskStep::new("task-1-1", "task-1", 1, "Tool", "只读检索", vec![]);
        first.complete(serde_json::json!({ "summary": "found files" }));
        let second = TaskStep::new(
            "task-1-2",
            "task-1",
            2,
            "Executor",
            "整理方案",
            vec!["task-1-1".to_string()],
        );

        task.set_steps(vec![first, second]);
        let context = build_step_dispatch_context(&task, 1);

        assert_eq!(context["stepId"], "task-1-2");
        assert_eq!(context["stepAttempt"], 0);
        assert_eq!(
            context["dependencyResults"][0]["result"]["summary"],
            "found files"
        );
    }

    #[test]
    fn test_task_runtime_loads_persisted_tasks_and_events() {
        let path = std::env::temp_dir().join(format!(
            "rust-mutil-agent-runtime-{}.db",
            uuid::Uuid::new_v4()
        ));
        let path_str = path.to_string_lossy().to_string();

        {
            let store = Arc::new(TaskStore::open(&path_str).unwrap());
            let mut runtime = TaskRuntime::with_store(store);
            runtime.insert_task(Task::new("task-persisted", "恢复任务"));
        }
        {
            let store = Arc::new(TaskStore::open(&path_str).unwrap());
            let runtime = TaskRuntime::with_store(store);
            let task = runtime.get_task("task-persisted").unwrap();
            let events = runtime.get_events("task-persisted");

            assert_eq!(task.user_goal, "恢复任务");
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].kind, TaskEventKind::Created);
        }

        let _ = std::fs::remove_file(path);
    }

    fn patch_proposal_for_task(task_id: &str) -> PatchProposal {
        let now = chrono::Utc::now();
        let diff = crate::workspace::patch::build_unified_diff("README.md", "old", "new");
        PatchProposal {
            id: "patch-task-1".to_string(),
            task_id: Some(task_id.to_string()),
            step_id: Some(format!("{task_id}-1")),
            approval_id: Some("approval-1".to_string()),
            summary: "更新 README".to_string(),
            status: crate::workspace::PatchProposalStatus::Applied,
            files: vec![crate::workspace::patch::PatchFileChange {
                path: "README.md".to_string(),
                change_type: crate::workspace::patch::PatchChangeType::Modify,
                old_content: "old".to_string(),
                new_content: "new".to_string(),
                diff: diff.clone(),
            }],
            unified_diff: diff,
            requested_by: "test".to_string(),
            created_at: now,
            updated_at: now,
            applied_at: Some(now),
            applied_by: Some("tester".to_string()),
            reverted_at: None,
            reverted_by: None,
        }
    }

    fn command_run(command: &str, success: bool) -> ProjectCommandRunResponse {
        ProjectCommandRunResponse {
            id: format!("run-{command}").replace(' ', "-"),
            approval_id: None,
            command: command.to_string(),
            working_dir: "src-tauri".to_string(),
            exit_code: Some(if success { 0 } else { 101 }),
            success,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 12,
            timed_out: false,
            stdout_truncated: false,
            stderr_truncated: false,
            created_at: "2026-06-09T02:00:00Z".to_string(),
        }
    }

    fn approval_for_patch(proposal: &PatchProposal, status: ApprovalStatus) -> ApprovalRequest {
        let mut approval = ApprovalRequest::new(crate::approval::CreateApprovalRequest {
            task_id: proposal.task_id.clone(),
            step_id: proposal.step_id.clone(),
            title: format!("应用补丁：{}", proposal.summary),
            reason: "需要确认补丁 diff。".to_string(),
            risk: crate::agent::action::RiskLevel::High,
            action_type: "workspace.applyPatch".to_string(),
            action_payload: serde_json::json!({ "patchId": proposal.id }),
            requested_by: Some("test".to_string()),
        })
        .unwrap();
        approval.id = proposal
            .approval_id
            .clone()
            .unwrap_or_else(|| "approval-1".to_string());
        if status != ApprovalStatus::Pending {
            approval.status = status;
            approval.decided_by = Some("tester".to_string());
            approval.decided_at = Some(chrono::Utc::now());
            approval.updated_at = approval.decided_at.unwrap();
        }
        approval
    }

    fn approval_for_command(task_id: &str, status: ApprovalStatus) -> ApprovalRequest {
        let mut approval = ApprovalRequest::new(crate::approval::CreateApprovalRequest {
            task_id: Some(task_id.to_string()),
            step_id: Some(format!("{task_id}-1")),
            title: "运行项目命令：cargo clippy".to_string(),
            reason: "需要确认项目命令。".to_string(),
            risk: crate::agent::action::RiskLevel::High,
            action_type: "runtime.runProjectCommand".to_string(),
            action_payload: serde_json::json!({
                "command": "cargo clippy",
                "workingDir": "src-tauri",
                "allowedByDefault": false,
            }),
            requested_by: Some("test".to_string()),
        })
        .unwrap();
        approval.id = "command-approval-1".to_string();
        if status != ApprovalStatus::Pending {
            approval.status = status;
            approval.decided_by = Some("tester".to_string());
            approval.decided_at = Some(chrono::Utc::now());
            approval.updated_at = approval.decided_at.unwrap();
        }
        approval
    }

    fn command_run_for_approval(
        approval: &ApprovalRequest,
        success: bool,
    ) -> ProjectCommandRunResponse {
        ProjectCommandRunResponse {
            approval_id: Some(approval.id.clone()),
            success,
            exit_code: Some(if success { 0 } else { 101 }),
            stderr: if success {
                String::new()
            } else {
                "clippy failed".to_string()
            },
            ..command_run("cargo clippy", success)
        }
    }

    fn task_with_running_executor_step(task_id: &str) -> Task {
        let mut task = Task::new(task_id, "应用补丁审批");
        let mut step = TaskStep::new(
            format!("{task_id}-1"),
            task_id,
            1,
            "Executor",
            "应用补丁",
            vec![],
        );
        step.start();
        task.steps = vec![step];
        task.status = TaskStatus::Running;
        task
    }

    #[test]
    fn test_record_command_approval_requested_marks_linked_step_waiting() {
        let task_id = "task-command-approval-requested";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(task_with_running_executor_step(task_id));
        let approval = approval_for_command(task_id, ApprovalStatus::Pending);

        let task = runtime
            .record_command_approval_requested(&approval)
            .unwrap();

        assert_eq!(task.status, TaskStatus::WaitingApproval);
        assert_eq!(task.steps[0].status, StepStatus::WaitingApproval);
        assert!(task.artifacts.iter().any(|artifact| {
            artifact["kind"] == serde_json::json!("commandApproval")
                && artifact["approvalId"] == serde_json::json!(approval.id)
        }));
        assert!(runtime
            .get_events(task_id)
            .iter()
            .any(|event| event.kind == TaskEventKind::ApprovalRequested));
    }

    #[test]
    fn test_record_command_approval_approved_restores_running_step() {
        let task_id = "task-command-approval-approved";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(task_with_running_executor_step(task_id));
        let pending = approval_for_command(task_id, ApprovalStatus::Pending);
        runtime.record_command_approval_requested(&pending);
        let approved = approval_for_command(task_id, ApprovalStatus::Approved);

        let task = runtime.record_command_approval_resolved(&approved).unwrap();

        assert_eq!(task.status, TaskStatus::Running);
        assert_eq!(task.steps[0].status, StepStatus::Running);
        assert!(task.artifacts.iter().any(|artifact| {
            artifact["kind"] == serde_json::json!("commandApprovalResolved")
                && artifact["status"] == serde_json::json!("approved")
        }));
        assert!(runtime
            .get_events(task_id)
            .iter()
            .any(|event| event.kind == TaskEventKind::ApprovalResolved));
    }

    #[test]
    fn test_record_command_approval_rejected_marks_task_retryable() {
        let task_id = "task-command-approval-rejected";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(task_with_running_executor_step(task_id));
        let pending = approval_for_command(task_id, ApprovalStatus::Pending);
        runtime.record_command_approval_requested(&pending);
        let rejected = approval_for_command(task_id, ApprovalStatus::Rejected);

        let task = runtime.record_command_approval_resolved(&rejected).unwrap();

        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(task.steps[0].status, StepStatus::Failed);
        assert!(runtime
            .get_events(task_id)
            .iter()
            .any(|event| event.kind == TaskEventKind::Failed));

        let retry = runtime.retry_task(task_id, "重新提交命令审批").unwrap();
        let task = runtime.get_task(task_id).unwrap();

        assert_eq!(retry.dispatches.len(), 1);
        assert_eq!(task.status, TaskStatus::Running);
        assert_eq!(task.steps[0].status, StepStatus::Running);
    }

    #[test]
    fn test_record_command_run_success_completes_linked_step() {
        let task_id = "task-command-run-success";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(task_with_running_executor_step(task_id));
        let pending = approval_for_command(task_id, ApprovalStatus::Pending);
        runtime.record_command_approval_requested(&pending);
        let approved = approval_for_command(task_id, ApprovalStatus::Approved);
        runtime.record_command_approval_resolved(&approved);
        let run = command_run_for_approval(&approved, true);

        let task = runtime.record_command_run(&approved, &run).unwrap();

        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(task.steps[0].status, StepStatus::Completed);
        assert!(task.artifacts.iter().any(|artifact| {
            artifact["kind"] == serde_json::json!("commandRun")
                && artifact["runId"] == serde_json::json!(run.id)
        }));
        assert!(runtime
            .get_events(task_id)
            .iter()
            .any(|event| event.kind == TaskEventKind::StepCompleted));
    }

    #[test]
    fn test_record_command_run_failure_marks_linked_step_retryable() {
        let task_id = "task-command-run-failure";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(task_with_running_executor_step(task_id));
        let pending = approval_for_command(task_id, ApprovalStatus::Pending);
        runtime.record_command_approval_requested(&pending);
        let approved = approval_for_command(task_id, ApprovalStatus::Approved);
        runtime.record_command_approval_resolved(&approved);
        let run = command_run_for_approval(&approved, false);

        let task = runtime.record_command_run(&approved, &run).unwrap();

        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(task.steps[0].status, StepStatus::Failed);
        assert!(runtime
            .get_events(task_id)
            .iter()
            .any(|event| event.kind == TaskEventKind::StepFailed));

        let retry = runtime.retry_task(task_id, "修复命令失败").unwrap();
        let task = runtime.get_task(task_id).unwrap();

        assert_eq!(retry.dispatches.len(), 1);
        assert_eq!(task.status, TaskStatus::Running);
        assert_eq!(task.steps[0].status, StepStatus::Running);
    }

    #[test]
    fn test_record_command_run_is_idempotent_for_same_approval() {
        let task_id = "task-command-run-idempotent";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(task_with_running_executor_step(task_id));
        let approved = approval_for_command(task_id, ApprovalStatus::Approved);
        let run = command_run_for_approval(&approved, true);

        runtime.record_command_run(&approved, &run).unwrap();
        runtime.record_command_run(&approved, &run).unwrap();

        let task = runtime.get_task(task_id).unwrap();
        assert_eq!(task.artifacts.len(), 1);
        assert_eq!(
            runtime
                .get_events(task_id)
                .iter()
                .filter(|event| event.kind == TaskEventKind::ArtifactCreated)
                .count(),
            1
        );
    }

    #[test]
    fn test_record_patch_approval_requested_marks_linked_step_waiting() {
        let task_id = "task-patch-approval-requested";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(task_with_running_executor_step(task_id));
        let proposal = patch_proposal_for_task(task_id);
        let approval = approval_for_patch(&proposal, ApprovalStatus::Pending);

        let task = runtime
            .record_patch_approval_requested(&proposal, &approval)
            .unwrap();

        assert_eq!(task.status, TaskStatus::WaitingApproval);
        assert_eq!(task.steps[0].status, StepStatus::WaitingApproval);
        assert!(task.artifacts.iter().any(|artifact| {
            artifact["kind"] == serde_json::json!("patchApproval")
                && artifact["patchId"] == serde_json::json!(proposal.id)
        }));
        assert!(runtime
            .get_events(task_id)
            .iter()
            .any(|event| event.kind == TaskEventKind::ApprovalRequested));
    }

    #[test]
    fn test_record_patch_approval_approved_restores_running_step() {
        let task_id = "task-patch-approval-approved";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(task_with_running_executor_step(task_id));
        let proposal = patch_proposal_for_task(task_id);
        let pending = approval_for_patch(&proposal, ApprovalStatus::Pending);
        runtime.record_patch_approval_requested(&proposal, &pending);
        let approved = approval_for_patch(&proposal, ApprovalStatus::Approved);

        let task = runtime
            .record_patch_approval_resolved(&proposal, &approved)
            .unwrap();

        assert_eq!(task.status, TaskStatus::Running);
        assert_eq!(task.steps[0].status, StepStatus::Running);
        assert!(task.artifacts.iter().any(|artifact| {
            artifact["kind"] == serde_json::json!("patchApprovalResolved")
                && artifact["status"] == serde_json::json!("approved")
        }));
        assert!(runtime
            .get_events(task_id)
            .iter()
            .any(|event| event.kind == TaskEventKind::ApprovalResolved));
    }

    #[test]
    fn test_record_patch_approval_rejected_marks_task_retryable() {
        let task_id = "task-patch-approval-rejected";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(task_with_running_executor_step(task_id));
        let proposal = patch_proposal_for_task(task_id);
        let pending = approval_for_patch(&proposal, ApprovalStatus::Pending);
        runtime.record_patch_approval_requested(&proposal, &pending);
        let rejected = approval_for_patch(&proposal, ApprovalStatus::Rejected);

        let task = runtime
            .record_patch_approval_resolved(&proposal, &rejected)
            .unwrap();

        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(task.steps[0].status, StepStatus::Failed);
        assert!(runtime
            .get_events(task_id)
            .iter()
            .any(|event| event.kind == TaskEventKind::Failed));

        let retry = runtime.retry_task(task_id, "重新提交审批").unwrap();
        let task = runtime.get_task(task_id).unwrap();

        assert_eq!(retry.dispatches.len(), 1);
        assert_eq!(task.status, TaskStatus::Running);
        assert_eq!(task.steps[0].status, StepStatus::Running);
    }

    #[test]
    fn test_record_patch_verification_success_completes_waiting_step() {
        let task_id = "task-patch-verification-complete";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(task_with_running_executor_step(task_id));
        let proposal = patch_proposal_for_task(task_id);
        let pending = approval_for_patch(&proposal, ApprovalStatus::Pending);
        runtime.record_patch_approval_requested(&proposal, &pending);
        let approved = approval_for_patch(&proposal, ApprovalStatus::Approved);
        runtime.record_patch_approval_resolved(&proposal, &approved);
        let runs = vec![command_run("cargo test", true)];

        let task = runtime
            .record_patch_verification(&proposal, "approval-1", &runs, &[], None)
            .unwrap();

        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(task.steps[0].status, StepStatus::Completed);
        assert!(runtime
            .get_events(task_id)
            .iter()
            .any(|event| event.kind == TaskEventKind::StepCompleted));
    }

    #[test]
    fn test_record_patch_applied_adds_task_artifact_and_event() {
        let task_id = "task-patch-artifact";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(Task::new(task_id, "应用补丁到任务"));
        let proposal = patch_proposal_for_task(task_id);
        let result = PatchApplyResult {
            patch_id: proposal.id.clone(),
            status: crate::workspace::PatchProposalStatus::Applied,
            files: vec!["README.md".to_string()],
            applied_at: proposal.applied_at.unwrap(),
            already_applied: false,
            auto_rollback: None,
        };

        let task = runtime
            .record_patch_applied(&proposal, &result, "approval-1")
            .unwrap();

        assert_eq!(task.artifacts.len(), 1);
        assert_eq!(task.artifacts[0]["kind"], serde_json::json!("patchApplied"));
        assert_eq!(
            task.artifacts[0]["patchId"],
            serde_json::json!("patch-task-1")
        );
        assert_eq!(
            task.artifacts[0]["approvalId"],
            serde_json::json!("approval-1")
        );
        assert_eq!(
            task.artifacts[0]["files"][0],
            serde_json::json!("README.md")
        );

        let events = runtime.get_events(task_id);
        let event = events
            .iter()
            .find(|event| event.kind == TaskEventKind::ArtifactCreated)
            .unwrap();
        assert_eq!(event.step_id.as_deref(), Some("task-patch-artifact-1"));
        assert_eq!(event.payload["patchId"], serde_json::json!("patch-task-1"));
    }

    #[test]
    fn test_record_patch_applied_is_idempotent_for_same_patch() {
        let task_id = "task-patch-idempotent";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(Task::new(task_id, "重复应用补丁"));
        let proposal = patch_proposal_for_task(task_id);
        let result = PatchApplyResult {
            patch_id: proposal.id.clone(),
            status: crate::workspace::PatchProposalStatus::Applied,
            files: vec!["README.md".to_string()],
            applied_at: proposal.applied_at.unwrap(),
            already_applied: false,
            auto_rollback: None,
        };

        runtime
            .record_patch_applied(&proposal, &result, "approval-1")
            .unwrap();
        runtime
            .record_patch_applied(&proposal, &result, "approval-1")
            .unwrap();

        let task = runtime.get_task(task_id).unwrap();
        assert_eq!(task.artifacts.len(), 1);
        assert_eq!(
            runtime
                .get_events(task_id)
                .iter()
                .filter(|event| event.kind == TaskEventKind::ArtifactCreated)
                .count(),
            1
        );
    }

    #[test]
    fn test_record_patch_verification_adds_artifact_and_event() {
        let task_id = "task-patch-verification";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(Task::new(task_id, "验证补丁"));
        let proposal = patch_proposal_for_task(task_id);
        let runs = vec![
            command_run("cargo test", true),
            command_run("npm run build", false),
        ];

        let task = runtime
            .record_patch_verification(&proposal, "approval-1", &runs, &[], None)
            .unwrap();

        assert_eq!(task.artifacts.len(), 1);
        assert_eq!(
            task.artifacts[0]["kind"],
            serde_json::json!("patchVerification")
        );
        assert_eq!(task.artifacts[0]["status"], serde_json::json!("failed"));
        assert_eq!(task.artifacts[0]["commandCount"], serde_json::json!(2));
        assert_eq!(task.artifacts[0]["successCount"], serde_json::json!(1));
        assert_eq!(task.artifacts[0]["failedCount"], serde_json::json!(1));
        assert_eq!(task.status, TaskStatus::Failed);
        assert!(task
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("补丁验证失败"));

        let events = runtime.get_events(task_id);
        let event = events
            .iter()
            .find(|event| event.kind == TaskEventKind::ArtifactCreated)
            .unwrap();
        assert!(event.message.contains("补丁验证失败"));
        assert_eq!(
            event.payload["kind"],
            serde_json::json!("patchVerification")
        );
    }

    #[test]
    fn test_record_patch_verification_failure_marks_linked_step_retryable() {
        let task_id = "task-patch-verification-retry";
        let mut task = Task::new(task_id, "验证失败后重试");
        let mut step = TaskStep::new(
            format!("{task_id}-1"),
            task_id,
            1,
            "Executor",
            "应用补丁并验证",
            vec![],
        );
        step.complete(serde_json::json!({ "summary": "patch applied" }));
        task.steps = vec![step];
        task.complete("之前已完成");

        let mut runtime = TaskRuntime::default();
        runtime.insert_task(task);
        let proposal = patch_proposal_for_task(task_id);
        let runs = vec![command_run("cargo test", false)];

        let task = runtime
            .record_patch_verification(&proposal, "approval-1", &runs, &[], None)
            .unwrap();

        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(task.steps[0].status, StepStatus::Failed);
        assert!(runtime
            .get_events(task_id)
            .iter()
            .any(|event| event.kind == TaskEventKind::StepFailed));
        assert!(runtime
            .get_events(task_id)
            .iter()
            .any(|event| event.kind == TaskEventKind::Failed));

        let retry = runtime.retry_task(task_id, "修复验证失败").unwrap();
        let task = runtime.get_task(task_id).unwrap();

        assert_eq!(retry.dispatches.len(), 1);
        assert_eq!(task.status, TaskStatus::Running);
        assert_eq!(task.steps[0].status, StepStatus::Running);
        assert!(task.error.is_none());
    }

    #[test]
    fn test_record_patch_verification_records_auto_rollback_metadata() {
        let task_id = "task-patch-verification-auto-rollback";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(Task::new(task_id, "验证失败自动回滚"));
        let proposal = patch_proposal_for_task(task_id);
        let reverted_at = chrono::Utc::now();
        let revert_result = PatchRevertResult {
            patch_id: proposal.id.clone(),
            status: crate::workspace::PatchProposalStatus::Reverted,
            files: vec!["README.md".to_string()],
            reverted_at,
            already_reverted: false,
        };
        let auto_rollback = PatchAutoRollbackResult {
            triggered_by: "verificationFailure".to_string(),
            reverted: true,
            error: None,
            result: Some(revert_result),
        };
        let runs = vec![command_run("cargo test", false)];

        let task = runtime
            .record_patch_verification(&proposal, "approval-1", &runs, &[], Some(&auto_rollback))
            .unwrap();

        assert_eq!(
            task.artifacts[0]["autoRollback"]["triggeredBy"],
            serde_json::json!("verificationFailure")
        );
        assert_eq!(
            task.artifacts[0]["autoRollback"]["reverted"],
            serde_json::json!(true)
        );
        assert_eq!(
            task.artifacts[0]["autoRollback"]["result"]["status"],
            serde_json::json!("reverted")
        );
    }

    #[test]
    fn test_record_patch_verification_is_idempotent_for_same_patch() {
        let task_id = "task-patch-verification-idempotent";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(Task::new(task_id, "重复验证补丁"));
        let proposal = patch_proposal_for_task(task_id);
        let runs = vec![command_run("cargo check", true)];

        runtime
            .record_patch_verification(&proposal, "approval-1", &runs, &[], None)
            .unwrap();
        runtime
            .record_patch_verification(&proposal, "approval-1", &runs, &[], None)
            .unwrap();

        let task = runtime.get_task(task_id).unwrap();
        assert_eq!(task.artifacts.len(), 1);
        assert_eq!(
            runtime
                .get_events(task_id)
                .iter()
                .filter(|event| event.kind == TaskEventKind::ArtifactCreated)
                .count(),
            1
        );
    }

    #[test]
    fn test_record_patch_reverted_adds_task_artifact_and_event() {
        let task_id = "task-patch-reverted";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(Task::new(task_id, "回滚补丁"));
        let mut proposal = patch_proposal_for_task(task_id);
        let reverted_at = chrono::Utc::now();
        proposal.status = crate::workspace::PatchProposalStatus::Reverted;
        proposal.reverted_at = Some(reverted_at);
        proposal.reverted_by = Some("tester".to_string());
        let result = PatchRevertResult {
            patch_id: proposal.id.clone(),
            status: crate::workspace::PatchProposalStatus::Reverted,
            files: vec!["README.md".to_string()],
            reverted_at,
            already_reverted: false,
        };

        let task = runtime.record_patch_reverted(&proposal, &result).unwrap();

        assert_eq!(task.artifacts.len(), 1);
        assert_eq!(
            task.artifacts[0]["kind"],
            serde_json::json!("patchReverted")
        );
        assert_eq!(
            task.artifacts[0]["patchId"],
            serde_json::json!("patch-task-1")
        );
        assert_eq!(
            task.artifacts[0]["files"][0],
            serde_json::json!("README.md")
        );
        assert!(task.artifacts[0]["rollbackDiff"]
            .as_str()
            .unwrap()
            .contains("-new"));

        let events = runtime.get_events(task_id);
        let event = events
            .iter()
            .find(|event| event.kind == TaskEventKind::ArtifactCreated)
            .unwrap();
        assert!(event.message.contains("补丁已回滚"));
        assert_eq!(event.payload["kind"], serde_json::json!("patchReverted"));
    }

    #[test]
    fn test_record_patch_reverted_is_idempotent_for_same_patch() {
        let task_id = "task-patch-reverted-idempotent";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(Task::new(task_id, "重复回滚补丁"));
        let mut proposal = patch_proposal_for_task(task_id);
        let reverted_at = chrono::Utc::now();
        proposal.status = crate::workspace::PatchProposalStatus::Reverted;
        proposal.reverted_at = Some(reverted_at);
        let result = PatchRevertResult {
            patch_id: proposal.id.clone(),
            status: crate::workspace::PatchProposalStatus::Reverted,
            files: vec!["README.md".to_string()],
            reverted_at,
            already_reverted: false,
        };

        runtime.record_patch_reverted(&proposal, &result).unwrap();
        runtime.record_patch_reverted(&proposal, &result).unwrap();

        let task = runtime.get_task(task_id).unwrap();
        assert_eq!(task.artifacts.len(), 1);
        assert_eq!(
            runtime
                .get_events(task_id)
                .iter()
                .filter(|event| event.kind == TaskEventKind::ArtifactCreated)
                .count(),
            1
        );
    }

    #[test]
    fn test_cancel_task_marks_pending_steps_skipped() {
        let task_id = "task-cancel";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(Task::new(task_id, "取消任务"));
        runtime.apply_plan(
            task_id,
            TaskPlan {
                task_id: task_id.to_string(),
                goal: "取消任务".to_string(),
                steps: vec![
                    crate::agent::planner_agent::PlanStep {
                        step_id: format!("{task_id}-1"),
                        order: 1,
                        agent: "Executor".to_string(),
                        instruction: "执行任务".to_string(),
                        depends_on: vec![],
                        status: crate::agent::planner_agent::StepStatus::Pending,
                    },
                    crate::agent::planner_agent::PlanStep {
                        step_id: format!("{task_id}-2"),
                        order: 2,
                        agent: "Memory".to_string(),
                        instruction: "记录结果".to_string(),
                        depends_on: vec![format!("{task_id}-1")],
                        status: crate::agent::planner_agent::StepStatus::Pending,
                    },
                ],
                created_at: chrono::Utc::now(),
            },
        );

        let task = runtime.cancel_task(task_id, "测试取消").unwrap();

        assert_eq!(task.status, TaskStatus::Cancelled);
        assert!(task
            .steps
            .iter()
            .all(|step| step.status == StepStatus::Skipped));
        assert!(runtime
            .get_events(task_id)
            .iter()
            .any(|event| event.kind == TaskEventKind::Cancelled));
    }

    #[test]
    fn test_cancelled_task_ignores_late_agent_reply() {
        let task_id = "task-late-reply";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(Task::new(task_id, "取消运行中任务"));
        runtime.apply_plan(
            task_id,
            TaskPlan {
                task_id: task_id.to_string(),
                goal: "取消运行中任务".to_string(),
                steps: vec![crate::agent::planner_agent::PlanStep {
                    step_id: format!("{task_id}-1"),
                    order: 1,
                    agent: "Executor".to_string(),
                    instruction: "执行任务".to_string(),
                    depends_on: vec![],
                    status: crate::agent::planner_agent::StepStatus::Pending,
                }],
                created_at: chrono::Utc::now(),
            },
        );

        let dispatches = runtime.advance_task(task_id);
        assert_eq!(dispatches.len(), 1);
        runtime.cancel_task(task_id, "测试取消").unwrap();

        let next_dispatches = runtime.complete_running_step(
            task_id,
            "Executor",
            "迟到结果",
            serde_json::json!({ "ok": true }),
        );
        let task = runtime.get_task(task_id).unwrap();

        assert!(next_dispatches.is_empty());
        assert_eq!(task.status, TaskStatus::Cancelled);
        assert_eq!(task.steps[0].status, StepStatus::Skipped);
    }

    #[test]
    fn test_cancelled_task_ignores_late_plan() {
        let task_id = "task-late-plan";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(Task::new(task_id, "取消规划中任务"));
        runtime.cancel_task(task_id, "测试取消").unwrap();

        let applied = runtime.apply_plan(
            task_id,
            TaskPlan {
                task_id: task_id.to_string(),
                goal: "取消规划中任务".to_string(),
                steps: vec![crate::agent::planner_agent::PlanStep {
                    step_id: format!("{task_id}-1"),
                    order: 1,
                    agent: "Executor".to_string(),
                    instruction: "执行任务".to_string(),
                    depends_on: vec![],
                    status: crate::agent::planner_agent::StepStatus::Pending,
                }],
                created_at: chrono::Utc::now(),
            },
        );
        let task = runtime.get_task(task_id).unwrap();

        assert!(!applied);
        assert_eq!(task.status, TaskStatus::Cancelled);
        assert!(task.steps.is_empty());
    }

    #[test]
    fn test_retry_cancelled_task_resets_skipped_steps_and_dispatches_ready_step() {
        let task_id = "task-retry-cancelled";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(Task::new(task_id, "重试取消任务"));
        runtime.apply_plan(
            task_id,
            TaskPlan {
                task_id: task_id.to_string(),
                goal: "重试取消任务".to_string(),
                steps: vec![
                    crate::agent::planner_agent::PlanStep {
                        step_id: format!("{task_id}-1"),
                        order: 1,
                        agent: "Executor".to_string(),
                        instruction: "执行任务".to_string(),
                        depends_on: vec![],
                        status: crate::agent::planner_agent::StepStatus::Pending,
                    },
                    crate::agent::planner_agent::PlanStep {
                        step_id: format!("{task_id}-2"),
                        order: 2,
                        agent: "Memory".to_string(),
                        instruction: "记录结果".to_string(),
                        depends_on: vec![format!("{task_id}-1")],
                        status: crate::agent::planner_agent::StepStatus::Pending,
                    },
                ],
                created_at: chrono::Utc::now(),
            },
        );
        runtime.cancel_task(task_id, "测试取消").unwrap();

        let retry = runtime.retry_task(task_id, "测试重试").unwrap();

        assert_eq!(retry.task.status, TaskStatus::Running);
        assert_eq!(retry.dispatches.len(), 1);
        assert_eq!(retry.dispatches[0].step_id, format!("{task_id}-1"));
        assert_eq!(retry.dispatches[0].context["stepAttempt"], 1);
        assert!(runtime
            .get_events(task_id)
            .iter()
            .any(|event| event.kind == TaskEventKind::Retried));
    }

    #[test]
    fn test_retry_failed_task_preserves_completed_dependencies() {
        let task_id = "task-retry-failed";
        let mut task = Task::new(task_id, "重试失败任务");
        let mut first = TaskStep::new(
            format!("{task_id}-1"),
            task_id,
            1,
            "Tool",
            "只读检索",
            vec![],
        );
        let mut second = TaskStep::new(
            format!("{task_id}-2"),
            task_id,
            2,
            "Executor",
            "执行修复",
            vec![format!("{task_id}-1")],
        );
        first.complete(serde_json::json!({ "summary": "found files" }));
        second.fail("执行失败");
        task.steps = vec![first, second];
        task.fail("执行失败");

        let mut runtime = TaskRuntime::default();
        runtime.insert_task(task);

        let retry = runtime.retry_task(task_id, "再次执行").unwrap();
        let task = runtime.get_task(task_id).unwrap();

        assert_eq!(retry.dispatches.len(), 1);
        assert_eq!(task.steps[0].status, StepStatus::Completed);
        assert_eq!(task.steps[1].status, StepStatus::Running);
        assert!(task.steps[1].error.is_none());
    }

    #[test]
    fn test_retry_ignores_late_reply_from_previous_attempt() {
        let task_id = "task-retry-late-reply";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(Task::new(task_id, "重试运行中任务"));
        runtime.apply_plan(
            task_id,
            TaskPlan {
                task_id: task_id.to_string(),
                goal: "重试运行中任务".to_string(),
                steps: vec![crate::agent::planner_agent::PlanStep {
                    step_id: format!("{task_id}-1"),
                    order: 1,
                    agent: "Executor".to_string(),
                    instruction: "执行任务".to_string(),
                    depends_on: vec![],
                    status: crate::agent::planner_agent::StepStatus::Pending,
                }],
                created_at: chrono::Utc::now(),
            },
        );

        let old_dispatches = runtime.advance_task(task_id);
        runtime.cancel_task(task_id, "测试取消").unwrap();
        let retry = runtime.retry_task(task_id, "测试重试").unwrap();

        let next_dispatches = runtime.complete_running_step(
            task_id,
            "Executor",
            "迟到结果",
            old_dispatches[0].context.clone(),
        );
        let task = runtime.get_task(task_id).unwrap();

        assert!(next_dispatches.is_empty());
        assert_eq!(task.steps[0].status, StepStatus::Running);
        assert_eq!(task.steps[0].attempts, 2);

        runtime.complete_running_step(
            task_id,
            "Executor",
            "新尝试结果",
            retry.dispatches[0].context.clone(),
        );
        let task = runtime.get_task(task_id).unwrap();
        assert_eq!(task.steps[0].status, StepStatus::Completed);
    }

    #[test]
    fn test_retry_ignores_late_error_from_previous_attempt() {
        let task_id = "task-retry-late-error";
        let mut runtime = TaskRuntime::default();
        runtime.insert_task(Task::new(task_id, "重试运行中任务"));
        runtime.apply_plan(
            task_id,
            TaskPlan {
                task_id: task_id.to_string(),
                goal: "重试运行中任务".to_string(),
                steps: vec![crate::agent::planner_agent::PlanStep {
                    step_id: format!("{task_id}-1"),
                    order: 1,
                    agent: "Executor".to_string(),
                    instruction: "执行任务".to_string(),
                    depends_on: vec![],
                    status: crate::agent::planner_agent::StepStatus::Pending,
                }],
                created_at: chrono::Utc::now(),
            },
        );

        let old_dispatches = runtime.advance_task(task_id);
        runtime.cancel_task(task_id, "测试取消").unwrap();
        runtime.retry_task(task_id, "测试重试").unwrap();

        runtime.fail_running_step(
            task_id,
            "Executor",
            "迟到错误",
            Some(&old_dispatches[0].context),
        );
        let task = runtime.get_task(task_id).unwrap();

        assert_eq!(task.status, TaskStatus::Running);
        assert_eq!(task.steps[0].status, StepStatus::Running);
        assert!(task.error.is_none());
    }

    #[test]
    fn test_complete_running_step_prefers_context_step_id() {
        let task_id = "task-exact-step";
        let mut task = Task::new(task_id, "精确匹配步骤");
        let mut first = TaskStep::new(
            format!("{task_id}-1"),
            task_id,
            1,
            "Executor",
            "第一个执行步骤",
            vec![],
        );
        let mut second = TaskStep::new(
            format!("{task_id}-2"),
            task_id,
            2,
            "Executor",
            "第二个执行步骤",
            vec![],
        );
        first.start();
        second.start();
        task.steps = vec![first, second];
        task.status = TaskStatus::Running;

        let mut runtime = TaskRuntime::default();
        runtime.insert_task(task);
        runtime.complete_running_step(
            task_id,
            "Executor",
            "第二步完成",
            serde_json::json!({ "stepId": format!("{task_id}-2") }),
        );
        let task = runtime.get_task(task_id).unwrap();

        assert_eq!(task.steps[0].status, StepStatus::Running);
        assert_eq!(task.steps[1].status, StepStatus::Completed);
    }

    #[test]
    fn test_merge_message_context_combines_objects() {
        let merged = merge_message_context(
            serde_json::json!({"projectPlanningContext": {"name": "demo"}}),
            serde_json::json!({"frontendLlmSettings": {"model": "deepseek-chat"}}),
        );

        assert_eq!(merged["projectPlanningContext"]["name"], "demo");
        assert_eq!(
            merged["frontendLlmSettings"]["model"],
            serde_json::json!("deepseek-chat")
        );
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
