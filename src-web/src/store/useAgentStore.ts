/**
 * Zustand Store — 管理 Agent 状态和聊天消息
 */

import { create } from "zustand";
import type {
  AgentStatus,
  Message,
  ChatMessage,
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
  ApprovalRequest,
  ApprovalStatus,
  CreatePatchProposalRequest,
  PatchProposal,
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

  // ===== 任务管理 =====
  tasks: Task[];
  tasksLoading: boolean;
  tasksError: string | null;
  selectedTaskId: string | null;
  selectedTask: Task | null;
  taskEvents: TaskEvent[];
  taskEventsLoading: boolean;
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
  /** 获取审批请求列表 */
  fetchApprovals: (status?: ApprovalStatus) => Promise<void>;
  /** 审批或拒绝动作 */
  decideApproval: (approvalId: string, approved: boolean, note?: string) => Promise<void>;
  /** 执行已通过审批的项目命令 */
  runApprovedCommand: (approvalId: string) => Promise<void>;

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
  patchProposals: PatchProposal[];
  patchProposalLoading: boolean;
  patchProposalError: string | null;
  lastPatchProposal: PatchProposal | null;
  /** 加载项目快照和文件列表 */
  fetchProjectOverview: () => Promise<void>;
  /** 加载最近命令运行记录 */
  fetchCommandRuns: (limit?: number) => Promise<void>;
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
  sending: false,
  sendError: null,
  selectedAgentId: "",
  setSelectedAgentId: (agentId) => set({ selectedAgentId: agentId }),
  lastFailedSend: null,

  sendMessage: async (content, agentId) => {
    const userMsg: ChatMessage = {
      id: uid(),
      role: "user",
      content,
      timestamp: new Date().toISOString(),
    };

    set((s) => ({
      messages: [...s.messages, userMsg],
      sending: true,
      sendError: null,
    }));

    try {
      const request: SendMessageRequest = {
        content,
        agentId,
        routeMode: agentId ? "direct" : "auto",
        sessionId: DEFAULT_SESSION_ID,
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
    api.clearHistory(DEFAULT_SESSION_ID).catch(() => {
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
      if (history.length > 0) {
        set({ messages: history });
      }
    } catch {
      // 静默失败，保留当前消息
    }
  },

  // ===== 任务管理 =====
  tasks: [],
  tasksLoading: false,
  tasksError: null,
  selectedTaskId: null,
  selectedTask: null,
  taskEvents: [],
  taskEventsLoading: false,

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
    } catch (err: unknown) {
      set({
        approvalsError: getErrorMessage(err, "处理审批请求失败"),
        approvalDecisionLoadingId: null,
      });
    }
  },

  runApprovedCommand: async (approvalId) => {
    set({ approvalExecutionLoadingId: approvalId, approvalsError: null });
    try {
      const result = await api.runApprovedProjectCommand({ approvalId });
      set((s) => ({
        latestCommandRun: result,
        commandRuns: [result, ...s.commandRuns.filter((run) => run.id !== result.id)].slice(0, 10),
        approvalExecutionLoadingId: null,
        approvalsError: null,
      }));
    } catch (err: unknown) {
      set({
        approvalsError: getErrorMessage(err, "执行已审批命令失败"),
        approvalExecutionLoadingId: null,
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
