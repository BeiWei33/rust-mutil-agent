/**
 * 多 Agent 协同智能体 — 类型定义
 */

/** Agent 能力描述 */
export interface Capability {
  /** 能力名称 */
  name: string;
  /** 能力描述 */
  description: string;
  /** 是否可用 */
  available: boolean;
}

/** Agent 状态 */
export interface AgentStatus {
  /** Agent 唯一标识（前端稳定 ID，如 coordinator/executor） */
  id: string;
  /** 后端运行时名称（如 Planner/Executor） */
  runtimeName?: string;
  /** Agent 中文显示名 */
  name: string;
  /** Agent 角色/类型 */
  role: string;
  /** 中文角色标签 */
  roleLabel?: string;
  /** 面向用户的一句话说明 */
  description?: string;
  /** 在线状态 */
  online: boolean;
  /** 运行状态 */
  status: "idle" | "busy" | "error" | "offline";
  /** 中文状态标签 */
  statusLabel?: string;
  /** 当前正在执行的任务描述 */
  currentTask: string | null;
  /** Agent 能力列表 */
  capabilities: Capability[];
  /** 最后活跃时间（ISO 字符串） */
  lastActive: string;
  /** 是否允许用户从聊天输入框直接选择 */
  selectable?: boolean;
  /** 是否推荐作为默认入口 */
  recommended?: boolean;
  /** 是否为内部能力型 Agent */
  isInternal?: boolean;
}

/** 消息角色 */
export type MessageRole = "user" | "assistant" | "system" | "agent";

/** 单条聊天消息 */
export interface Message {
  /** 消息唯一 ID */
  id: string;
  /** 消息角色 */
  role: MessageRole;
  /** 消息内容（支持 Markdown） */
  content: string;
  /** 发送时间戳（ISO 字符串） */
  timestamp: string;
  /** 发送者名称（Agent 消息时显示 Agent 名） */
  senderName?: string;
}

/** 聊天消息（Zustand store 内部使用） */
export type ChatMessage = Message;

/** 请求级 LLM 设置 */
export interface LlmRequestSettings {
  /** 当前模型名称 */
  model?: string;
  /** API Key（仅请求期传递，后端不会写入任务上下文） */
  apiKey?: string;
  /** API 基础 URL */
  apiBaseUrl?: string;
  /** 最大 Token 数 */
  maxTokens?: number;
  /** 温度参数 */
  temperature?: number;
}

/** 发送消息请求 */
export interface SendMessageRequest {
  /** 消息内容 */
  content: string;
  /** 目标 Agent ID（可选，不指定则由后端自动路由） */
  agentId?: string;
  /** 路由模式：自动分配或直连指定成员 */
  routeMode?: "auto" | "direct";
  /** 会话 ID（预留） */
  sessionId?: string;
  /** 请求级 LLM 设置（可选） */
  llmSettings?: LlmRequestSettings;
}

/** 消息路由信息 */
export interface RouteInfo {
  mode: "auto" | "direct" | string;
  requestedAgentId?: string | null;
  targetAgentId: string;
  targetRuntimeName: string;
  targetDisplayName: string;
  fallback: boolean;
  reason: string;
}

/** 发送消息响应 */
export interface SendMessageResponse {
  /** 响应消息 */
  message: Message;
  /** 处理该消息的 Agent ID */
  handledBy: string;
  /** 任务 ID */
  taskId?: string;
  /** 提交状态 */
  status?: "accepted" | "completed" | "failed" | "timeout" | string;
  /** 路由信息 */
  route?: RouteInfo;
  /** 非致命警告 */
  warnings?: string[];
}

/** 任务状态 */
export type TaskStatus =
  | "draft"
  | "planning"
  | "waitingApproval"
  | "running"
  | "reviewing"
  | "failed"
  | "completed"
  | "cancelled";

/** 任务步骤状态 */
export type StepStatus =
  | "pending"
  | "waitingApproval"
  | "running"
  | "failed"
  | "completed"
  | "skipped";

/** 任务步骤 */
export interface TaskStep {
  id: string;
  taskId: string;
  order: number;
  agentId: string;
  title: string;
  instruction: string;
  status: StepStatus;
  dependsOn: string[];
  attempts: number;
  result?: unknown | null;
  error?: string | null;
  startedAt?: string | null;
  completedAt?: string | null;
}

/** 软件工程任务 */
export interface Task {
  id: string;
  title: string;
  userGoal: string;
  status: TaskStatus;
  steps: TaskStep[];
  artifacts: unknown[];
  output?: string | null;
  error?: string | null;
  createdAt: string;
  updatedAt: string;
}

/** 任务事件类型 */
export type TaskEventKind =
  | "created"
  | "planned"
  | "stepStarted"
  | "stepCompleted"
  | "stepFailed"
  | "completed"
  | "failed"
  | "cancelled"
  | "retried"
  | "artifactCreated";

/** 任务事件 */
export interface TaskEvent {
  id: string;
  taskId: string;
  stepId?: string | null;
  kind: TaskEventKind;
  message: string;
  payload: unknown;
  createdAt: string;
}

/** 创建任务请求 */
export interface CreateTaskRequest {
  content: string;
  agentId?: string;
  llmSettings?: LlmRequestSettings;
}

/** 创建任务响应 */
export interface CreateTaskResponse {
  taskId: string;
  task?: Task | null;
}

/** 取消任务请求 */
export interface CancelTaskRequest {
  taskId: string;
  reason?: string;
}

/** 取消任务响应 */
export interface CancelTaskResponse {
  taskId: string;
  task?: Task | null;
}

/** 重试任务请求 */
export interface RetryTaskRequest {
  taskId: string;
  reason?: string;
}

/** 重试任务响应 */
export interface RetryTaskResponse {
  taskId: string;
  task?: Task | null;
}

/** 任务列表响应 */
export interface TaskListResponse {
  tasks: Task[];
}

/** 项目 manifest */
export interface ProjectManifest {
  path: string;
  kind: string;
  summary: string;
}

/** 项目重要文件 */
export interface ProjectImportantFile {
  path: string;
  kind: string;
  description: string;
}

/** 推荐验证命令 */
export interface ProjectCommand {
  label: string;
  command: string;
  workingDir: string;
  kind: string;
}

/** 运行受控项目命令请求 */
export interface ProjectCommandRunRequest {
  command: string;
  workingDir: string;
}

/** 运行受控项目命令响应 */
export interface ProjectCommandRunResponse {
  id: string;
  approvalId?: string | null;
  command: string;
  workingDir: string;
  exitCode?: number | null;
  success: boolean;
  stdout: string;
  stderr: string;
  durationMs: number;
  timedOut: boolean;
  stdoutTruncated: boolean;
  stderrTruncated: boolean;
  createdAt: string;
}

/** 最近命令运行记录响应 */
export interface ProjectCommandRunListResponse {
  runs: ProjectCommandRunResponse[];
}

/** 审批风险等级 */
export type ApprovalRisk = "low" | "medium" | "high" | "critical";

/** 审批状态 */
export type ApprovalStatus = "pending" | "approved" | "rejected" | "cancelled";

/** 审批请求 */
export interface ApprovalRequest {
  id: string;
  taskId?: string | null;
  stepId?: string | null;
  title: string;
  reason: string;
  risk: ApprovalRisk;
  actionType: string;
  actionPayload: unknown;
  status: ApprovalStatus;
  requestedBy: string;
  decidedBy?: string | null;
  decisionNote?: string | null;
  createdAt: string;
  updatedAt: string;
  decidedAt?: string | null;
}

/** 审批列表响应 */
export interface ApprovalListResponse {
  approvals: ApprovalRequest[];
}

/** 审批动作请求 */
export interface ApprovalDecisionRequest {
  approvalId: string;
  approved: boolean;
  note?: string;
  decidedBy?: string;
}

/** 执行已审批项目命令请求 */
export interface RunApprovedProjectCommandRequest {
  approvalId: string;
}

/** 补丁提案状态 */
export type PatchProposalStatus =
  | "draft"
  | "pendingApproval"
  | "approved"
  | "rejected"
  | "applied"
  | "reverted";

/** 补丁文件变更类型 */
export type PatchChangeType = "modify";

/** 创建补丁文件输入 */
export interface PatchFileInput {
  path: string;
  oldContent: string;
  newContent: string;
}

/** 创建补丁提案请求 */
export interface CreatePatchProposalRequest {
  summary: string;
  files: PatchFileInput[];
  taskId?: string | null;
  stepId?: string | null;
  requestedBy?: string | null;
}

/** 补丁文件变更 */
export interface PatchFileChange {
  path: string;
  changeType: PatchChangeType;
  oldContent: string;
  newContent: string;
  diff: string;
}

/** 补丁提案 */
export interface PatchProposal {
  id: string;
  taskId?: string | null;
  stepId?: string | null;
  approvalId?: string | null;
  summary: string;
  status: PatchProposalStatus;
  files: PatchFileChange[];
  unifiedDiff: string;
  requestedBy: string;
  createdAt: string;
  updatedAt: string;
  appliedAt?: string | null;
  appliedBy?: string | null;
  revertedAt?: string | null;
  revertedBy?: string | null;
}

/** 创建补丁提案响应 */
export interface CreatePatchProposalResponse {
  proposal: PatchProposal;
  approval: ApprovalRequest;
}

/** 补丁提案列表响应 */
export interface PatchProposalListResponse {
  proposals: PatchProposal[];
}

/** 应用已审批补丁请求 */
export interface ApplyApprovedPatchRequest {
  approvalId: string;
  autoRollbackOnVerificationFailure?: boolean;
}

/** 回滚已应用补丁请求 */
export interface RevertAppliedPatchRequest {
  patchId: string;
}

/** 应用补丁结果 */
export interface PatchApplyResult {
  patchId: string;
  status: PatchProposalStatus;
  files: string[];
  appliedAt: string;
  alreadyApplied: boolean;
  autoRollback?: PatchAutoRollbackResult | null;
}

/** 回滚补丁结果 */
export interface PatchRevertResult {
  patchId: string;
  status: PatchProposalStatus;
  files: string[];
  revertedAt: string;
  alreadyReverted: boolean;
}

/** 自动回滚结果 */
export interface PatchAutoRollbackResult {
  triggeredBy: string;
  reverted: boolean;
  error?: string | null;
  result?: PatchRevertResult | null;
}

/** 项目快照 */
export interface ProjectSnapshot {
  root: string;
  name: string;
  techStack: string[];
  manifests: ProjectManifest[];
  importantFiles: ProjectImportantFile[];
  recommendedCommands: ProjectCommand[];
  generatedAt: string;
}

/** Workspace 文件项 */
export interface WorkspaceEntry {
  path: string;
  name: string;
  isDir: boolean;
  extension?: string | null;
  sizeBytes: number;
  modifiedAt?: string | null;
}

/** 项目文件列表响应 */
export interface ProjectFileListResponse {
  files: WorkspaceEntry[];
}

/** 文件读取响应 */
export interface FileReadResponse {
  path: string;
  content: string;
  sizeBytes: number;
}

/** 搜索结果项 */
export interface SearchMatch {
  path: string;
  line: number;
  column: number;
  preview: string;
}

/** 搜索响应 */
export interface SearchResponse {
  query: string;
  matches: SearchMatch[];
  truncated: boolean;
}

/** 搜索请求 */
export interface SearchProjectTextRequest {
  query: string;
  maxResults?: number;
}

/** Tauri 后端结构化错误 */
export interface ApiErrorPayload {
  code?: string;
  message?: string;
  detail?: string | null;
  retryable?: boolean;
  requestId?: string;
  error?: string;
  reason?: string;
}

/** 健康检查响应 */
export interface HealthCheckResponse {
  /** 服务是否健康 */
  healthy: boolean;
  /** 版本号 */
  version: string;
  /** Agent 数量 */
  agentCount: number;
}

/** Agent 列表响应 */
export interface AgentListResponse {
  agents: AgentStatus[];
}

/** 设置项 */
export interface AppSettings {
  /** 当前模型名称 */
  model: string;
  /** API Key（可留空；桌面端后端可从 DEEPSEEK_API_KEY 用户环境变量读取） */
  apiKey: string;
  /** API 基础 URL */
  apiBaseUrl: string;
  /** 最大 Token 数 */
  maxTokens: number;
  /** 温度参数 */
  temperature: number;
}

/** 页面路由（简易状态切换） */
export type PageRoute = "chat" | "tasks" | "project" | "approvals" | "agents" | "settings";
