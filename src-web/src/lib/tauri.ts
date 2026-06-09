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
  SkipTaskStepRequest,
  SkipTaskStepResponse,
  EvolutionDecisionRequest,
  EvolutionDecisionResponse,
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
  ToolInvocationListResponse,
  ToolInvocationRecord,
  KnowledgeItem,
  StoreKnowledgeRequest,
  StoreKnowledgeResponse,
  SearchKnowledgeRequest,
  SearchKnowledgeResponse,
  ApprovalDecisionRequest,
  ApprovalListResponse,
  ApprovalRequest,
  ApprovalStatus,
  RunApprovedProjectCommandRequest,
  ToolActionApprovalRequest,
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
 * 跳过单个任务步骤
 * @param request 跳过步骤请求
 * @returns 更新后的任务
 */
export async function skipTaskStep(
  request: SkipTaskStepRequest
): Promise<SkipTaskStepResponse> {
  return invoke<SkipTaskStepResponse>(`${CMD_PREFIX}skip_task_step`, {
    request,
  });
}

/**
 * 接受或拒绝 Evolution 建议
 * @param request 决策请求
 * @returns 更新后的任务
 */
export async function decideEvolutionNote(
  request: EvolutionDecisionRequest
): Promise<EvolutionDecisionResponse> {
  return invoke<EvolutionDecisionResponse>(`${CMD_PREFIX}decide_evolution_note`, {
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
 * 创建通用工具动作审批请求
 * @param request 工具动作、风险和可选任务/步骤关联
 * @returns 新建的审批请求
 */
export async function requestToolActionApproval(
  request: ToolActionApprovalRequest
): Promise<ApprovalRequest> {
  return invoke<ApprovalRequest>(`${CMD_PREFIX}request_tool_action_approval`, {
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
 * 获取最近 ToolAgent 工具调用审计记录
 * @param limit 最大返回数量
 * @returns 工具调用记录列表
 */
export async function listToolInvocations(
  limit = 20
): Promise<ToolInvocationListResponse> {
  return invoke<ToolInvocationListResponse>(`${CMD_PREFIX}list_tool_invocations`, {
    limit,
  });
}

/**
 * 存储长期知识条目
 * @param request 标题、内容、来源和标签
 * @returns 新知识条目 ID
 */
export async function storeKnowledge(
  request: StoreKnowledgeRequest
): Promise<StoreKnowledgeResponse> {
  return invoke<StoreKnowledgeResponse>(`${CMD_PREFIX}store_knowledge`, {
    request,
  });
}

/**
 * 搜索长期知识条目
 * @param request 搜索关键词和最大返回数量
 * @returns 知识条目列表
 */
export async function searchKnowledge(
  request: SearchKnowledgeRequest
): Promise<SearchKnowledgeResponse> {
  return invoke<SearchKnowledgeResponse>(`${CMD_PREFIX}search_knowledge`, {
    request,
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
    id: "coder",
    runtimeName: "Coder",
    name: "编码工程师",
    role: "coder",
    roleLabel: "补丁草案",
    description: "负责读取上下文并生成受控补丁草案，不直接写文件。",
    online: true,
    status: "idle",
    statusLabel: "空闲",
    currentTask: null,
    selectable: true,
    recommended: false,
    isInternal: false,
    capabilities: [
      { name: "补丁草案", description: "生成可转为 patch proposal 的结构化草案", available: true },
    ],
    lastActive: new Date().toISOString(),
  },
  {
    id: "tester",
    runtimeName: "Tester",
    name: "测试工程师",
    role: "tester",
    roleLabel: "验证计划",
    description: "负责整理验证命令、分析失败信号，不直接绕过受控命令执行。",
    online: true,
    status: "idle",
    statusLabel: "空闲",
    currentTask: null,
    selectable: true,
    recommended: false,
    isInternal: false,
    capabilities: [
      { name: "验证计划", description: "整理测试命令和失败判定", available: true },
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
    id: "review",
    runtimeName: "Review",
    name: "代码评审员",
    role: "review",
    roleLabel: "风险审查",
    description: "负责审查执行结果、潜在风险和缺失验证，输出结构化 ReviewReport。",
    online: true,
    status: "idle",
    statusLabel: "空闲",
    currentTask: null,
    selectable: true,
    recommended: false,
    isInternal: false,
    capabilities: [
      { name: "风险审查", description: "审查执行结果、风险和缺失验证", available: true },
    ],
    lastActive: new Date().toISOString(),
  },
  {
    id: "evolution",
    runtimeName: "Evolution",
    name: "演进顾问",
    role: "evolution",
    roleLabel: "经验沉淀",
    description: "负责总结任务经验并提出可接受或拒绝的后续改进建议。",
    online: true,
    status: "idle",
    statusLabel: "空闲",
    currentTask: null,
    selectable: false,
    recommended: false,
    isInternal: true,
    capabilities: [
      { name: "经验沉淀", description: "总结任务经验并提出改进建议", available: true },
      { name: "上下文记忆", description: "沉淀可复用任务知识", available: true },
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
let MOCK_TOOL_INVOCATIONS: ToolInvocationRecord[] = [
  {
    id: "mock-tool-invocation-1",
    taskId: null,
    stepId: null,
    approvalId: null,
    toolName: "web_search",
    argsSummary: { query: "Rust Agent 审计", max_results: 5 },
    success: true,
    error: null,
    durationMs: 36,
    createdAt: new Date().toISOString(),
  },
];
let MOCK_KNOWLEDGE_ITEMS: KnowledgeItem[] = [
  {
    id: "mock-knowledge-1",
    title: "示例失败经验",
    content: "验证失败后优先查看命令审计中的 stderr 和关联任务 artifact。",
    source: "browser-mock",
    tags: ["FailureCase", "verification"],
    createdAt: new Date().toISOString(),
  },
];
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
  const runtimeName = runtimeNameForMockAgent(agentId);
  const steps = [
    {
      id: `${taskId}-1`,
      taskId,
      order: 1,
      agentId: runtimeName,
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

function runtimeNameForMockAgent(agentId: string): string {
  return (
    MOCK_AGENTS.find(
      (agent) =>
        agent.id === agentId || agent.runtimeName === agentId || agent.role === agentId
    )?.runtimeName ?? "Planner"
  );
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

async function mockSkipTaskStep(
  request: SkipTaskStepRequest
): Promise<SkipTaskStepResponse> {
  await new Promise((r) => setTimeout(r, 150));
  const now = new Date().toISOString();
  const task = MOCK_TASKS.find((item) => item.id === request.taskId) ?? null;
  if (!task) return { taskId: request.taskId, stepId: request.stepId, task: null };

  const reason = request.reason || "用户跳过步骤。";
  const skippedSteps = task.steps.map((step) =>
    step.id === request.stepId && step.status !== "completed" && step.status !== "skipped"
      ? {
          ...step,
          status: "skipped" as const,
          result: { skipped: true, reason },
          error: reason,
          completedAt: now,
        }
      : step
  );
  const readySteps = skippedSteps.map((step) => {
    if (step.status !== "pending") return step;
    const ready = step.dependsOn.every((depId) =>
      skippedSteps.some(
        (candidate) =>
          candidate.id === depId &&
          (candidate.status === "completed" || candidate.status === "skipped")
      )
    );
    return ready
      ? {
          ...step,
          status: "running" as const,
          attempts: step.attempts + 1,
          startedAt: now,
          error: null,
        }
      : step;
  });
  const completedSteps = readySteps.filter((step) => step.status === "completed").length;
  const skippedCount = readySteps.filter((step) => step.status === "skipped").length;
  const done =
    readySteps.length > 0 &&
    readySteps.every((step) => step.status === "completed" || step.status === "skipped");
  const updated: Task = {
    ...task,
    status: done ? "completed" : task.status === "cancelled" ? task.status : "running",
    output: done
      ? skippedCount === 0
        ? `任务执行调度器已完成，共完成 ${completedSteps} 个步骤。`
        : `任务执行调度器已完成，共完成 ${completedSteps} 个步骤，跳过 ${skippedCount} 个步骤。`
      : task.output,
    error: null,
    updatedAt: now,
    steps: readySteps,
  };

  MOCK_TASKS = MOCK_TASKS.map((item) => (item.id === request.taskId ? updated : item));
  MOCK_EVENTS[request.taskId] = [
    ...(MOCK_EVENTS[request.taskId] ?? []),
    {
      id: generateId(),
      taskId: request.taskId,
      stepId: request.stepId,
      kind: "stepSkipped",
      message: `步骤已跳过：${reason}`,
      payload: { stepId: request.stepId, reason },
      createdAt: now,
    },
  ];
  if (done) {
    MOCK_EVENTS[request.taskId] = [
      ...(MOCK_EVENTS[request.taskId] ?? []),
      {
        id: generateId(),
        taskId: request.taskId,
        stepId: null,
        kind: "completed",
        message: updated.output || "任务已完成。",
        payload: { completedSteps, skippedSteps: skippedCount },
        createdAt: now,
      },
    ];
  }

  return { taskId: request.taskId, stepId: request.stepId, task: updated };
}

async function mockDecideEvolutionNote(
  request: EvolutionDecisionRequest
): Promise<EvolutionDecisionResponse> {
  const task = MOCK_TASKS.find((item) => item.id === request.taskId);
  if (!task) return { taskId: request.taskId, task: null };
  const artifact = {
    kind: "evolutionDecision",
    taskId: request.taskId,
    decision: request.accepted ? "accepted" : "rejected",
    accepted: request.accepted,
    note: request.note ?? null,
    decidedBy: request.decidedBy ?? "browser-mock",
    decidedAt: new Date().toISOString(),
    appliesAutomatically: false,
  };
  const updated = {
    ...task,
    artifacts: [...task.artifacts, artifact],
    updatedAt: artifact.decidedAt,
  };
  MOCK_TASKS = MOCK_TASKS.map((item) => (item.id === request.taskId ? updated : item));
  MOCK_EVENTS[request.taskId] = [
    ...(MOCK_EVENTS[request.taskId] ?? []),
    {
      id: generateId(),
      taskId: request.taskId,
      stepId: null,
      kind: "artifactCreated",
      message: `Evolution 建议已${request.accepted ? "接受" : "拒绝"}。`,
      payload: artifact,
      createdAt: artifact.decidedAt,
    },
  ];
  return { taskId: request.taskId, task: updated };
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
  updateMockCommandRun(approval, run);
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
    taskId: request.taskId ?? null,
    stepId: request.stepId ?? null,
    title: `运行项目命令：${command}`,
    reason: `命令 [${command}] 不在受控允许列表中，需要用户确认后再进入后续执行流程。`,
    risk: "high",
    actionType: "runtime.runProjectCommand",
    actionPayload: {
      command,
      workingDir,
      allowedByDefault: false,
      taskId: request.taskId ?? null,
      stepId: request.stepId ?? null,
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
  updateMockCommandApprovalRequested(approval);
  return approval;
}

async function mockRequestToolActionApproval(
  request: ToolActionApprovalRequest
): Promise<ApprovalRequest> {
  await new Promise((r) => setTimeout(r, 160));
  const now = new Date().toISOString();
  const approval: ApprovalRequest = {
    id: generateId(),
    taskId: request.taskId ?? null,
    stepId: request.stepId ?? null,
    title: request.title.trim(),
    reason: request.reason.trim(),
    risk: request.risk ?? "medium",
    actionType: request.actionType.trim(),
    actionPayload: request.actionPayload ?? {},
    status: "pending",
    requestedBy: request.requestedBy ?? "ToolAgent",
    decidedBy: null,
    decisionNote: null,
    createdAt: now,
    updatedAt: now,
    decidedAt: null,
  };
  MOCK_APPROVALS = [approval, ...MOCK_APPROVALS].slice(0, 100);
  updateMockToolApprovalRequested(approval);
  return approval;
}

function commandPayload(value: unknown): ProjectCommandRunRequest | null {
  if (!value || typeof value !== "object") return null;
  const payload = value as {
    command?: unknown;
    workingDir?: unknown;
    taskId?: unknown;
    stepId?: unknown;
  };
  if (typeof payload.command !== "string" || typeof payload.workingDir !== "string") {
    return null;
  }
  return {
    command: payload.command,
    workingDir: payload.workingDir,
    taskId: typeof payload.taskId === "string" ? payload.taskId : null,
    stepId: typeof payload.stepId === "string" ? payload.stepId : null,
  };
}

function appendMockTaskEvent(taskId: string, event: Omit<TaskEvent, "id" | "taskId">) {
  MOCK_EVENTS[taskId] = [
    ...(MOCK_EVENTS[taskId] ?? []),
    {
      id: generateId(),
      taskId,
      ...event,
    },
  ];
}

function hasApprovalArtifact(task: Task, kind: string, approvalId: string): boolean {
  return task.artifacts.some((artifact) => {
    const value = artifact as { kind?: unknown; approvalId?: unknown };
    return value.kind === kind && value.approvalId === approvalId;
  });
}

function toolActionLabel(approval: ApprovalRequest): string {
  const payload =
    approval.actionPayload && typeof approval.actionPayload === "object"
      ? (approval.actionPayload as { tool?: unknown; name?: unknown })
      : null;
  const label = typeof payload?.tool === "string" ? payload.tool : payload?.name;
  return typeof label === "string" && label.trim() ? label.trim() : approval.title;
}

function updateMockToolApprovalRequested(approval: ApprovalRequest) {
  if (!approval.taskId || !approval.actionType.startsWith("tool.")) return;
  const now = approval.createdAt;
  const artifact = {
    kind: "toolApproval",
    approvalId: approval.id,
    actionType: approval.actionType,
    title: approval.title,
    tool: toolActionLabel(approval),
    risk: approval.risk,
    status: approval.status,
    requestedBy: approval.requestedBy,
    createdAt: now,
    payload: approval.actionPayload,
  };
  const target = MOCK_TASKS.find((task) => task.id === approval.taskId);
  if (!target || hasApprovalArtifact(target, "toolApproval", approval.id)) return;

  MOCK_TASKS = MOCK_TASKS.map((task) =>
    task.id === approval.taskId
      ? {
          ...task,
          status: ["completed", "failed", "cancelled"].includes(task.status)
            ? task.status
            : "waitingApproval",
          updatedAt: now,
          artifacts: [...task.artifacts, artifact],
          steps: task.steps.map((step) =>
            step.id === approval.stepId &&
            ["pending", "running", "waitingApproval"].includes(step.status)
              ? { ...step, status: "waitingApproval" as const, error: null }
              : step
          ),
        }
      : task
  );
  appendMockTaskEvent(approval.taskId, {
    stepId: approval.stepId ?? null,
    kind: "approvalRequested",
    message: `工具动作等待审批：${toolActionLabel(approval)}`,
    payload: artifact,
    createdAt: now,
  });
}

function updateMockToolApprovalResolved(approval: ApprovalRequest) {
  if (!approval.taskId || !approval.actionType.startsWith("tool.")) return;
  const now = approval.decidedAt ?? approval.updatedAt;
  const artifact = {
    kind: "toolApprovalResolved",
    approvalId: approval.id,
    actionType: approval.actionType,
    title: approval.title,
    tool: toolActionLabel(approval),
    risk: approval.risk,
    status: approval.status,
    decidedBy: approval.decidedBy,
    decisionNote: approval.decisionNote,
    decidedAt: approval.decidedAt,
    payload: approval.actionPayload,
  };
  const target = MOCK_TASKS.find((task) => task.id === approval.taskId);
  if (!target || hasApprovalArtifact(target, "toolApprovalResolved", approval.id)) return;

  const rejected = approval.status === "rejected";
  const failedMessage = `工具动作审批已拒绝：${toolActionLabel(approval)}`;
  MOCK_TASKS = MOCK_TASKS.map((task) => {
    if (task.id !== approval.taskId) return task;
    const terminal = ["completed", "failed", "cancelled"].includes(task.status);
    return {
      ...task,
      status:
        approval.status === "approved" && task.status === "waitingApproval"
          ? "running"
          : rejected && !terminal
            ? "failed"
            : task.status,
      error: rejected && !terminal ? failedMessage : task.error,
      updatedAt: now,
      artifacts: [...task.artifacts, artifact],
      steps: task.steps.map((step) =>
        step.id === approval.stepId
          ? approval.status === "approved" && step.status === "waitingApproval"
            ? { ...step, status: "running" as const, error: null }
            : rejected && ["pending", "running", "waitingApproval"].includes(step.status)
              ? { ...step, status: "failed" as const, error: failedMessage, completedAt: now }
              : step
          : step
      ),
    };
  });
  if (rejected) {
    appendMockTaskEvent(approval.taskId, {
      stepId: approval.stepId ?? null,
      kind: "stepFailed",
      message: failedMessage,
      payload: artifact,
      createdAt: now,
    });
  }
  appendMockTaskEvent(approval.taskId, {
    stepId: approval.stepId ?? null,
    kind: "approvalResolved",
    message: `工具动作审批${approval.status === "approved" ? "已通过" : "已拒绝"}：${toolActionLabel(
      approval
    )}`,
    payload: artifact,
    createdAt: now,
  });
}

function updateMockCommandApprovalRequested(approval: ApprovalRequest) {
  if (!approval.taskId || approval.actionType !== "runtime.runProjectCommand") return;
  const payload = commandPayload(approval.actionPayload);
  const now = approval.createdAt;
  const artifact = {
    kind: "commandApproval",
    approvalId: approval.id,
    command: payload?.command ?? approval.title,
    workingDir: payload?.workingDir ?? null,
    status: approval.status,
    requestedBy: approval.requestedBy,
    createdAt: now,
  };
  const target = MOCK_TASKS.find((task) => task.id === approval.taskId);
  if (!target || hasApprovalArtifact(target, "commandApproval", approval.id)) return;

  MOCK_TASKS = MOCK_TASKS.map((task) =>
    task.id === approval.taskId
      ? {
          ...task,
          status: ["completed", "failed", "cancelled"].includes(task.status)
            ? task.status
            : "waitingApproval",
          updatedAt: now,
          artifacts: [...task.artifacts, artifact],
          steps: task.steps.map((step) =>
            step.id === approval.stepId &&
            ["pending", "running", "waitingApproval"].includes(step.status)
              ? { ...step, status: "waitingApproval" as const, error: null }
              : step
          ),
        }
      : task
  );
  appendMockTaskEvent(approval.taskId, {
    stepId: approval.stepId ?? null,
    kind: "approvalRequested",
    message: `项目命令等待审批：${payload?.command ?? approval.title}`,
    payload: artifact,
    createdAt: now,
  });
}

function updateMockCommandApprovalResolved(approval: ApprovalRequest) {
  if (!approval.taskId || approval.actionType !== "runtime.runProjectCommand") return;
  const payload = commandPayload(approval.actionPayload);
  const now = approval.decidedAt ?? approval.updatedAt;
  const artifact = {
    kind: "commandApprovalResolved",
    approvalId: approval.id,
    command: payload?.command ?? approval.title,
    workingDir: payload?.workingDir ?? null,
    status: approval.status,
    decidedBy: approval.decidedBy,
    decisionNote: approval.decisionNote,
    decidedAt: approval.decidedAt,
  };
  const target = MOCK_TASKS.find((task) => task.id === approval.taskId);
  if (!target || hasApprovalArtifact(target, "commandApprovalResolved", approval.id)) return;

  const rejected = approval.status === "rejected";
  const failedMessage = `项目命令审批已拒绝：${payload?.command ?? approval.title}`;
  MOCK_TASKS = MOCK_TASKS.map((task) => {
    if (task.id !== approval.taskId) return task;
    const terminal = ["completed", "failed", "cancelled"].includes(task.status);
    return {
      ...task,
      status:
        approval.status === "approved" && task.status === "waitingApproval"
          ? "running"
          : rejected && !terminal
            ? "failed"
            : task.status,
      error: rejected && !terminal ? failedMessage : task.error,
      updatedAt: now,
      artifacts: [...task.artifacts, artifact],
      steps: task.steps.map((step) =>
        step.id === approval.stepId
          ? approval.status === "approved" && step.status === "waitingApproval"
            ? { ...step, status: "running" as const, error: null }
            : rejected && ["pending", "running", "waitingApproval"].includes(step.status)
              ? { ...step, status: "failed" as const, error: failedMessage, completedAt: now }
              : step
          : step
      ),
    };
  });
  if (rejected) {
    appendMockTaskEvent(approval.taskId, {
      stepId: approval.stepId ?? null,
      kind: "stepFailed",
      message: failedMessage,
      payload: artifact,
      createdAt: now,
    });
  }
  appendMockTaskEvent(approval.taskId, {
    stepId: approval.stepId ?? null,
    kind: "approvalResolved",
    message: `项目命令审批${approval.status === "approved" ? "已通过" : "已拒绝"}：${
      payload?.command ?? approval.title
    }`,
    payload: artifact,
    createdAt: now,
  });
}

function updateMockCommandRun(approval: ApprovalRequest, run: ProjectCommandRunResponse) {
  if (!approval.taskId || approval.actionType !== "runtime.runProjectCommand") return;
  const now = run.createdAt;
  const artifact = {
    kind: "commandRun",
    approvalId: approval.id,
    runId: run.id,
    command: run.command,
    workingDir: run.workingDir,
    success: run.success,
    exitCode: run.exitCode,
    durationMs: run.durationMs,
    timedOut: run.timedOut,
    createdAt: now,
  };
  const target = MOCK_TASKS.find((task) => task.id === approval.taskId);
  if (!target || hasApprovalArtifact(target, "commandRun", approval.id)) return;

  const failedMessage = `项目命令执行失败：${run.command}`;
  const targetTerminal = ["completed", "failed", "cancelled"].includes(target.status);
  MOCK_TASKS = MOCK_TASKS.map((task) => {
    if (task.id !== approval.taskId) return task;
    const terminal = ["completed", "failed", "cancelled"].includes(task.status);
    let steps = task.steps.map((step) =>
      step.id === approval.stepId && run.success && !terminal
        ? ["pending", "running", "waitingApproval"].includes(step.status)
          ? {
              ...step,
              status: "completed" as const,
              result: {
                approvalId: approval.id,
                runId: run.id,
                command: run.command,
                workingDir: run.workingDir,
                success: run.success,
                exitCode: run.exitCode,
              },
              completedAt: now,
              error: null,
            }
          : step
        : step
    );
    if (!run.success && task.status !== "cancelled") {
      steps = steps.map((step) =>
        step.id === approval.stepId
          ? { ...step, status: "failed" as const, error: failedMessage, completedAt: now }
          : step
      );
    }
    const allCompleted = steps.length > 0 && steps.every((step) => step.status === "completed");
    return {
      ...task,
      status: !run.success && task.status !== "cancelled"
        ? "failed"
        : run.success && !terminal && allCompleted
          ? "completed"
          : task.status,
      output:
        run.success && !terminal && allCompleted
          ? `任务执行调度器已完成，共完成 ${steps.length} 个步骤。`
          : task.output,
      error: !run.success && task.status !== "cancelled" ? failedMessage : task.error,
      updatedAt: now,
      artifacts: [...task.artifacts, artifact],
      steps,
    };
  });
  if (run.success && !targetTerminal) {
    appendMockTaskEvent(approval.taskId, {
      stepId: approval.stepId ?? null,
      kind: "stepCompleted",
      message: "已审批项目命令执行通过，步骤已完成。",
      payload: artifact,
      createdAt: now,
    });
  } else {
    appendMockTaskEvent(approval.taskId, {
      stepId: approval.stepId ?? null,
      kind: "stepFailed",
      message: failedMessage,
      payload: artifact,
      createdAt: now,
    });
  }
  appendMockTaskEvent(approval.taskId, {
    stepId: approval.stepId ?? null,
    kind: "artifactCreated",
    message: `项目命令执行${run.success ? "通过" : "失败"}：${run.command}`,
    payload: artifact,
    createdAt: now,
  });
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

async function mockListToolInvocations(
  limit = 20
): Promise<ToolInvocationListResponse> {
  await new Promise((r) => setTimeout(r, 80));
  return { invocations: MOCK_TOOL_INVOCATIONS.slice(0, limit) };
}

async function mockStoreKnowledge(
  request: StoreKnowledgeRequest
): Promise<StoreKnowledgeResponse> {
  await new Promise((r) => setTimeout(r, 100));
  const title = request.title.trim();
  const content = request.content.trim();
  if (!title) throw new Error("知识标题不能为空。");
  if (!content) throw new Error("知识内容不能为空。");

  const item: KnowledgeItem = {
    id: generateId(),
    title,
    content,
    source: request.source?.trim() || null,
    tags: (request.tags ?? []).map((tag) => tag.trim()).filter(Boolean),
    createdAt: new Date().toISOString(),
  };
  MOCK_KNOWLEDGE_ITEMS = [item, ...MOCK_KNOWLEDGE_ITEMS].slice(0, 100);
  return { id: item.id };
}

async function mockSearchKnowledge(
  request: SearchKnowledgeRequest
): Promise<SearchKnowledgeResponse> {
  await new Promise((r) => setTimeout(r, 100));
  const query = request.query.trim();
  if (!query) throw new Error("搜索关键词不能为空。");
  const lower = query.toLowerCase();
  const limit = Math.max(1, Math.min(request.limit ?? 20, 100));
  return {
    query,
    items: MOCK_KNOWLEDGE_ITEMS.filter((item) => {
      return (
        item.title.toLowerCase().includes(lower) ||
        item.content.toLowerCase().includes(lower) ||
        item.tags.some((tag) => tag.toLowerCase().includes(lower))
      );
    }).slice(0, limit),
  };
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

  let result: PatchApplyResult = {
    patchId: updated.id,
    status: updated.status,
    files: updated.files.map((file) => file.path),
    appliedAt,
    alreadyApplied,
  };

  const mockVerificationFails =
    updated.summary.toLowerCase().includes("fail") || updated.summary.includes("失败");
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
          exitCode: mockVerificationFails ? 1 : 0,
          success: !mockVerificationFails,
          stdout: mockVerificationFails ? "" : "mock frontend verification passed",
          stderr: mockVerificationFails ? "mock frontend verification failed" : "",
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

  if (
    request.autoRollbackOnVerificationFailure &&
    verificationRuns.some((run) => !run.success)
  ) {
    const rollback = await mockRevertAppliedPatch({ patchId: updated.id });
    const autoRollback = {
      triggeredBy: "verificationFailure",
      reverted: rollback.status === "reverted",
      error: null,
      result: rollback,
    };
    result = {
      ...result,
      status: rollback.status,
      autoRollback,
    };
    if (updated.taskId) {
      const withAutoRollback = (artifact: unknown) => {
        const value = artifact as { kind?: unknown; patchId?: unknown };
        return value.kind === "patchVerification" && value.patchId === updated.id
          ? { ...(artifact as object), autoRollback }
          : artifact;
      };
      MOCK_TASKS = MOCK_TASKS.map((task) =>
        task.id === updated.taskId
          ? {
              ...task,
              artifacts: task.artifacts.map(withAutoRollback),
            }
          : task
      );
      MOCK_EVENTS[updated.taskId] = (MOCK_EVENTS[updated.taskId] ?? []).map((event) =>
        event.kind === "artifactCreated"
          ? {
              ...event,
              payload: withAutoRollback(event.payload),
            }
          : event
      );
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
  } else if (updated.actionType === "runtime.runProjectCommand") {
    updateMockCommandApprovalResolved(updated);
  } else if (updated.actionType.startsWith("tool.")) {
    updateMockToolApprovalResolved(updated);
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
  skipTaskStep: isTauri() ? skipTaskStep : mockSkipTaskStep,
  decideEvolutionNote: isTauri() ? decideEvolutionNote : mockDecideEvolutionNote,
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
  requestToolActionApproval: isTauri()
    ? requestToolActionApproval
    : mockRequestToolActionApproval,
  runApprovedProjectCommand: isTauri()
    ? runApprovedProjectCommand
    : mockRunApprovedProjectCommand,
  listProjectCommandRuns: isTauri() ? listProjectCommandRuns : mockListProjectCommandRuns,
  listToolInvocations: isTauri() ? listToolInvocations : mockListToolInvocations,
  storeKnowledge: isTauri() ? storeKnowledge : mockStoreKnowledge,
  searchKnowledge: isTauri() ? searchKnowledge : mockSearchKnowledge,
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
