/**
 * Zustand Store — 管理 Agent 状态和聊天消息
 */

import { create } from "zustand";
import type {
  AgentStatus,
  Message,
  ChatMessage,
  AppSettings,
  PageRoute,
  SendMessageRequest,
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
      };
      const res = await api.sendMessage(request);

      set((s) => ({
        messages: [...s.messages, res.message],
        sending: false,
        lastFailedSend: null,
      }));
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

  clearMessages: () =>
    set({
      messages: [],
      sendError: null,
      lastFailedSend: null,
    }),

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
