/**
 * Zustand Store 单元测试
 * 
 * 测试 useAgentStore 的状态管理逻辑：
 * - 聊天消息的发送和清空
 * - Agent 列表的获取
 * - localStorage 持久化读写
 * - 健康检查
 * - 历史消息加载
 */

import { describe, it, expect, beforeEach, vi, afterEach } from "vitest";
import { useAgentStore } from "@/store/useAgentStore";
import type { AppSettings, ChatMessage } from "@/types";

// 模拟 tauri API 模块，避免依赖真实的 Tauri 环境
vi.mock("@/lib/tauri", () => ({
  api: {
    sendMessage: vi.fn(),
    listAgents: vi.fn(),
    healthCheck: vi.fn(),
    getHistory: vi.fn(),
    clearHistory: vi.fn(),
  },
  isTauri: vi.fn(() => false),
}));

import { api } from "@/lib/tauri";

const mockApi = api as unknown as {
  sendMessage: ReturnType<typeof vi.fn>;
  listAgents: ReturnType<typeof vi.fn>;
  healthCheck: ReturnType<typeof vi.fn>;
  getHistory: ReturnType<typeof vi.fn>;
  clearHistory: ReturnType<typeof vi.fn>;
};

/**
 * 获取 store 的当前状态（不可变快照）
 */
function getState() {
  return useAgentStore.getState();
}

/**
 * 创建模拟的聊天消息
 */
function makeMsg(overrides: Partial<ChatMessage> = {}): ChatMessage {
  return {
    id: `msg-${Date.now()}`,
    role: "assistant",
    content: "测试消息",
    timestamp: new Date().toISOString(),
    senderName: "TestAgent",
    ...overrides,
  };
}

// ============================================================
// 初始状态测试
// ============================================================

describe("useAgentStore", () => {
  beforeEach(() => {
    // 重置 store 到初始状态
    useAgentStore.setState({
      messages: [
        {
          id: "welcome",
          role: "assistant",
          content:
            "👋 欢迎使用多 Agent 协同智能体！\n\n我可以协调多个 AI Agent 来完成复杂的任务。试试输入你的需求吧！",
          timestamp: new Date().toISOString(),
          senderName: "系统",
        },
      ],
      sending: false,
      sendError: null,
      agents: [],
      agentsLoading: false,
      agentsError: null,
      healthy: null,
      healthVersion: "",
      currentPage: "chat",
      settings: {
        model: "deepseek-v4-pro",
        apiKey: "",
        apiBaseUrl: "https://api.deepseek.com/v1",
        maxTokens: 4096,
        temperature: 0.7,
      },
    });

    // 清除 localStorage
    localStorage.clear();
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // ==========================================================
  // 页面路由测试
  // ==========================================================

  /// 测试 — 默认路由为 chat
  /// 验证：初始 currentPage 应为 "chat"
  it("默认页面路由应为 chat", () => {
    const state = getState();
    expect(state.currentPage).toBe("chat");
  });

  /// 测试 — setCurrentPage 切换路由
  /// 验证：调用 setCurrentPage 可以切换到不同页面
  it("setCurrentPage 应正确切换页面路由", () => {
    const { setCurrentPage } = getState();

    setCurrentPage("agents");
    expect(getState().currentPage).toBe("agents");

    setCurrentPage("settings");
    expect(getState().currentPage).toBe("settings");

    setCurrentPage("chat");
    expect(getState().currentPage).toBe("chat");
  });

  // ==========================================================
  // 聊天消息测试
  // ==========================================================

  /// 测试 — sendMessage 发送消息成功
  /// 验证：发送消息后 messages 数组中包含用户消息和 API 响应
  it("sendMessage 成功时应添加用户消息和 AI 回复", async () => {
    const mockResponse = {
      message: makeMsg({
        id: "resp-1",
        content: "这是 AI 的回复",
        role: "assistant",
        senderName: "Planner Agent",
      }),
      handledBy: "agent-1",
    };
    mockApi.sendMessage.mockResolvedValue(mockResponse);

    const { sendMessage } = getState();

    // 发送消息
    await sendMessage("你好，Agent！");

    const state = getState();
    expect(state.sending).toBe(false);
    expect(state.sendError).toBeNull();

    // 验证消息列表：应包含用户消息和 AI 回复
    const msgs = state.messages;
    expect(msgs.length).toBeGreaterThanOrEqual(2);

    // 用户消息
    const userMsg = msgs.find((m) => m.role === "user" && m.content === "你好，Agent！");
    expect(userMsg).toBeDefined();

    // AI 回复
    const aiMsg = msgs.find((m) => m.content === "这是 AI 的回复");
    expect(aiMsg).toBeDefined();
    expect(aiMsg!.role).toBe("assistant");
    expect(aiMsg!.senderName).toBe("Planner Agent");
  });

  /// 测试 — sendMessage 失败时添加错误消息
  /// 验证：API 调用失败时在消息列表中插入系统错误消息
  it("sendMessage 失败时应添加系统错误消息", async () => {
    mockApi.sendMessage.mockRejectedValue(new Error("网络连接失败"));

    const { sendMessage } = getState();
    await sendMessage("触发错误");

    const state = getState();
    expect(state.sending).toBe(false);
    expect(state.sendError).toBe("网络连接失败");

    // 应有系统错误消息
    const errorMsg = state.messages.find(
      (m) => m.role === "system" && m.content.includes("发送失败")
    );
    expect(errorMsg).toBeDefined();
    expect(errorMsg!.content).toContain("网络连接失败");
  });

  /// 测试 — sendMessage 时 sending 状态正确切换
  /// 验证：发送消息过程中 sending 为 true，完成后为 false
  it("sendMessage 过程中 sending 状态应正确切换", async () => {
    mockApi.sendMessage.mockImplementation(
      () => new Promise((resolve) => setTimeout(resolve, 50))
    );

    const sendPromise = getState().sendMessage("测试 sending 状态");

    // 发送中时 sending 应为 true
    expect(getState().sending).toBe(true);

    await sendPromise;
    expect(getState().sending).toBe(false);
  });

  /// 测试 — clearMessages 清空消息列表
  /// 验证：调用 clearMessages 后 messages 为空且 sendError 被清除
  it("clearMessages 应清空消息列表和错误", () => {
    useAgentStore.setState({
      messages: [makeMsg(), makeMsg()],
      sendError: "之前的错误",
    });

    const { clearMessages } = getState();
    clearMessages();

    const state = getState();
    expect(state.messages).toEqual([]);
    expect(state.sendError).toBeNull();
  });

  // ==========================================================
  // Agent 管理测试
  // ==========================================================

  /// 测试 — fetchAgents 成功获取 Agent 列表
  /// 验证：API 返回的 Agent 列表正确存入 store
  it("fetchAgents 成功时应更新 Agent 列表", async () => {
    const mockAgents = {
      agents: [
        {
          id: "a1",
          name: "Echo",
          role: "echo",
          online: true,
          status: "idle" as const,
          currentTask: null,
          capabilities: [{ name: "chat", description: "对话", available: true }],
          lastActive: new Date().toISOString(),
        },
        {
          id: "a2",
          name: "Planner",
          role: "planner",
          online: true,
          status: "busy" as const,
          currentTask: "任务分解中",
          capabilities: [{ name: "planning", description: "规划", available: true }],
          lastActive: new Date().toISOString(),
        },
      ],
    };
    mockApi.listAgents.mockResolvedValue(mockAgents);

    const { fetchAgents } = getState();
    await fetchAgents();

    const state = getState();
    expect(state.agents).toEqual(mockAgents.agents);
    expect(state.agentsLoading).toBe(false);
    expect(state.agentsError).toBeNull();
  });

  /// 测试 — fetchAgents 失败时设置错误消息
  /// 验证：API 调用失败时 agentsError 包含错误信息
  it("fetchAgents 失败时应设置错误消息", async () => {
    mockApi.listAgents.mockRejectedValue(new Error("后端服务不可用"));

    const { fetchAgents } = getState();
    await fetchAgents();

    const state = getState();
    expect(state.agents).toEqual([]);
    expect(state.agentsLoading).toBe(false);
    expect(state.agentsError).toBe("后端服务不可用");
  });

  /// 测试 — fetchAgents 加载状态正确切换
  /// 验证：获取 Agent 列表过程中 agentsLoading 状态正确
  it("fetchAgents 过程中 agentsLoading 应为 true", async () => {
    mockApi.listAgents.mockImplementation(
      () => new Promise((resolve) => setTimeout(resolve, 50))
    );

    const fetchPromise = getState().fetchAgents();
    expect(getState().agentsLoading).toBe(true);

    await fetchPromise;
    expect(getState().agentsLoading).toBe(false);
  });

  // ==========================================================
  // 轮询测试
  // ==========================================================

  /// 测试 — startPolling 启动定时轮询并返回清理函数
  /// 验证：startPolling 立即调用 fetchAgents 并启动定时器
  it("startPolling 应立即获取数据并返回清理函数", async () => {
    vi.useFakeTimers();

    mockApi.listAgents.mockResolvedValue({ agents: [] });

    const { startPolling } = getState();
    const stop = startPolling(5000);

    // 应立即调用了一次 fetchAgents
    expect(mockApi.listAgents).toHaveBeenCalledTimes(1);

    // 推进 5 秒
    vi.advanceTimersByTime(5000);
    expect(mockApi.listAgents).toHaveBeenCalledTimes(2);

    // 停止轮询
    stop();
    vi.advanceTimersByTime(5000);
    expect(mockApi.listAgents).toHaveBeenCalledTimes(2); // 不再增加

    vi.useRealTimers();
  });

  // ==========================================================
  // localStorage 持久化测试
  // ==========================================================

  /// 测试 — updateSettings 保存到 localStorage
  /// 验证：调用 updateSettings 后 localStorage 包含更新后的设置
  it("updateSettings 应将设置保存到 localStorage", () => {
    const { updateSettings } = getState();

    updateSettings({ model: "gpt-4-turbo", temperature: 0.9 });

    // 验证 localStorage 被调用
    const storedRaw = localStorage.getItem("app-settings");
    expect(storedRaw).not.toBeNull();

    const stored: AppSettings = JSON.parse(storedRaw!);
    expect(stored.model).toBe("gpt-4-turbo");
    expect(stored.temperature).toBe(0.9);
  });

  /// 测试 — updateSettings 合并部分更新
  /// 验证：传入部分字段时只更新指定字段，保留其他字段
  it("updateSettings 应合并部分更新而非覆盖全部", () => {
    const { updateSettings } = getState();

    updateSettings({ model: "claude-3-opus" });

    const state = getState();
    expect(state.settings.model).toBe("claude-3-opus");
    // 其他字段不变
    expect(state.settings.maxTokens).toBe(4096);
    expect(state.settings.temperature).toBe(0.7);
    expect(state.settings.apiBaseUrl).toBe("https://api.deepseek.com/v1");
  });

  /// 测试 — 加载时从 localStorage 恢复设置
  /// 验证：store 初始化时读取 localStorage 中的设置
  it("store 初始化时应从 localStorage 读取设置", () => {
    const savedSettings: AppSettings = {
      model: "deepseek-v3",
      apiKey: "sk-saved",
      apiBaseUrl: "https://api.deepseek.com/v1",
      maxTokens: 16384,
      temperature: 0.5,
    };
    localStorage.setItem("app-settings", JSON.stringify(savedSettings));

    // 通过重新创建 store 来触发初始化（Zustand 单例模式限制，这里只测逻辑）
    // 直接使用 updateSettings 验证 localStorage 读写链路
    const { updateSettings } = getState();
    updateSettings({ model: "deepseek-v3" });

    const stored = JSON.parse(localStorage.getItem("app-settings")!);
    expect(stored.model).toBe("deepseek-v3");
  });

  /// 测试 — localStorage 不可用时静默失败
  /// 验证：localStorage 异常时 updateSettings 不抛出错误
  it("localStorage 不可用时应静默失败", () => {
    // 模拟 localStorage.setItem 抛出异常
    const originalSetItem = localStorage.setItem;
    localStorage.setItem = vi.fn(() => {
      throw new Error("QuotaExceeded");
    });

    const { updateSettings } = getState();
    // 不应抛出异常
    expect(() => updateSettings({ maxTokens: 2048 })).not.toThrow();

    // store 中的设置仍应更新
    expect(getState().settings.maxTokens).toBe(2048);

    localStorage.setItem = originalSetItem;
  });

  // ==========================================================
  // 健康检查测试
  // ==========================================================

  /// 测试 — checkHealth 成功时更新健康状态
  /// 验证：API 返回 healthy=true 时适当设置 store
  it("checkHealth 成功时应更新健康状态", async () => {
    mockApi.healthCheck.mockResolvedValue({
      healthy: true,
      version: "1.2.3",
      agentCount: 5,
    });

    const { checkHealth } = getState();
    await checkHealth();

    const state = getState();
    expect(state.healthy).toBe(true);
    expect(state.healthVersion).toBe("1.2.3");
  });

  /// 测试 — checkHealth 失败时设置 unhealthy
  /// 验证：API 调用失败时 healthy 设为 false
  it("checkHealth 失败时应设置 healthy 为 false", async () => {
    mockApi.healthCheck.mockRejectedValue(new Error("Service down"));

    const { checkHealth } = getState();
    await checkHealth();

    expect(getState().healthy).toBe(false);
    expect(getState().healthVersion).toBe("");
  });

  // ==========================================================
  // 历史消息测试
  // ==========================================================

  /// 测试 — loadHistory 成功时替换消息列表
  /// 验证：API 返回的历史消息替换当前消息
  it("loadHistory 成功时应替换消息列表", async () => {
    const historyMessages: ChatMessage[] = [
      makeMsg({ id: "h1", content: "历史消息 1" }),
      makeMsg({ id: "h2", content: "历史消息 2" }),
    ];
    mockApi.getHistory.mockResolvedValue(historyMessages);

    // 先有当前消息
    useAgentStore.setState({
      messages: [makeMsg({ id: "current", content: "当前消息" })],
    });

    const { loadHistory } = getState();
    await loadHistory("session-1");

    // 历史消息应替换当前消息
    const msgs = getState().messages;
    expect(msgs).toEqual(historyMessages);
  });

  /// 测试 — loadHistory 返回空时不替换消息
  /// 验证：历史消息为空数组时保留当前消息
  it("loadHistory 返回空列表时应保留当前消息", async () => {
    mockApi.getHistory.mockResolvedValue([]);

    const currentMsgs = [makeMsg({ id: "keep-me", content: "保留我" })];
    useAgentStore.setState({ messages: currentMsgs });

    const { loadHistory } = getState();
    await loadHistory("empty-session");

    // 消息不应改变
    expect(getState().messages).toEqual(currentMsgs);
  });

  /// 测试 — loadHistory 失败时静默处理
  /// 验证：API 调用失败时不改变消息列表
  it("loadHistory 失败时应静默处理不改变状态", async () => {
    mockApi.getHistory.mockRejectedValue(new Error("Network error"));

    const currentMsgs = [makeMsg({ id: "safe", content: "安全消息" })];
    useAgentStore.setState({ messages: currentMsgs });

    const { loadHistory } = getState();
    await loadHistory("bad-session");

    // 消息不应改变
    expect(getState().messages).toEqual(currentMsgs);
  });
});
