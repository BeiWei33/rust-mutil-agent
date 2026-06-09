/**
 * Tauri IPC 封装工具函数
 * 提供与 Rust 后端通信的统一接口
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  SendMessageRequest,
  SendMessageResponse,
  AgentStatus,
  AgentListResponse,
  HealthCheckResponse,
  Message,
  CreateTaskRequest,
  CreateTaskResponse,
  CancelTaskRequest,
  CancelTaskResponse,
  RetryTaskRequest,
  RetryTaskResponse,
  Task,
  TaskEvent,
  TaskListResponse,
  ProjectSnapshot,
  ProjectFileListResponse,
  FileReadResponse,
  SearchProjectTextRequest,
  SearchResponse,
  ProjectCommandRunRequest,
  ProjectCommandRunResponse,
  ProjectCommandRunListResponse,
  ApprovalDecisionRequest,
  ApprovalListResponse,
  ApprovalRequest,
  ApprovalStatus,
  RunApprovedProjectCommandRequest,
  CreatePatchProposalRequest,
  CreatePatchProposalResponse,
  ApplyApprovedPatchRequest,
  RevertAppliedPatchRequest,
  PatchApplyResult,
  PatchRevertResult,
  PatchProposal,
  PatchProposalListResponse,
} from "@/types";

/**
 * Tauri IPC 命令前缀。
 *
 * 当前 Rust 后端注册的是普通 `#[tauri::command]`，不是 `plugin:agent|...` 插件命令。
 */
const CMD_PREFIX = "";

/**
 * 发送聊天消息到指定 Agent
 * @param request 消息请求
 * @returns Agent 响应
 */
export async function sendMessage(
  request: SendMessageRequest
): Promise<SendMessageResponse> {
  return invoke<SendMessageResponse>(`${CMD_PREFIX}send_message`, {
    request,
  });
}

/**
 * 创建软件工程任务
 * @param request 任务请求
 * @returns 创建结果
 */
export async function createTask(
  request: CreateTaskRequest
): Promise<CreateTaskResponse> {
  return invoke<CreateTaskResponse>(`${CMD_PREFIX}create_task`, {
    request,
  });
}

/**
 * 取消软件工程任务
 * @param request 取消请求
 * @returns 取消结果
 */
export async function cancelTask(
  request: CancelTaskRequest
): Promise<CancelTaskResponse> {
  return invoke<CancelTaskResponse>(`${CMD_PREFIX}cancel_task`, {
    request,
  });
}

/**
 * 重试软件工程任务
 * @param request 重试请求
 * @returns 重试结果
 */
export async function retryTask(
  request: RetryTaskRequest
): Promise<RetryTaskResponse> {
  return invoke<RetryTaskResponse>(`${CMD_PREFIX}retry_task`, {
    request,
  });
}

/**
 * 获取单个 Agent 的状态
 * @param agentId Agent ID
 * @returns Agent 状态
 */
export async function getAgentStatus(
  agentId: string
): Promise<AgentStatus> {
  return invoke<AgentStatus>(`${CMD_PREFIX}get_agent_status`, {
    agentId,
  });
}

/**
 * 获取所有 Agent 列表及状态
 * @returns Agent 列表
 */
export async function listAgents(): Promise<AgentListResponse> {
  return invoke<AgentListResponse>(`${CMD_PREFIX}list_agents`);
}

/**
 * 后端健康检查
 * @returns 健康检查结果
 */
export async function healthCheck(): Promise<HealthCheckResponse> {
  return invoke<HealthCheckResponse>(`${CMD_PREFIX}health_check`);
}

/**
 * 获取任务详情
 * @param taskId 任务 ID
 * @returns 任务详情
 */
export async function getTask(taskId: string): Promise<Task | null> {
  return invoke<Task | null>(`${CMD_PREFIX}get_task`, {
    taskId,
  });
}

/**
 * 获取任务列表
 * @returns 任务列表
 */
export async function listTasks(): Promise<TaskListResponse> {
  return invoke<TaskListResponse>(`${CMD_PREFIX}list_tasks`);
}

/**
 * 获取任务事件
 * @param taskId 任务 ID
 * @returns 事件列表
 */
export async function getTaskEvents(taskId: string): Promise<TaskEvent[]> {
  return invoke<TaskEvent[]>(`${CMD_PREFIX}get_task_events`, {
    taskId,
  });
}

/**
 * 获取项目快照
 * @returns 项目结构和技术栈摘要
 */
export async function getProjectSnapshot(): Promise<ProjectSnapshot> {
  return invoke<ProjectSnapshot>(`${CMD_PREFIX}get_project_snapshot`);
}

/**
 * 列出项目文件
 * @param maxFiles 最大返回数量
 * @returns 文件列表
 */
export async function listProjectFiles(maxFiles = 500): Promise<ProjectFileListResponse> {
  return invoke<ProjectFileListResponse>(`${CMD_PREFIX}list_project_files`, {
    maxFiles,
  });
}

/**
 * 读取项目文件
 * @param path workspace 相对路径
 * @returns 文件内容
 */
export async function readProjectFile(path: string): Promise<FileReadResponse> {
  return invoke<FileReadResponse>(`${CMD_PREFIX}read_project_file`, {
    path,
  });
}

/**
 * 搜索项目文本
 * @param request 搜索请求
 * @returns 搜索结果
 */
export async function searchProjectText(
  request: SearchProjectTextRequest
): Promise<SearchResponse> {
  return invoke<SearchResponse>(`${CMD_PREFIX}search_project_text`, {
    request,
  });
}

/**
 * 运行受控项目命令
 * @param request 命令和工作目录
 * @returns 命令运行结果
 */
export async function runProjectCommand(
  request: ProjectCommandRunRequest
): Promise<ProjectCommandRunResponse> {
  return invoke<ProjectCommandRunResponse>(`${CMD_PREFIX}run_project_command`, {
    request,
  });
}

/**
 * 为非 allowlist 项目命令创建审批请求
 * @param request 命令和工作目录
 * @returns 新建的审批请求
 */
export async function requestProjectCommandApproval(
  request: ProjectCommandRunRequest
): Promise<ApprovalRequest> {
  return invoke<ApprovalRequest>(`${CMD_PREFIX}request_project_command_approval`, {
    request,
  });
}

/**
 * 执行已通过审批的项目命令
 * @param request 审批 ID
 * @returns 命令运行结果
 */
export async function runApprovedProjectCommand(
  request: RunApprovedProjectCommandRequest
): Promise<ProjectCommandRunResponse> {
  return invoke<ProjectCommandRunResponse>(`${CMD_PREFIX}run_approved_project_command`, {
    request,
  });
}

/**
 * 获取最近受控项目命令运行记录
 * @param limit 最大返回数量
 * @returns 命令运行记录列表
 */
export async function listProjectCommandRuns(
  limit = 20
): Promise<ProjectCommandRunListResponse> {
  return invoke<ProjectCommandRunListResponse>(`${CMD_PREFIX}list_project_command_runs`, {
    limit,
  });
}

/**
 * 创建补丁提案并生成审批请求
 * @param request 补丁摘要和文件变更
 * @returns 补丁提案和对应审批
 */
export async function createPatchProposal(
  request: CreatePatchProposalRequest
): Promise<CreatePatchProposalResponse> {
  return invoke<CreatePatchProposalResponse>(`${CMD_PREFIX}create_patch_proposal`, {
    request,
  });
}

/**
 * 获取最近补丁提案
 * @param limit 最大返回数量
 * @returns 补丁提案列表
 */
export async function listPatchProposals(
  limit = 20
): Promise<PatchProposalListResponse> {
  return invoke<PatchProposalListResponse>(`${CMD_PREFIX}list_patch_proposals`, {
    limit,
  });
}

/**
 * 获取单个补丁提案
 * @param patchId 补丁提案 ID
 * @returns 补丁提案
 */
export async function getPatchProposal(patchId: string): Promise<PatchProposal | null> {
  return invoke<PatchProposal | null>(`${CMD_PREFIX}get_patch_proposal`, {
    patchId,
  });
}

/**
 * 应用已通过审批的补丁提案
 * @param request 审批 ID
 * @returns 补丁应用结果
 */
export async function applyApprovedPatch(
  request: ApplyApprovedPatchRequest
): Promise<PatchApplyResult> {
  return invoke<PatchApplyResult>(`${CMD_PREFIX}apply_approved_patch`, {
    request,
  });
}

/**
 * 回滚已应用补丁提案
 * @param request 补丁 ID
 * @returns 补丁回滚结果
 */
export async function revertAppliedPatch(
  request: RevertAppliedPatchRequest
): Promise<PatchRevertResult> {
  return invoke<PatchRevertResult>(`${CMD_PREFIX}revert_applied_patch`, {
    request,
  });
}

/**
 * 获取审批请求列表
 * @param status 可选状态过滤
 * @param limit 最大返回数量
 * @returns 审批请求列表
 */
export async function listApprovalRequests(
  status?: ApprovalStatus,
  limit = 50
): Promise<ApprovalListResponse> {
  return invoke<ApprovalListResponse>(`${CMD_PREFIX}list_approval_requests`, {
    status,
    limit,
  });
}

/**
 * 审批或拒绝一个动作
 * @param request 审批决策
 * @returns 更新后的审批请求
 */
export async function approveAction(
  request: ApprovalDecisionRequest
): Promise<ApprovalRequest | null> {
  return invoke<ApprovalRequest | null>(`${CMD_PREFIX}approve_action`, {
    request,
  });
}

/**
 * 获取对话历史
 * @param sessionId 会话 ID
 * @returns 消息列表
 */
export async function getHistory(sessionId: string): Promise<Message[]> {
  return invoke<Message[]>(`${CMD_PREFIX}get_history`, {
    sessionId,
  });
}

/**
 * 清空对话历史
 * @param sessionId 会话 ID
 */
export async function clearHistory(sessionId: string): Promise<void> {
  return invoke<void>(`${CMD_PREFIX}clear_history`, {
    sessionId,
  });
}

// ============ 浏览器环境降级（非 Tauri 环境下的模拟接口） ============

/** 检测是否运行在 Tauri 环境中 */
let _isTauri: boolean | null = null;

export function isTauri(): boolean {
  if (_isTauri !== null) return _isTauri;
  try {
    _isTauri = !!(window as any).__TAURI_INTERNALS__;
  } catch {
    _isTauri = false;
  }
  return _isTauri;
}

/** 生成唯一 ID */
function generateId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

/** 模拟 Agent 数据 */
const MOCK_AGENTS: AgentStatus[] = [
  {
    id: "coordinator",
    runtimeName: "Planner",
    name: "协调员/总控",
    role: "planner",
    roleLabel: "任务规划与调度",
    description: "理解你的需求，拆解任务，并安排合适的 AI 成员协同处理。",
    online: true,
    status: "idle",
    statusLabel: "空闲",
    currentTask: null,
    selectable: true,
    recommended: true,
    isInternal: false,
    capabilities: [
      { name: "任务拆解", description: "将复杂任务拆分为可执行步骤", available: true },
      { name: "成员调度", description: "根据任务类型安排合适的 AI 成员", available: true },
    ],
    lastActive: new Date().toISOString(),
  },
  {
    id: "executor",
    runtimeName: "Executor",
    name: "执行工程师",
    role: "executor",
    roleLabel: "任务执行",
    description: "负责执行明确任务，例如代码处理、命令运行、文件操作和问题修复。",
    online: true,
    status: "busy",
    statusLabel: "忙碌",
    currentTask: "正在处理示例任务",
    selectable: true,
    recommended: false,
    isInternal: false,
    capabilities: [
      { name: "代码/命令执行", description: "执行代码、脚本和命令", available: true },
      { name: "工具调用", description: "调用工具完成具体操作", available: true },
    ],
    lastActive: new Date().toISOString(),
  },
  {
    id: "memory",
    runtimeName: "Memory",
    name: "记忆管理员",
    role: "memory",
    roleLabel: "记忆与检索",
    description: "负责保存、查找和整理历史上下文，通常由协调员自动调用。",
    online: true,
    status: "idle",
    statusLabel: "空闲",
    currentTask: null,
    selectable: false,
    recommended: false,
    isInternal: true,
    capabilities: [
      { name: "上下文记忆", description: "管理对话历史和长期记忆", available: true },
      { name: "信息检索", description: "从知识库或历史记录中检索信息", available: true },
    ],
    lastActive: new Date().toISOString(),
  },
];

let MOCK_TASKS: Task[] = [];
let MOCK_EVENTS: Record<string, TaskEvent[]> = {};
let MOCK_COMMAND_RUNS: ProjectCommandRunResponse[] = [];
let MOCK_APPROVALS: ApprovalRequest[] = [];
let MOCK_PATCH_PROPOSALS: PatchProposal[] = [];

const MOCK_PROJECT_FILES = [
  "README.md",
  "docs/TECHNICAL_DOCUMENTATION.md",
  "docs/SELF_EVOLVING_AGENT_ROADMAP.md",
  "src-tauri/Cargo.toml",
  "src-tauri/src/main.rs",
  "src-tauri/src/commands.rs",
  "src-tauri/src/orchestrator/mod.rs",
  "src-web/package.json",
  "src-web/src/App.tsx",
  "src-web/src/store/useAgentStore.ts",
];

/** 浏览器环境下降级的 sendMessage */
async function mockSendMessage(
  request: SendMessageRequest
): Promise<SendMessageResponse> {
  // 模拟网络延迟
  await new Promise((r) => setTimeout(r, 500 + Math.random() * 1000));

  const mockReplies: Record<string, string> = {
    hello: "你好！我是多 Agent 协同系统，有什么可以帮助你的？",
    help: "我可以帮你：\n1. **代码编写** - 生成高质量代码\n2. **任务规划** - 分解复杂任务\n3. **数据分析** - 分析数据并生成报告\n\n请输入你的需求！",
    code: "```typescript\n// 这是一个示例代码\nfunction greet(name: string): string {\n  return `你好，${name}！`;\n}\n```",
    default: `收到你的消息：「${request.content}」\n\n这是一个模拟回复。在 Tauri 环境中，这里会显示 Agent 的真实响应。\n\n当前可用的 Agent 包括：\n- 🧠 规划 Agent\n- 💻 代码 Agent\n- 📊 分析 Agent`,
  };

  const lower = request.content.toLowerCase();
  let replyContent = mockReplies.default;
  for (const [key, val] of Object.entries(mockReplies)) {
    if (lower.includes(key)) {
      replyContent = val;
      break;
    }
  }

  const agents = MOCK_AGENTS.filter((a) => a.online);
  const picked = agents[Math.floor(Math.random() * agents.length)];

  const task = buildMockTask(request.content, request.agentId || "coordinator");
  MOCK_TASKS = [task, ...MOCK_TASKS];

  return {
    message: {
      id: generateId(),
      role: "assistant",
      content: `${replyContent}\n\n任务编号：${task.id}`,
      timestamp: new Date().toISOString(),
      senderName: picked.name,
    },
    handledBy: picked.id,
    taskId: task.id,
    status: "accepted",
  };
}

function buildMockTask(content: string, agentId: string): Task {
  const taskId = generateId();
  const now = new Date().toISOString();
  const steps = [
    {
      id: `${taskId}-1`,
      taskId,
      order: 1,
      agentId: agentId === "executor" ? "Executor" : "Planner",
      title: "理解任务目标",
      instruction: content,
      status: "completed" as const,
      dependsOn: [],
      attempts: 1,
      result: { mode: "mock" },
      error: null,
      startedAt: now,
      completedAt: now,
    },
    {
      id: `${taskId}-2`,
      taskId,
      order: 2,
      agentId: "Executor",
      title: "模拟执行步骤",
      instruction: `处理：${content}`,
      status: "completed" as const,
      dependsOn: [`${taskId}-1`],
      attempts: 1,
      result: { mode: "mock" },
      error: null,
      startedAt: now,
      completedAt: now,
    },
  ];

  MOCK_EVENTS[taskId] = [
    {
      id: generateId(),
      taskId,
      stepId: null,
      kind: "created",
      message: "任务已创建。",
      payload: null,
      createdAt: now,
    },
    {
      id: generateId(),
      taskId,
      stepId: null,
      kind: "completed",
      message: "浏览器 mock 任务已完成。",
      payload: null,
      createdAt: now,
    },
  ];

  return {
    id: taskId,
    title: content.slice(0, 40) || "未命名任务",
    userGoal: content,
    status: "completed",
    steps,
    artifacts: [],
    output: "浏览器 mock 任务已完成。",
    error: null,
    createdAt: now,
    updatedAt: now,
  };
}

async function mockCreateTask(
  request: CreateTaskRequest
): Promise<CreateTaskResponse> {
  await new Promise((r) => setTimeout(r, 250));
  const task = buildMockTask(request.content, request.agentId || "coordinator");
  MOCK_TASKS = [task, ...MOCK_TASKS];
  return { taskId: task.id, task };
}

async function mockCancelTask(
  request: CancelTaskRequest
): Promise<CancelTaskResponse> {
  await new Promise((r) => setTimeout(r, 150));
  const now = new Date().toISOString();
  const task = MOCK_TASKS.find((item) => item.id === request.taskId) ?? null;
  if (!task) return { taskId: request.taskId, task: null };

  const reason = request.reason || "用户取消任务。";
  const cancelled: Task = {
    ...task,
    status: "cancelled",
    output: reason,
    updatedAt: now,
    steps: task.steps.map((step) =>
      step.status === "pending" || step.status === "running"
        ? {
            ...step,
            status: "skipped" as const,
            error: reason,
            completedAt: now,
          }
        : step
    ),
  };

  MOCK_TASKS = MOCK_TASKS.map((item) => (item.id === request.taskId ? cancelled : item));
  MOCK_EVENTS[request.taskId] = [
    ...(MOCK_EVENTS[request.taskId] ?? []),
    {
      id: generateId(),
      taskId: request.taskId,
      stepId: null,
      kind: "cancelled",
      message: `任务已取消：${reason}`,
      payload: { reason },
      createdAt: now,
    },
  ];

  return { taskId: request.taskId, task: cancelled };
}

async function mockRetryTask(
  request: RetryTaskRequest
): Promise<RetryTaskResponse> {
  await new Promise((r) => setTimeout(r, 150));
  const now = new Date().toISOString();
  const task = MOCK_TASKS.find((item) => item.id === request.taskId) ?? null;
  if (!task) return { taskId: request.taskId, task: null };

  const reason = request.reason || "用户重试任务。";
  const retried: Task = {
    ...task,
    status: "running",
    output: null,
    error: null,
    updatedAt: now,
    steps: task.steps.map((step) =>
      step.status === "completed"
        ? step
        : {
            ...step,
            status: "pending" as const,
            result: null,
            error: null,
            startedAt: null,
            completedAt: null,
          }
    ),
  };

  MOCK_TASKS = MOCK_TASKS.map((item) => (item.id === request.taskId ? retried : item));
  MOCK_EVENTS[request.taskId] = [
    ...(MOCK_EVENTS[request.taskId] ?? []),
    {
      id: generateId(),
      taskId: request.taskId,
      stepId: null,
      kind: "retried",
      message: `任务已重新进入调度：${reason}`,
      payload: { reason },
      createdAt: now,
    },
  ];

  return { taskId: request.taskId, task: retried };
}

/** 浏览器环境下降级的 listAgents */
async function mockListAgents(): Promise<AgentListResponse> {
  await new Promise((r) => setTimeout(r, 200));
  // 随机刷新状态以模拟轮询
  const agents = MOCK_AGENTS.map((a) => ({
    ...a,
    lastActive: new Date().toISOString(),
    status: a.status === "busy" && Math.random() > 0.7 ? "idle" as const : a.status,
  }));
  return { agents };
}

/** 浏览器环境下降级的 healthCheck */
async function mockHealthCheck(): Promise<HealthCheckResponse> {
  return {
    healthy: true,
    version: "0.1.0-dev",
    agentCount: MOCK_AGENTS.filter((a) => a.online).length,
  };
}

async function mockListTasks(): Promise<TaskListResponse> {
  await new Promise((r) => setTimeout(r, 150));
  return { tasks: MOCK_TASKS };
}

async function mockGetTask(taskId: string): Promise<Task | null> {
  await new Promise((r) => setTimeout(r, 100));
  return MOCK_TASKS.find((task) => task.id === taskId) ?? null;
}

async function mockGetTaskEvents(taskId: string): Promise<TaskEvent[]> {
  await new Promise((r) => setTimeout(r, 100));
  return MOCK_EVENTS[taskId] ?? [];
}

async function mockGetProjectSnapshot(): Promise<ProjectSnapshot> {
  await new Promise((r) => setTimeout(r, 150));
  return {
    root: "D:/AI/workspace/codex/rust/rust-mutil-agent",
    name: "rust-mutil-agent",
    techStack: ["Rust", "Tauri v2", "React", "Vite", "TypeScript", "Zustand", "Tailwind CSS"],
    manifests: [
      {
        path: "src-tauri/Cargo.toml",
        kind: "Cargo manifest",
        summary: "package rust-mutil-agent 0.1.0",
      },
      {
        path: "src-web/package.json",
        kind: "Node package",
        summary: "React + Vite frontend package",
      },
    ],
    importantFiles: MOCK_PROJECT_FILES.slice(0, 6).map((path) => ({
      path,
      kind: "Project file",
      description: "浏览器 mock 项目文件。",
    })),
    recommendedCommands: [
      { label: "Rust tests", command: "cargo test", workingDir: "src-tauri", kind: "test" },
      { label: "Rust check", command: "cargo check", workingDir: "src-tauri", kind: "check" },
      { label: "Frontend tests", command: "npm test -- --run", workingDir: "src-web", kind: "test" },
    ],
    generatedAt: new Date().toISOString(),
  };
}

async function mockListProjectFiles(maxFiles = 500): Promise<ProjectFileListResponse> {
  await new Promise((r) => setTimeout(r, 100));
  return {
    files: MOCK_PROJECT_FILES.slice(0, maxFiles).map((path) => ({
      path,
      name: path.split("/").pop() || path,
      isDir: false,
      extension: path.includes(".") ? path.split(".").pop() : null,
      sizeBytes: 1024,
      modifiedAt: new Date().toISOString(),
    })),
  };
}

async function mockReadProjectFile(path: string): Promise<FileReadResponse> {
  await new Promise((r) => setTimeout(r, 100));
  return {
    path,
    content: `// 浏览器 mock 文件\n// ${path}\n\nexport const example = "ProjectPanel";\n`,
    sizeBytes: 72,
  };
}

async function mockSearchProjectText(
  request: SearchProjectTextRequest
): Promise<SearchResponse> {
  await new Promise((r) => setTimeout(r, 120));
  return {
    query: request.query,
    truncated: false,
    matches: MOCK_PROJECT_FILES.slice(0, request.maxResults ?? 10).map((path, index) => ({
      path,
      line: index + 1,
      column: 1,
      preview: `mock match for "${request.query}"`,
    })),
  };
}

async function mockRunProjectCommand(
  request: ProjectCommandRunRequest
): Promise<ProjectCommandRunResponse> {
  await new Promise((r) => setTimeout(r, 300));
  const run = {
    id: generateId(),
    approvalId: null,
    command: request.command,
    workingDir: request.workingDir,
    exitCode: 0,
    success: true,
    stdout: `mock run passed\n${request.workingDir}$ ${request.command}`,
    stderr: "",
    durationMs: 300,
    timedOut: false,
    stdoutTruncated: false,
    stderrTruncated: false,
    createdAt: new Date().toISOString(),
  };
  MOCK_COMMAND_RUNS = [run, ...MOCK_COMMAND_RUNS].slice(0, 50);
  return run;
}

async function mockRequestProjectCommandApproval(
  request: ProjectCommandRunRequest
): Promise<ApprovalRequest> {
  await new Promise((r) => setTimeout(r, 180));
  const now = new Date().toISOString();
  const command = request.command.trim().split(/\s+/).join(" ");
  const workingDir = request.workingDir.trim();
  const approval: ApprovalRequest = {
    id: generateId(),
    taskId: null,
    stepId: null,
    title: `运行项目命令：${command}`,
    reason: `命令 [${command}] 不在受控允许列表中，需要用户确认后再进入后续执行流程。`,
    risk: "high",
    actionType: "runtime.runProjectCommand",
    actionPayload: {
      command,
      workingDir,
      allowedByDefault: false,
    },
    status: "pending",
    requestedBy: "ProjectPanel",
    decidedBy: null,
    decisionNote: null,
    createdAt: now,
    updatedAt: now,
    decidedAt: null,
  };
  MOCK_APPROVALS = [approval, ...MOCK_APPROVALS].slice(0, 100);
  return approval;
}

function commandPayload(value: unknown): ProjectCommandRunRequest | null {
  if (!value || typeof value !== "object") return null;
  const payload = value as { command?: unknown; workingDir?: unknown };
  if (typeof payload.command !== "string" || typeof payload.workingDir !== "string") {
    return null;
  }
  return {
    command: payload.command,
    workingDir: payload.workingDir,
  };
}

function patchPayload(value: unknown): { patchId: string } | null {
  if (!value || typeof value !== "object") return null;
  const payload = value as { patchId?: unknown };
  if (typeof payload.patchId !== "string" || payload.patchId.trim().length === 0) {
    return null;
  }
  return { patchId: payload.patchId.trim() };
}

async function mockRunApprovedProjectCommand(
  request: RunApprovedProjectCommandRequest
): Promise<ProjectCommandRunResponse> {
  await new Promise((r) => setTimeout(r, 300));
  const existing = MOCK_COMMAND_RUNS.find((run) => run.approvalId === request.approvalId);
  if (existing) return existing;

  const approval = MOCK_APPROVALS.find((item) => item.id === request.approvalId);
  if (!approval) throw new Error("找不到对应的审批请求。");
  if (approval.status !== "approved") throw new Error("审批请求尚未通过，不能执行对应命令。");
  if (approval.actionType !== "runtime.runProjectCommand") {
    throw new Error("这条审批不是项目命令执行请求，不能作为命令运行。");
  }
  const payload = commandPayload(approval.actionPayload);
  if (!payload) throw new Error("审批 payload 缺少命令内容或工作目录。");

  const run: ProjectCommandRunResponse = {
    id: generateId(),
    approvalId: request.approvalId,
    command: payload.command,
    workingDir: payload.workingDir,
    exitCode: 0,
    success: true,
    stdout: `mock approved run passed\n${payload.workingDir}$ ${payload.command}`,
    stderr: "",
    durationMs: 300,
    timedOut: false,
    stdoutTruncated: false,
    stderrTruncated: false,
    createdAt: new Date().toISOString(),
  };
  MOCK_COMMAND_RUNS = [run, ...MOCK_COMMAND_RUNS].slice(0, 50);
  return run;
}

async function mockListProjectCommandRuns(
  limit = 20
): Promise<ProjectCommandRunListResponse> {
  await new Promise((r) => setTimeout(r, 80));
  return { runs: MOCK_COMMAND_RUNS.slice(0, limit) };
}

function buildMockPatchDiff(path: string, oldContent: string, newContent: string): string {
  const oldLines = oldContent.split("\n").filter((_, index, lines) =>
    index < lines.length - 1 || lines[index] !== ""
  );
  const newLines = newContent.split("\n").filter((_, index, lines) =>
    index < lines.length - 1 || lines[index] !== ""
  );
  return [
    `diff --git a/${path} b/${path}`,
    `--- a/${path}`,
    `+++ b/${path}`,
    `@@ -1,${oldLines.length} +1,${newLines.length} @@`,
    ...oldLines.map((line) => `-${line}`),
    ...newLines.map((line) => `+${line}`),
  ].join("\n");
}

async function mockCreatePatchProposal(
  request: CreatePatchProposalRequest
): Promise<CreatePatchProposalResponse> {
  await new Promise((r) => setTimeout(r, 180));
  const now = new Date().toISOString();
  const files = request.files.map((file) => {
    const diff = buildMockPatchDiff(file.path, file.oldContent, file.newContent);
    return {
      path: file.path,
      changeType: "modify" as const,
      oldContent: file.oldContent,
      newContent: file.newContent,
      diff,
    };
  });
  const proposal: PatchProposal = {
    id: generateId(),
    taskId: request.taskId ?? null,
    stepId: request.stepId ?? null,
    approvalId: null,
    summary: request.summary.trim(),
    status: "pendingApproval",
    files,
    unifiedDiff: files.map((file) => file.diff).join("\n"),
    requestedBy: request.requestedBy || "ProjectPanel",
    createdAt: now,
    updatedAt: now,
    appliedAt: null,
    appliedBy: null,
    revertedAt: null,
    revertedBy: null,
  };
  const approval: ApprovalRequest = {
    id: generateId(),
    taskId: proposal.taskId ?? null,
    stepId: proposal.stepId ?? null,
    title: `应用补丁：${proposal.summary}`,
    reason: `补丁提案 [${proposal.summary}] 将修改 ${proposal.files.length} 个文件，需要用户确认 diff 后再进入应用流程。`,
    risk: "high",
    actionType: "workspace.applyPatch",
    actionPayload: {
      patchId: proposal.id,
      summary: proposal.summary,
      files: proposal.files.map((file) => ({
        path: file.path,
        changeType: file.changeType,
        diff: file.diff,
      })),
      unifiedDiff: proposal.unifiedDiff,
    },
    status: "pending",
    requestedBy: proposal.requestedBy,
    decidedBy: null,
    decisionNote: null,
    createdAt: now,
    updatedAt: now,
    decidedAt: null,
  };
  const linkedProposal = { ...proposal, approvalId: approval.id };
  MOCK_PATCH_PROPOSALS = [linkedProposal, ...MOCK_PATCH_PROPOSALS].slice(0, 50);
  MOCK_APPROVALS = [approval, ...MOCK_APPROVALS].slice(0, 100);
  return { proposal: linkedProposal, approval };
}

async function mockListPatchProposals(
  limit = 20
): Promise<PatchProposalListResponse> {
  await new Promise((r) => setTimeout(r, 80));
  return { proposals: MOCK_PATCH_PROPOSALS.slice(0, limit) };
}

async function mockGetPatchProposal(patchId: string): Promise<PatchProposal | null> {
  await new Promise((r) => setTimeout(r, 80));
  return MOCK_PATCH_PROPOSALS.find((proposal) => proposal.id === patchId) ?? null;
}

async function mockApplyApprovedPatch(
  request: ApplyApprovedPatchRequest
): Promise<PatchApplyResult> {
  await new Promise((r) => setTimeout(r, 220));

  const approval = MOCK_APPROVALS.find((item) => item.id === request.approvalId);
  if (!approval) throw new Error("找不到对应的审批请求。");
  if (approval.status !== "approved") throw new Error("审批请求尚未通过，不能应用对应补丁。");
  if (approval.actionType !== "workspace.applyPatch") {
    throw new Error("这条审批不是补丁应用请求，不能作为补丁应用。");
  }

  const payload = patchPayload(approval.actionPayload);
  if (!payload) throw new Error("审批 payload 缺少补丁提案 ID。");

  const proposal = MOCK_PATCH_PROPOSALS.find((item) => item.id === payload.patchId);
  if (!proposal) throw new Error("找不到对应的补丁提案。");
  if (proposal.approvalId !== approval.id) {
    throw new Error("补丁提案与审批请求不匹配，已拒绝应用。");
  }
  if (proposal.status !== "approved" && proposal.status !== "applied") {
    throw new Error("补丁提案尚未通过审批，不能应用。");
  }

  const alreadyApplied = proposal.status === "applied";
  const appliedAt = alreadyApplied && proposal.appliedAt
    ? proposal.appliedAt
    : new Date().toISOString();
  const updated: PatchProposal = {
    ...proposal,
    status: "applied",
    appliedAt,
    appliedBy: proposal.appliedBy || "user",
    revertedAt: null,
    revertedBy: null,
    updatedAt: appliedAt,
  };
  MOCK_PATCH_PROPOSALS = MOCK_PATCH_PROPOSALS.map((item) =>
    item.id === updated.id ? updated : item
  );

  const result: PatchApplyResult = {
    patchId: updated.id,
    status: updated.status,
    files: updated.files.map((file) => file.path),
    appliedAt,
    alreadyApplied,
  };

  const verificationRuns: ProjectCommandRunResponse[] = alreadyApplied
    ? []
    : [
        {
          id: generateId(),
          approvalId: null,
          command: "cargo test",
          workingDir: "src-tauri",
          exitCode: 0,
          success: true,
          stdout: "mock patch verification passed",
          stderr: "",
          durationMs: 450,
          timedOut: false,
          stdoutTruncated: false,
          stderrTruncated: false,
          createdAt: new Date().toISOString(),
        },
        {
          id: generateId(),
          approvalId: null,
          command: "npm test -- --run",
          workingDir: "src-web",
          exitCode: 0,
          success: true,
          stdout: "mock frontend verification passed",
          stderr: "",
          durationMs: 380,
          timedOut: false,
          stdoutTruncated: false,
          stderrTruncated: false,
          createdAt: new Date().toISOString(),
        },
      ];
  if (verificationRuns.length > 0) {
    MOCK_COMMAND_RUNS = [...verificationRuns, ...MOCK_COMMAND_RUNS].slice(0, 50);
  }

  if (updated.taskId) {
    const artifact = {
      kind: "patchApplied",
      patchId: updated.id,
      approvalId: approval.id,
      summary: updated.summary,
      status: result.status,
      files: result.files,
      appliedAt: result.appliedAt,
      appliedBy: updated.appliedBy,
      alreadyApplied: result.alreadyApplied,
      unifiedDiff: updated.unifiedDiff,
    };
    const targetTask = MOCK_TASKS.find((task) => task.id === updated.taskId);
    const hasArtifact = (task: Task, kind: string) =>
      task.artifacts.some((item) => {
        const value = item as { kind?: unknown; patchId?: unknown };
        return value.kind === kind && value.patchId === updated.id;
      });
    const shouldAppendArtifact = !!targetTask && !hasArtifact(targetTask, "patchApplied");
    MOCK_TASKS = MOCK_TASKS.map((task) =>
      task.id === updated.taskId && shouldAppendArtifact
        ? {
            ...task,
            artifacts: [...task.artifacts, artifact],
            updatedAt: appliedAt,
          }
        : task
    );
    if (shouldAppendArtifact) {
      MOCK_EVENTS[updated.taskId] = [
        ...(MOCK_EVENTS[updated.taskId] ?? []),
        {
          id: generateId(),
          taskId: updated.taskId,
          stepId: updated.stepId ?? null,
          kind: "artifactCreated",
          message: `补丁已应用：${updated.summary}`,
          payload: artifact,
          createdAt: appliedAt,
        },
      ];
    }
    const verificationArtifact = {
      kind: "patchVerification",
      patchId: updated.id,
      approvalId: approval.id,
      summary: updated.summary,
      status: verificationRuns.every((run) => run.success) ? "passed" : "failed",
      verifiedAt: new Date().toISOString(),
      commandCount: verificationRuns.length,
      successCount: verificationRuns.filter((run) => run.success).length,
      failedCount: verificationRuns.filter((run) => !run.success).length,
      runs: verificationRuns.map((run) => ({
        id: run.id,
        command: run.command,
        workingDir: run.workingDir,
        success: run.success,
        exitCode: run.exitCode,
        durationMs: run.durationMs,
        timedOut: run.timedOut,
        createdAt: run.createdAt,
      })),
      errors: [],
    };
    const refreshedTargetTask = MOCK_TASKS.find((task) => task.id === updated.taskId);
    const shouldAppendVerification =
      verificationRuns.length > 0 &&
      !!refreshedTargetTask &&
      !hasArtifact(refreshedTargetTask, "patchVerification");
    MOCK_TASKS = MOCK_TASKS.map((task) =>
      task.id === updated.taskId && shouldAppendVerification
        ? {
            ...task,
            artifacts: [...task.artifacts, verificationArtifact],
            updatedAt: verificationArtifact.verifiedAt,
          }
        : task
    );
    if (shouldAppendVerification) {
      MOCK_EVENTS[updated.taskId] = [
        ...(MOCK_EVENTS[updated.taskId] ?? []),
        {
          id: generateId(),
          taskId: updated.taskId,
          stepId: updated.stepId ?? null,
          kind: "artifactCreated",
          message: `补丁验证通过：${updated.summary}`,
          payload: verificationArtifact,
          createdAt: verificationArtifact.verifiedAt,
        },
      ];
    }
  }

  return result;
}

async function mockRevertAppliedPatch(
  request: RevertAppliedPatchRequest
): Promise<PatchRevertResult> {
  await new Promise((r) => setTimeout(r, 180));

  const proposal = MOCK_PATCH_PROPOSALS.find((item) => item.id === request.patchId);
  if (!proposal) throw new Error("找不到对应的补丁提案。");
  if (proposal.status !== "applied" && proposal.status !== "reverted") {
    throw new Error("补丁提案尚未应用，不能回滚。");
  }

  const alreadyReverted = proposal.status === "reverted";
  const revertedAt = alreadyReverted && proposal.revertedAt
    ? proposal.revertedAt
    : new Date().toISOString();
  const updated: PatchProposal = {
    ...proposal,
    status: "reverted",
    revertedAt,
    revertedBy: proposal.revertedBy || "user",
    updatedAt: revertedAt,
  };
  MOCK_PATCH_PROPOSALS = MOCK_PATCH_PROPOSALS.map((item) =>
    item.id === updated.id ? updated : item
  );

  const result: PatchRevertResult = {
    patchId: updated.id,
    status: updated.status,
    files: updated.files.map((file) => file.path),
    revertedAt,
    alreadyReverted,
  };

  if (updated.taskId) {
    const artifact = {
      kind: "patchReverted",
      patchId: updated.id,
      approvalId: updated.approvalId,
      summary: updated.summary,
      status: result.status,
      files: result.files,
      revertedAt: result.revertedAt,
      revertedBy: updated.revertedBy,
      alreadyReverted: result.alreadyReverted,
      rollbackDiff: updated.files
        .map((file) => buildMockPatchDiff(file.path, file.newContent, file.oldContent))
        .join("\n"),
    };
    const targetTask = MOCK_TASKS.find((task) => task.id === updated.taskId);
    const hasRevertArtifact = (task: Task) =>
      task.artifacts.some((item) => {
        const value = item as { kind?: unknown; patchId?: unknown };
        return value.kind === "patchReverted" && value.patchId === updated.id;
      });
    const shouldAppendArtifact = !!targetTask && !hasRevertArtifact(targetTask);
    MOCK_TASKS = MOCK_TASKS.map((task) =>
      task.id === updated.taskId && shouldAppendArtifact
        ? {
            ...task,
            artifacts: [...task.artifacts, artifact],
            updatedAt: revertedAt,
          }
        : task
    );
    if (shouldAppendArtifact) {
      MOCK_EVENTS[updated.taskId] = [
        ...(MOCK_EVENTS[updated.taskId] ?? []),
        {
          id: generateId(),
          taskId: updated.taskId,
          stepId: updated.stepId ?? null,
          kind: "artifactCreated",
          message: `补丁已回滚：${updated.summary}`,
          payload: artifact,
          createdAt: revertedAt,
        },
      ];
    }
  }

  return result;
}

async function mockListApprovalRequests(
  status?: ApprovalStatus,
  limit = 50
): Promise<ApprovalListResponse> {
  await new Promise((r) => setTimeout(r, 80));
  const approvals = status
    ? MOCK_APPROVALS.filter((approval) => approval.status === status)
    : MOCK_APPROVALS;
  return { approvals: approvals.slice(0, limit) };
}

async function mockApproveAction(
  request: ApprovalDecisionRequest
): Promise<ApprovalRequest | null> {
  await new Promise((r) => setTimeout(r, 120));
  const approval = MOCK_APPROVALS.find((item) => item.id === request.approvalId) ?? null;
  if (!approval) return null;

  const updated: ApprovalRequest = {
    ...approval,
    status: request.approved ? "approved" : "rejected",
    decidedBy: request.decidedBy || "user",
    decisionNote: request.note || null,
    decidedAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  };
  MOCK_APPROVALS = MOCK_APPROVALS.map((item) =>
    item.id === request.approvalId ? updated : item
  );
  if (updated.actionType === "workspace.applyPatch") {
    const payload = updated.actionPayload as { patchId?: unknown };
    const patchId = typeof payload.patchId === "string" ? payload.patchId : null;
    if (patchId) {
      MOCK_PATCH_PROPOSALS = MOCK_PATCH_PROPOSALS.map((proposal) =>
        proposal.id === patchId
          ? {
              ...proposal,
              status: request.approved ? "approved" : "rejected",
              updatedAt: updated.updatedAt,
            }
          : proposal
      );
    }
  }
  return updated;
}

/** 浏览器环境下降级的 getHistory */
async function mockGetHistory(_sessionId: string): Promise<Message[]> {
  return [
    {
      id: "welcome-1",
      role: "assistant",
      content: "欢迎使用多 Agent 协同智能体！我可以协调多个 AI Agent 来完成复杂的任务。",
      timestamp: new Date(Date.now() - 60000).toISOString(),
      senderName: "系统",
    },
  ];
}

/**
 * 自动降级：Tauri 环境使用 invoke，浏览器环境使用模拟数据
 */
export const api = {
  sendMessage: isTauri() ? sendMessage : mockSendMessage,
  createTask: isTauri() ? createTask : mockCreateTask,
  cancelTask: isTauri() ? cancelTask : mockCancelTask,
  retryTask: isTauri() ? retryTask : mockRetryTask,
  getAgentStatus: isTauri()
    ? getAgentStatus
    : async (id: string) => {
        const agent = MOCK_AGENTS.find((a) => a.id === id);
        if (!agent) throw new Error(`Agent ${id} not found`);
        return { ...agent, lastActive: new Date().toISOString() };
      },
  listAgents: isTauri() ? listAgents : mockListAgents,
  listTasks: isTauri() ? listTasks : mockListTasks,
  getTask: isTauri() ? getTask : mockGetTask,
  getTaskEvents: isTauri() ? getTaskEvents : mockGetTaskEvents,
  getProjectSnapshot: isTauri() ? getProjectSnapshot : mockGetProjectSnapshot,
  listProjectFiles: isTauri() ? listProjectFiles : mockListProjectFiles,
  readProjectFile: isTauri() ? readProjectFile : mockReadProjectFile,
  searchProjectText: isTauri() ? searchProjectText : mockSearchProjectText,
  runProjectCommand: isTauri() ? runProjectCommand : mockRunProjectCommand,
  requestProjectCommandApproval: isTauri()
    ? requestProjectCommandApproval
    : mockRequestProjectCommandApproval,
  runApprovedProjectCommand: isTauri()
    ? runApprovedProjectCommand
    : mockRunApprovedProjectCommand,
  listProjectCommandRuns: isTauri() ? listProjectCommandRuns : mockListProjectCommandRuns,
  createPatchProposal: isTauri() ? createPatchProposal : mockCreatePatchProposal,
  listPatchProposals: isTauri() ? listPatchProposals : mockListPatchProposals,
  getPatchProposal: isTauri() ? getPatchProposal : mockGetPatchProposal,
  applyApprovedPatch: isTauri() ? applyApprovedPatch : mockApplyApprovedPatch,
  revertAppliedPatch: isTauri() ? revertAppliedPatch : mockRevertAppliedPatch,
  listApprovalRequests: isTauri() ? listApprovalRequests : mockListApprovalRequests,
  approveAction: isTauri() ? approveAction : mockApproveAction,
  healthCheck: isTauri() ? healthCheck : mockHealthCheck,
  getHistory: isTauri() ? getHistory : mockGetHistory,
  clearHistory: isTauri()
    ? clearHistory
    : async () => { /* no-op in browser */ },
};
