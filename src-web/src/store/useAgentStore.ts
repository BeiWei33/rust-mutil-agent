/**
 * Zustand Store — 管理 Agent 状态和聊天消息
 */

import { create } from "zustand";
import type {
  AgentStatus,
  Message,
  ChatMessage,
  ChatSession,
  AppSettings,
  LlmRequestSettings,
  PageRoute,
  SendMessageRequest,
  Task,
  TaskEvent,
  ProjectSnapshot,
  WorkspaceEntry,
  FileReadResponse,
  SearchMatch,
  ProjectCommandRunRequest,
  ProjectCommandRunResponse,
  ToolInvocationRecord,
  ApprovalRequest,
  ApprovalStatus,
  CreatePatchProposalRequest,
  PatchApplyResult,
  PatchProposal,
  PatchRevertResult,
} from "@/types";
import { api } from "@/lib/tauri";
import { getErrorDetail, getErrorMessage } from "@/lib/errors";

/** 生成唯一 ID */
function uid(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

/** 默认设置 */
const DEFAULT_SETTINGS: AppSettings = {
  model: "deepseek-v4-pro",
  apiKey: "",
  apiBaseUrl: "https://api.deepseek.com/v1",
  maxTokens: 4096,
  temperature: 0.7,
};

const DEFAULT_SESSION_ID = "default";
const CHAT_SESSIONS_STORAGE_KEY = "chat-sessions";
const CURRENT_SESSION_STORAGE_KEY = "chat-current-session";

function nowIso(): string {
  return new Date().toISOString();
}

function defaultChatSession(): ChatSession {
  const now = nowIso();
  return {
    id: DEFAULT_SESSION_ID,
    title: "默认会话",
    createdAt: now,
    updatedAt: now,
  };
}

function isChatSession(value: unknown): value is ChatSession {
  if (!value || typeof value !== "object") return false;
  const session = value as Partial<ChatSession>;
  return (
    typeof session.id === "string" &&
    session.id.trim().length > 0 &&
    typeof session.title === "string" &&
    session.title.trim().length > 0 &&
    typeof session.createdAt === "string" &&
    typeof session.updatedAt === "string"
  );
}

function normalizeSessions(sessions: ChatSession[]): ChatSession[] {
  const byId = new Map<string, ChatSession>();
  for (const session of sessions) {
    byId.set(session.id, {
      ...session,
      title: session.title.trim() || "未命名会话",
    });
  }
  if (!byId.has(DEFAULT_SESSION_ID)) {
    const fallback = defaultChatSession();
    if (sessions.length > 0) {
      fallback.createdAt = "1970-01-01T00:00:00.000Z";
      fallback.updatedAt = "1970-01-01T00:00:00.000Z";
    }
    byId.set(DEFAULT_SESSION_ID, fallback);
  }
  return [...byId.values()].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
}

function loadChatSessions(): ChatSession[] {
  try {
    const raw = localStorage.getItem(CHAT_SESSIONS_STORAGE_KEY);
    if (!raw) return normalizeSessions([defaultChatSession()]);
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return normalizeSessions([defaultChatSession()]);
    const sessions = parsed.filter(isChatSession);
    return normalizeSessions(sessions);
  } catch {
    return normalizeSessions([defaultChatSession()]);
  }
}

function persistChatSessions(sessions: ChatSession[]) {
  try {
    localStorage.setItem(CHAT_SESSIONS_STORAGE_KEY, JSON.stringify(sessions));
  } catch {
    // localStorage 不可用时仅保留内存状态。
  }
}

function loadCurrentSessionId(sessions: ChatSession[]): string {
  try {
    const stored = localStorage.getItem(CURRENT_SESSION_STORAGE_KEY);
    if (stored && sessions.some((session) => session.id === stored)) {
      return stored;
    }
  } catch {
    // 使用默认会话。
  }
  return sessions[0]?.id ?? DEFAULT_SESSION_ID;
}

function persistCurrentSessionId(sessionId: string) {
  try {
    localStorage.setItem(CURRENT_SESSION_STORAGE_KEY, sessionId);
  } catch {
    // localStorage 不可用时仅保留内存状态。
  }
}

function summarizeSessionTitle(content: string): string {
  const normalized = content.split(/\s+/).filter(Boolean).join(" ");
  if (!normalized) return "新会话";
  const chars = [...normalized];
  return chars.length > 18 ? `${chars.slice(0, 18).join("")}...` : normalized;
}

function createSession(title = "新会话"): ChatSession {
  const now = nowIso();
  return {
    id: `session-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    title,
    createdAt: now,
    updatedAt: now,
  };
}

function touchChatSession(
  sessions: ChatSession[],
  sessionId: string,
  titleSeed?: string
): ChatSession[] {
  const now = nowIso();
  let found = false;
  const updated = sessions.map((session) => {
    if (session.id !== sessionId) return session;
    found = true;
    const shouldRename = session.title === "新会话" && titleSeed;
    return {
      ...session,
      title: shouldRename ? summarizeSessionTitle(titleSeed) : session.title,
      updatedAt: now,
    };
  });
  if (!found) {
    updated.push({
      id: sessionId,
      title: titleSeed ? summarizeSessionTitle(titleSeed) : "新会话",
      createdAt: now,
      updatedAt: now,
    });
  }
  return normalizeSessions(updated);
}

/** 从 localStorage 加载设置 */
function loadSettings(): AppSettings {
  try {
    const raw = localStorage.getItem("app-settings");
    if (raw) return { ...DEFAULT_SETTINGS, ...JSON.parse(raw) };
  } catch {
    // 解析失败使用默认值
  }
  return { ...DEFAULT_SETTINGS };
}

function optionalText(value: string): string | undefined {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : undefined;
}

function buildLlmRequestSettings(settings: AppSettings): LlmRequestSettings {
  return {
    model: optionalText(settings.model),
    apiKey: optionalText(settings.apiKey),
    apiBaseUrl: optionalText(settings.apiBaseUrl),
    maxTokens: settings.maxTokens,
    temperature: settings.temperature,
  };
}

const initialChatSessions = loadChatSessions();
const initialCurrentSessionId = loadCurrentSessionId(initialChatSessions);

interface AgentState {
  // ===== 页面路由 =====
  currentPage: PageRoute;
  setCurrentPage: (page: PageRoute) => void;

  // ===== Agent 管理 =====
  agents: AgentStatus[];
  agentsLoading: boolean;
  agentsError: string | null;
  /** 获取 Agent 列表 */
  fetchAgents: () => Promise<void>;
  /** 开启 Agent 状态轮询 */
  startPolling: (intervalMs?: number) => () => void;

  // ===== 聊天消息 =====
  messages: ChatMessage[];
  chatSessions: ChatSession[];
  currentSessionId: string;
  sending: boolean;
  sendError: string | null;
  /** 当前选择的 Agent ID，空字符串表示自动分配 */
  selectedAgentId: string;
  /** 设置当前发送目标 */
  setSelectedAgentId: (agentId: string) => void;
  /** 最后一次发送失败的内容，用于重试 */
  lastFailedSend: { content: string; agentId?: string } | null;
  /** 发送消息 */
  sendMessage: (content: string, agentId?: string) => Promise<void>;
  /** 重试最后一次失败的消息 */
  retryLastFailedSend: () => Promise<void>;
  /** 清除发送错误 */
  clearSendError: () => void;
  /** 清空消息 */
  clearMessages: () => void;
  /** 加载历史消息 */
  loadHistory: (sessionId: string) => Promise<void>;
  /** 切换聊天会话 */
  setCurrentSession: (sessionId: string) => Promise<void>;
  /** 创建并切换到新会话 */
  createChatSession: (title?: string) => Promise<void>;

  // ===== 任务管理 =====
  tasks: Task[];
  tasksLoading: boolean;
  tasksError: string | null;
  selectedTaskId: string | null;
  selectedTask: Task | null;
  taskEvents: TaskEvent[];
  taskEventsLoading: boolean;
  skipTaskStepLoadingId: string | null;
  /** 获取任务列表 */
  fetchTasks: () => Promise<void>;
  /** 获取单个任务详情 */
  fetchTask: (taskId: string) => Promise<void>;
  /** 获取任务事件 */
  fetchTaskEvents: (taskId: string) => Promise<void>;
  /** 创建任务 */
  createTask: (content: string, agentId?: string) => Promise<void>;
  /** 取消任务 */
  cancelTask: (taskId: string, reason?: string) => Promise<void>;
  /** 重试任务 */
  retryTask: (taskId: string, reason?: string) => Promise<void>;
  /** 跳过单个任务步骤 */
  skipTaskStep: (taskId: string, stepId: string, reason?: string) => Promise<void>;
  /** 设置当前选中任务 */
  setSelectedTaskId: (taskId: string | null) => void;
  /** 开启任务轮询 */
  startTaskPolling: (intervalMs?: number) => () => void;

  // ===== 审批管理 =====
  approvals: ApprovalRequest[];
  approvalsLoading: boolean;
  approvalsError: string | null;
  approvalDecisionLoadingId: string | null;
  approvalExecutionLoadingId: string | null;
  patchApplyLoadingId: string | null;
  patchRevertLoadingId: string | null;
  lastPatchApplyResult: PatchApplyResult | null;
  lastPatchRevertResult: PatchRevertResult | null;
  /** 获取审批请求列表 */
  fetchApprovals: (status?: ApprovalStatus) => Promise<void>;
  /** 审批或拒绝动作 */
  decideApproval: (approvalId: string, approved: boolean, note?: string) => Promise<void>;
  /** 执行已通过审批的项目命令 */
  runApprovedCommand: (approvalId: string) => Promise<void>;
  /** 应用已通过审批的补丁 */
  applyApprovedPatch: (
    approvalId: string,
    options?: { autoRollbackOnVerificationFailure?: boolean }
  ) => Promise<void>;
  /** 回滚已应用的补丁 */
  revertAppliedPatch: (patchId: string) => Promise<void>;

  // ===== 项目理解 =====
  projectSnapshot: ProjectSnapshot | null;
  projectFiles: WorkspaceEntry[];
  projectLoading: boolean;
  projectError: string | null;
  selectedProjectFile: FileReadResponse | null;
  fileLoading: boolean;
  searchQuery: string;
  searchResults: SearchMatch[];
  searchTruncated: boolean;
  searchLoading: boolean;
  commandRunLoadingKey: string | null;
  commandRunError: string | null;
  commandApprovalLoading: boolean;
  commandApprovalError: string | null;
  lastCommandApproval: ApprovalRequest | null;
  latestCommandRun: ProjectCommandRunResponse | null;
  commandRuns: ProjectCommandRunResponse[];
  toolInvocations: ToolInvocationRecord[];
  toolInvocationError: string | null;
  patchProposals: PatchProposal[];
  patchProposalLoading: boolean;
  patchProposalError: string | null;
  lastPatchProposal: PatchProposal | null;
  /** 加载项目快照和文件列表 */
  fetchProjectOverview: () => Promise<void>;
  /** 加载最近命令运行记录 */
  fetchCommandRuns: (limit?: number) => Promise<void>;
  /** 加载最近工具调用记录 */
  fetchToolInvocations: (limit?: number) => Promise<void>;
  /** 加载最近补丁提案 */
  fetchPatchProposals: (limit?: number) => Promise<void>;
  /** 读取项目文件 */
  readProjectFile: (path: string) => Promise<void>;
  /** 搜索项目文本 */
  searchProjectText: (query: string) => Promise<void>;
  /** 运行受控项目命令 */
  runProjectCommand: (request: ProjectCommandRunRequest) => Promise<void>;
  /** 为非 allowlist 项目命令创建审批请求 */
  requestProjectCommandApproval: (request: ProjectCommandRunRequest) => Promise<void>;
  /** 创建补丁提案并生成审批请求 */
  createPatchProposal: (request: CreatePatchProposalRequest) => Promise<void>;
  /** 清除项目错误 */
  clearProjectError: () => void;

  // ===== 健康检查 =====
  healthy: boolean | null;
  healthVersion: string;
  checkHealth: () => Promise<void>;

  // ===== 设置 =====
  settings: AppSettings;
  updateSettings: (partial: Partial<AppSettings>) => void;
}

export const useAgentStore = create<AgentState>((set, get) => ({
  // ===== 页面路由 =====
  currentPage: "chat",
  setCurrentPage: (page) => set({ currentPage: page }),

  // ===== Agent 管理 =====
  agents: [],
  agentsLoading: false,
  agentsError: null,

  fetchAgents: async () => {
    set({ agentsLoading: true, agentsError: null });
    try {
      const res = await api.listAgents();
      set((s) => {
        const selectedStillValid =
          !s.selectedAgentId ||
          res.agents.some(
            (agent) => agent.id === s.selectedAgentId && agent.online && agent.selectable !== false
          );

        return {
          agents: res.agents,
          agentsLoading: false,
          selectedAgentId: selectedStillValid ? s.selectedAgentId : "",
        };
      });
    } catch (err: unknown) {
      set({
        agentsError: getErrorMessage(err, "获取 AI 团队成员失败"),
        agentsLoading: false,
      });
    }
  },

  startPolling: (intervalMs = 5000) => {
    // 立即获取一次
    get().fetchAgents();
    const timer = setInterval(() => {
      get().fetchAgents();
    }, intervalMs);
    // 返回清理函数
    return () => clearInterval(timer);
  },

  // ===== 聊天消息 =====
  messages: [
    {
      id: "welcome",
      role: "assistant",
      content:
        "👋 你好，我是你的 AI 数字工作组。\n\n你可以直接告诉我想完成什么；如果不确定该找谁，默认交给「协调员/总控」自动安排合适成员处理。",
      timestamp: new Date().toISOString(),
      senderName: "系统",
    },
  ],
  chatSessions: initialChatSessions,
  currentSessionId: initialCurrentSessionId,
  sending: false,
  sendError: null,
  selectedAgentId: "",
  setSelectedAgentId: (agentId) => set({ selectedAgentId: agentId }),
  lastFailedSend: null,

  sendMessage: async (content, agentId) => {
    const sessionId = get().currentSessionId || DEFAULT_SESSION_ID;
    const userMsg: ChatMessage = {
      id: uid(),
      role: "user",
      content,
      timestamp: new Date().toISOString(),
    };

    set((s) => ({
      messages: [...s.messages, userMsg],
      chatSessions: touchChatSession(s.chatSessions, sessionId, content),
      currentSessionId: sessionId,
      sending: true,
      sendError: null,
    }));
    persistCurrentSessionId(sessionId);
    persistChatSessions(get().chatSessions);

    try {
      const request: SendMessageRequest = {
        content,
        agentId,
        routeMode: agentId ? "direct" : "auto",
        sessionId,
        llmSettings: buildLlmRequestSettings(get().settings),
      };
      const res = await api.sendMessage(request);

      set((s) => ({
        messages: [...s.messages, res.message],
        sending: false,
        lastFailedSend: null,
      }));

      if (res.taskId) {
        get().fetchTask(res.taskId).catch(() => {
          // 任务刷新失败不影响聊天主流程。
        });
      }
    } catch (err: unknown) {
      const reason = getErrorMessage(err, "发送消息失败");
      const detail = getErrorDetail(err);
      const detailText = detail ? `\n\n技术详情：${detail}` : "";
      const errorMsg: ChatMessage = {
        id: uid(),
        role: "system",
        content: `❌ 发送失败：消息没有发送成功\n\n${reason}\n\n你可以点击“重试”，或切换为“自动分配”后再次发送。${detailText}`,
        timestamp: new Date().toISOString(),
        senderName: "系统",
      };
      set((s) => ({
        messages: [...s.messages, errorMsg],
        sending: false,
        sendError: reason,
        lastFailedSend: { content, agentId },
      }));
    }
  },

  retryLastFailedSend: async () => {
    const failed = get().lastFailedSend;
    if (!failed) return;
    await get().sendMessage(failed.content, failed.agentId);
  },

  clearSendError: () => set({ sendError: null, lastFailedSend: null }),

  clearMessages: () => {
    const sessionId = get().currentSessionId || DEFAULT_SESSION_ID;
    api.clearHistory(sessionId).catch(() => {
      // 清空本地消息不依赖后端历史清理成功。
    });
    set({
      messages: [],
      sendError: null,
      lastFailedSend: null,
    });
  },

  loadHistory: async (sessionId) => {
    try {
      const history = await api.getHistory(sessionId);
      if (history.length > 0 || sessionId !== DEFAULT_SESSION_ID) {
        set((s) => {
          const chatSessions = touchChatSession(s.chatSessions, sessionId);
          persistChatSessions(chatSessions);
          persistCurrentSessionId(sessionId);
          return {
            messages: history,
            chatSessions,
            currentSessionId: sessionId,
          };
        });
      }
    } catch {
      // 静默失败，保留当前消息
    }
  },

  setCurrentSession: async (sessionId) => {
    const target = sessionId.trim() || DEFAULT_SESSION_ID;
    set((s) => {
      const chatSessions = touchChatSession(s.chatSessions, target);
      persistChatSessions(chatSessions);
      persistCurrentSessionId(target);
      return {
        chatSessions,
        currentSessionId: target,
        messages: [],
        sendError: null,
        lastFailedSend: null,
      };
    });
    try {
      const history = await api.getHistory(target);
      set({ messages: history });
    } catch {
      // 切换会话失败时保留空会话视图，避免显示上一会话内容。
    }
  },

  createChatSession: async (title) => {
    const session = createSession(title);
    set((s) => {
      const chatSessions = normalizeSessions([session, ...s.chatSessions]);
      persistChatSessions(chatSessions);
      persistCurrentSessionId(session.id);
      return {
        chatSessions,
        currentSessionId: session.id,
        messages: [],
        sendError: null,
        lastFailedSend: null,
      };
    });
  },

  // ===== 任务管理 =====
  tasks: [],
  tasksLoading: false,
  tasksError: null,
  selectedTaskId: null,
  selectedTask: null,
  taskEvents: [],
  taskEventsLoading: false,
  skipTaskStepLoadingId: null,

  fetchTasks: async () => {
    set({ tasksLoading: true, tasksError: null });
    try {
      const res = await api.listTasks();
      set((s) => {
        const selectedStillExists =
          s.selectedTaskId && res.tasks.some((task) => task.id === s.selectedTaskId);
        const nextSelectedId =
          selectedStillExists ? s.selectedTaskId : res.tasks[0]?.id ?? null;
        const selectedTask =
          nextSelectedId ? res.tasks.find((task) => task.id === nextSelectedId) ?? null : null;

        return {
          tasks: res.tasks,
          tasksLoading: false,
          selectedTaskId: nextSelectedId,
          selectedTask,
        };
      });
    } catch (err: unknown) {
      set({
        tasksError: getErrorMessage(err, "获取任务列表失败"),
        tasksLoading: false,
      });
    }
  },

  fetchTask: async (taskId) => {
    try {
      const task = await api.getTask(taskId);
      set((s) => {
        const withoutOld = s.tasks.filter((item) => item.id !== taskId);
        const tasks = task ? [task, ...withoutOld] : withoutOld;
        return {
          tasks,
          selectedTaskId: taskId,
          selectedTask: task,
          tasksError: null,
        };
      });
      await get().fetchTaskEvents(taskId);
    } catch (err: unknown) {
      set({ tasksError: getErrorMessage(err, "获取任务详情失败") });
    }
  },

  fetchTaskEvents: async (taskId) => {
    set({ taskEventsLoading: true });
    try {
      const events = await api.getTaskEvents(taskId);
      set({ taskEvents: events, taskEventsLoading: false });
    } catch {
      set({ taskEventsLoading: false });
    }
  },

  createTask: async (content, agentId) => {
    set({ tasksLoading: true, tasksError: null });
    try {
      const res = await api.createTask({
        content,
        agentId,
        llmSettings: buildLlmRequestSettings(get().settings),
      });
      set((s) => {
        const task = res.task ?? null;
        const withoutOld = s.tasks.filter((item) => item.id !== res.taskId);
        return {
          tasks: task ? [task, ...withoutOld] : withoutOld,
          selectedTaskId: res.taskId,
          selectedTask: task,
          tasksLoading: false,
        };
      });
      await get().fetchTask(res.taskId);
    } catch (err: unknown) {
      set({
        tasksError: getErrorMessage(err, "创建任务失败"),
        tasksLoading: false,
      });
    }
  },

  cancelTask: async (taskId, reason = "用户取消任务。") => {
    set({ tasksError: null });
    try {
      const res = await api.cancelTask({ taskId, reason });
      set((s) => {
        const task = res.task ?? null;
        const tasks = task
          ? s.tasks.map((item) => (item.id === taskId ? task : item))
          : s.tasks.filter((item) => item.id !== taskId);
        return {
          tasks,
          selectedTask: s.selectedTaskId === taskId ? task : s.selectedTask,
          tasksError: null,
        };
      });
      await get().fetchTaskEvents(taskId);
    } catch (err: unknown) {
      set({ tasksError: getErrorMessage(err, "取消任务失败") });
    }
  },

  retryTask: async (taskId, reason = "用户重试任务。") => {
    set({ tasksError: null });
    try {
      const res = await api.retryTask({ taskId, reason });
      set((s) => {
        const task = res.task ?? null;
        const tasks = task
          ? s.tasks.map((item) => (item.id === taskId ? task : item))
          : s.tasks.filter((item) => item.id !== taskId);
        return {
          tasks,
          selectedTask: s.selectedTaskId === taskId ? task : s.selectedTask,
          tasksError: null,
        };
      });
      await get().fetchTaskEvents(taskId);
    } catch (err: unknown) {
      set({ tasksError: getErrorMessage(err, "重试任务失败") });
    }
  },

  skipTaskStep: async (taskId, stepId, reason = "用户跳过步骤。") => {
    set({ tasksError: null, skipTaskStepLoadingId: stepId });
    try {
      const res = await api.skipTaskStep({ taskId, stepId, reason });
      set((s) => {
        const task = res.task ?? null;
        const tasks = task
          ? s.tasks.map((item) => (item.id === taskId ? task : item))
          : s.tasks.filter((item) => item.id !== taskId);
        return {
          tasks,
          selectedTask: s.selectedTaskId === taskId ? task : s.selectedTask,
          tasksError: null,
          skipTaskStepLoadingId: null,
        };
      });
      await get().fetchTaskEvents(taskId);
    } catch (err: unknown) {
      set({
        tasksError: getErrorMessage(err, "跳过任务步骤失败"),
        skipTaskStepLoadingId: null,
      });
    }
  },

  setSelectedTaskId: (taskId) => {
    const task = taskId ? get().tasks.find((item) => item.id === taskId) ?? null : null;
    set({ selectedTaskId: taskId, selectedTask: task, taskEvents: [] });
    if (taskId) {
      get().fetchTask(taskId);
    }
  },

  startTaskPolling: (intervalMs = 2500) => {
    get().fetchTasks();
    const timer = setInterval(() => {
      const selectedTaskId = get().selectedTaskId;
      get().fetchTasks();
      if (selectedTaskId) {
        get().fetchTask(selectedTaskId);
      }
    }, intervalMs);
    return () => clearInterval(timer);
  },

  // ===== 审批管理 =====
  approvals: [],
  approvalsLoading: false,
  approvalsError: null,
  approvalDecisionLoadingId: null,
  approvalExecutionLoadingId: null,
  patchApplyLoadingId: null,
  patchRevertLoadingId: null,
  lastPatchApplyResult: null,
  lastPatchRevertResult: null,

  fetchApprovals: async (status) => {
    set({ approvalsLoading: true, approvalsError: null });
    try {
      const result = await api.listApprovalRequests(status, 50);
      set({ approvals: result.approvals, approvalsLoading: false });
    } catch (err: unknown) {
      set({
        approvalsError: getErrorMessage(err, "获取审批请求失败"),
        approvalsLoading: false,
      });
    }
  },

  decideApproval: async (approvalId, approved, note) => {
    set({ approvalDecisionLoadingId: approvalId, approvalsError: null });
    try {
      const updated = await api.approveAction({
        approvalId,
        approved,
        note,
        decidedBy: "user",
      });
      set((s) => {
        const approvals = updated
          ? s.approvals.map((approval) =>
              approval.id === approvalId ? updated : approval
            )
          : s.approvals.filter((approval) => approval.id !== approvalId);
        return {
          approvals,
          approvalDecisionLoadingId: null,
          approvalsError: null,
        };
      });
      if (updated?.actionType === "workspace.applyPatch") {
        get().fetchPatchProposals(10).catch(() => {
          // 审批状态已更新，补丁列表刷新失败不阻断主流程。
        });
      }
      if (updated?.taskId) {
        await get().fetchTask(updated.taskId).catch(() => {
          // 审批状态已更新，任务刷新失败不阻断主流程。
        });
      }
    } catch (err: unknown) {
      set({
        approvalsError: getErrorMessage(err, "处理审批请求失败"),
        approvalDecisionLoadingId: null,
      });
    }
  },

  runApprovedCommand: async (approvalId) => {
    const linkedTaskId =
      get().approvals.find((approval) => approval.id === approvalId)?.taskId ?? null;
    set({ approvalExecutionLoadingId: approvalId, approvalsError: null });
    try {
      const result = await api.runApprovedProjectCommand({ approvalId });
      set((s) => ({
        latestCommandRun: result,
        commandRuns: [result, ...s.commandRuns.filter((run) => run.id !== result.id)].slice(0, 10),
        approvalExecutionLoadingId: null,
        approvalsError: null,
      }));
      if (linkedTaskId) {
        await get().fetchTask(linkedTaskId).catch(() => {
          // 命令执行结果已返回，任务刷新失败不阻断主流程。
        });
      }
    } catch (err: unknown) {
      set({
        approvalsError: getErrorMessage(err, "执行已审批命令失败"),
        approvalExecutionLoadingId: null,
      });
    }
  },

  applyApprovedPatch: async (approvalId, options) => {
    set({ patchApplyLoadingId: approvalId, approvalsError: null });
    try {
      const request = {
        approvalId,
        ...(options?.autoRollbackOnVerificationFailure === undefined
          ? {}
          : {
              autoRollbackOnVerificationFailure:
                options.autoRollbackOnVerificationFailure,
            }),
      };
      const result = await api.applyApprovedPatch(request);
      const linkedTaskId =
        get().patchProposals.find((proposal) => proposal.id === result.patchId)?.taskId ??
        (get().lastPatchProposal?.id === result.patchId ? get().lastPatchProposal?.taskId : null);
      const rollbackResult = result.autoRollback?.result ?? null;
      set((s) => ({
        lastPatchApplyResult: result,
        lastPatchRevertResult: rollbackResult ?? s.lastPatchRevertResult,
        patchApplyLoadingId: null,
        approvalsError: null,
        patchProposals: s.patchProposals.map((proposal) =>
          proposal.id === result.patchId
            ? {
                ...proposal,
                status: result.status,
                appliedAt: result.appliedAt,
                revertedAt: rollbackResult?.revertedAt ?? proposal.revertedAt,
                updatedAt: rollbackResult?.revertedAt ?? result.appliedAt,
              }
            : proposal
        ),
        lastPatchProposal:
          s.lastPatchProposal?.id === result.patchId
            ? {
                ...s.lastPatchProposal,
                status: result.status,
                appliedAt: result.appliedAt,
                revertedAt: rollbackResult?.revertedAt ?? s.lastPatchProposal.revertedAt,
                updatedAt: rollbackResult?.revertedAt ?? result.appliedAt,
              }
            : s.lastPatchProposal,
      }));
      get().fetchPatchProposals(10).catch(() => {
        // 应用结果已返回，列表刷新失败不影响审批面板状态。
      });
      get().fetchCommandRuns(10).catch(() => {
        // 自动验证结果刷新失败不影响补丁应用主流程。
      });
      if (linkedTaskId) {
        await get().fetchTask(linkedTaskId).catch(() => {
          // 任务 artifact 刷新失败不影响补丁应用主流程。
        });
      }
    } catch (err: unknown) {
      set({
        approvalsError: getErrorMessage(err, "应用已审批补丁失败"),
        patchApplyLoadingId: null,
      });
    }
  },

  revertAppliedPatch: async (patchId) => {
    const linkedTaskId =
      get().patchProposals.find((proposal) => proposal.id === patchId)?.taskId ??
      (get().lastPatchProposal?.id === patchId ? get().lastPatchProposal?.taskId : null);
    set({ patchRevertLoadingId: patchId, approvalsError: null });
    try {
      const result = await api.revertAppliedPatch({ patchId });
      set((s) => ({
        lastPatchRevertResult: result,
        patchRevertLoadingId: null,
        approvalsError: null,
        patchProposals: s.patchProposals.map((proposal) =>
          proposal.id === result.patchId
            ? {
                ...proposal,
                status: result.status,
                revertedAt: result.revertedAt,
                updatedAt: result.revertedAt,
              }
            : proposal
        ),
        lastPatchProposal:
          s.lastPatchProposal?.id === result.patchId
            ? {
                ...s.lastPatchProposal,
                status: result.status,
                revertedAt: result.revertedAt,
                updatedAt: result.revertedAt,
              }
            : s.lastPatchProposal,
      }));
      get().fetchPatchProposals(10).catch(() => {
        // 回滚结果已返回，列表刷新失败不影响审批面板状态。
      });
      if (linkedTaskId) {
        await get().fetchTask(linkedTaskId).catch(() => {
          // 任务 artifact 刷新失败不影响回滚主流程。
        });
      }
    } catch (err: unknown) {
      set({
        approvalsError: getErrorMessage(err, "回滚已应用补丁失败"),
        patchRevertLoadingId: null,
      });
    }
  },

  // ===== 项目理解 =====
  projectSnapshot: null,
  projectFiles: [],
  projectLoading: false,
  projectError: null,
  selectedProjectFile: null,
  fileLoading: false,
  searchQuery: "",
  searchResults: [],
  searchTruncated: false,
  searchLoading: false,
  commandRunLoadingKey: null,
  commandRunError: null,
  commandApprovalLoading: false,
  commandApprovalError: null,
  lastCommandApproval: null,
  latestCommandRun: null,
  commandRuns: [],
  toolInvocations: [],
  toolInvocationError: null,
  patchProposals: [],
  patchProposalLoading: false,
  patchProposalError: null,
  lastPatchProposal: null,

  fetchProjectOverview: async () => {
    set({ projectLoading: true, projectError: null });
    try {
      const [snapshot, files] = await Promise.all([
        api.getProjectSnapshot(),
        api.listProjectFiles(500),
      ]);
      set({
        projectSnapshot: snapshot,
        projectFiles: files.files,
        projectLoading: false,
      });
      get().fetchCommandRuns(10).catch(() => {
        // 命令审计记录加载失败不影响项目概览。
      });
      get().fetchToolInvocations(10).catch(() => {
        // 工具审计记录加载失败不影响项目概览。
      });
      get().fetchPatchProposals(10).catch(() => {
        // 补丁提案加载失败不影响项目概览。
      });
    } catch (err: unknown) {
      set({
        projectError: getErrorMessage(err, "获取项目概览失败"),
        projectLoading: false,
      });
    }
  },

  fetchCommandRuns: async (limit = 10) => {
    try {
      const result = await api.listProjectCommandRuns(limit);
      set({ commandRuns: result.runs, commandRunError: null });
    } catch (err: unknown) {
      set({ commandRunError: getErrorMessage(err, "获取命令运行记录失败") });
    }
  },

  fetchToolInvocations: async (limit = 10) => {
    try {
      const result = await api.listToolInvocations(limit);
      set({ toolInvocations: result.invocations, toolInvocationError: null });
    } catch (err: unknown) {
      set({ toolInvocationError: getErrorMessage(err, "获取工具调用记录失败") });
    }
  },

  fetchPatchProposals: async (limit = 10) => {
    try {
      const result = await api.listPatchProposals(limit);
      set({ patchProposals: result.proposals, patchProposalError: null });
    } catch (err: unknown) {
      set({ patchProposalError: getErrorMessage(err, "获取补丁提案失败") });
    }
  },

  readProjectFile: async (path) => {
    set({ fileLoading: true, projectError: null });
    try {
      const file = await api.readProjectFile(path);
      set({ selectedProjectFile: file, fileLoading: false });
    } catch (err: unknown) {
      set({
        projectError: getErrorMessage(err, "读取项目文件失败"),
        fileLoading: false,
      });
    }
  },

  searchProjectText: async (query) => {
    const trimmed = query.trim();
    set({ searchQuery: query });
    if (!trimmed) {
      set({ searchResults: [], searchTruncated: false, searchLoading: false });
      return;
    }

    set({ searchLoading: true, projectError: null });
    try {
      const result = await api.searchProjectText({ query: trimmed, maxResults: 50 });
      set({
        searchResults: result.matches,
        searchTruncated: result.truncated,
        searchLoading: false,
      });
    } catch (err: unknown) {
      set({
        projectError: getErrorMessage(err, "搜索项目失败"),
        searchLoading: false,
      });
    }
  },

  runProjectCommand: async (request) => {
    const key = `${request.workingDir}:${request.command}`;
    set({ commandRunLoadingKey: key, commandRunError: null, latestCommandRun: null });
    try {
      const result = await api.runProjectCommand(request);
      set((s) => ({
        latestCommandRun: result,
        commandRuns: [result, ...s.commandRuns.filter((run) => run.id !== result.id)].slice(0, 10),
        commandRunLoadingKey: null,
        commandRunError: null,
      }));
    } catch (err: unknown) {
      set({
        commandRunError: getErrorMessage(err, "运行项目命令失败"),
        commandRunLoadingKey: null,
      });
    }
  },

  requestProjectCommandApproval: async (request) => {
    set({
      commandApprovalLoading: true,
      commandApprovalError: null,
      lastCommandApproval: null,
    });
    try {
      const approval = await api.requestProjectCommandApproval(request);
      set((s) => ({
        approvals: [approval, ...s.approvals.filter((item) => item.id !== approval.id)],
        lastCommandApproval: approval,
        commandApprovalLoading: false,
        commandApprovalError: null,
      }));
    } catch (err: unknown) {
      set({
        commandApprovalError: getErrorMessage(err, "创建命令审批失败"),
        commandApprovalLoading: false,
      });
    }
  },

  createPatchProposal: async (request) => {
    set({
      patchProposalLoading: true,
      patchProposalError: null,
      lastPatchProposal: null,
    });
    try {
      const result = await api.createPatchProposal({
        ...request,
        requestedBy: request.requestedBy || "ProjectPanel",
      });
      set((s) => ({
        patchProposals: [
          result.proposal,
          ...s.patchProposals.filter((proposal) => proposal.id !== result.proposal.id),
        ].slice(0, 10),
        approvals: [
          result.approval,
          ...s.approvals.filter((approval) => approval.id !== result.approval.id),
        ],
        lastPatchProposal: result.proposal,
        patchProposalLoading: false,
        patchProposalError: null,
      }));
    } catch (err: unknown) {
      set({
        patchProposalError: getErrorMessage(err, "创建补丁提案失败"),
        patchProposalLoading: false,
      });
    }
  },

  clearProjectError: () =>
    set({
      projectError: null,
      commandRunError: null,
      toolInvocationError: null,
      commandApprovalError: null,
      patchProposalError: null,
    }),

  // ===== 健康检查 =====
  healthy: null,
  healthVersion: "",

  checkHealth: async () => {
    try {
      const res = await api.healthCheck();
      set({ healthy: res.healthy, healthVersion: res.version });
    } catch {
      set({ healthy: false, healthVersion: "" });
    }
  },

  // ===== 设置 =====
  settings: loadSettings(),

  updateSettings: (partial) => {
    const newSettings = { ...get().settings, ...partial };
    // 持久化到 localStorage
    try {
      localStorage.setItem("app-settings", JSON.stringify(newSettings));
    } catch {
      // localStorage 不可用时静默失败
    }
    set({ settings: newSettings });
  },
}));
