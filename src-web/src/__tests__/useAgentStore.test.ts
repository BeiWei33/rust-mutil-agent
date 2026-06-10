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
import type { AppSettings, ChatMessage, ChatSession } from "@/types";

// 模拟 tauri API 模块，避免依赖真实的 Tauri 环境
vi.mock("@/lib/tauri", () => ({
  api: {
    sendMessage: vi.fn(),
    listAgents: vi.fn(),
    healthCheck: vi.fn(),
    getTask: vi.fn(),
    getTaskEvents: vi.fn(),
    getHistory: vi.fn(),
    clearHistory: vi.fn(),
    runProjectCommand: vi.fn(),
    requestProjectCommandApproval: vi.fn(),
    runApprovedProjectCommand: vi.fn(),
    listProjectCommandRuns: vi.fn(),
    listToolInvocations: vi.fn(),
    createPatchProposal: vi.fn(),
    listPatchProposals: vi.fn(),
    getPatchProposal: vi.fn(),
    applyApprovedPatch: vi.fn(),
    revertAppliedPatch: vi.fn(),
    listApprovalRequests: vi.fn(),
    approveAction: vi.fn(),
  },
  isTauri: vi.fn(() => false),
}));

import { api } from "@/lib/tauri";

const mockApi = api as unknown as {
  sendMessage: ReturnType<typeof vi.fn>;
  listAgents: ReturnType<typeof vi.fn>;
  healthCheck: ReturnType<typeof vi.fn>;
  getTask: ReturnType<typeof vi.fn>;
  getTaskEvents: ReturnType<typeof vi.fn>;
  getHistory: ReturnType<typeof vi.fn>;
  clearHistory: ReturnType<typeof vi.fn>;
  runProjectCommand: ReturnType<typeof vi.fn>;
  requestProjectCommandApproval: ReturnType<typeof vi.fn>;
  runApprovedProjectCommand: ReturnType<typeof vi.fn>;
  listProjectCommandRuns: ReturnType<typeof vi.fn>;
  listToolInvocations: ReturnType<typeof vi.fn>;
  createPatchProposal: ReturnType<typeof vi.fn>;
  listPatchProposals: ReturnType<typeof vi.fn>;
  getPatchProposal: ReturnType<typeof vi.fn>;
  applyApprovedPatch: ReturnType<typeof vi.fn>;
  revertAppliedPatch: ReturnType<typeof vi.fn>;
  listApprovalRequests: ReturnType<typeof vi.fn>;
  approveAction: ReturnType<typeof vi.fn>;
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
      chatSessions: [
        {
          id: "default",
          title: "默认会话",
          createdAt: "2026-06-09T00:00:00Z",
          updatedAt: "2026-06-09T00:00:00Z",
        },
      ],
      currentSessionId: "default",
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
      approvals: [],
      approvalsLoading: false,
      approvalsError: null,
      approvalDecisionLoadingId: null,
      approvalExecutionLoadingId: null,
      patchApplyLoadingId: null,
      patchRevertLoadingId: null,
      lastPatchApplyResult: null,
      lastPatchRevertResult: null,
      settings: {
        model: "deepseek-v4-pro",
        apiKey: "",
        apiBaseUrl: "https://api.deepseek.com/v1",
        maxTokens: 4096,
        temperature: 0.7,
        reasoningEffort: "",
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

    setCurrentPage("approvals");
    expect(getState().currentPage).toBe("approvals");

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
    expect(mockApi.sendMessage).toHaveBeenCalledWith(
      expect.objectContaining({
        content: "你好，Agent！",
        sessionId: "default",
        llmSettings: expect.objectContaining({
          reasoningEffort: undefined,
        }),
      })
    );
  });

  /// 测试 — sendMessage 使用当前会话
  /// 验证：切换到新会话后发送消息会携带当前 sessionId，并用首条消息更新会话标题
  it("sendMessage 应使用当前会话并更新新会话标题", async () => {
    const session: ChatSession = {
      id: "session-2",
      title: "新会话",
      createdAt: "2026-06-09T10:00:00Z",
      updatedAt: "2026-06-09T10:00:00Z",
    };
    useAgentStore.setState({
      chatSessions: [session],
      currentSessionId: "session-2",
    });
    mockApi.sendMessage.mockResolvedValue({
      message: makeMsg({ id: "resp-session-2", content: "收到" }),
      handledBy: "agent-1",
    });

    await getState().sendMessage("实现多会话管理");

    expect(mockApi.sendMessage).toHaveBeenCalledWith(
      expect.objectContaining({
        content: "实现多会话管理",
        sessionId: "session-2",
      })
    );
    expect(getState().chatSessions[0]).toMatchObject({
      id: "session-2",
      title: "实现多会话管理",
    });
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
    mockApi.clearHistory.mockResolvedValue(undefined);
    useAgentStore.setState({
      messages: [makeMsg(), makeMsg()],
      sendError: "之前的错误",
    });

    const { clearMessages } = getState();
    clearMessages();

    const state = getState();
    expect(state.messages).toEqual([]);
    expect(state.sendError).toBeNull();
    expect(mockApi.clearHistory).toHaveBeenCalledWith("default");
  });

  /// 测试 — clearMessages 使用当前会话
  /// 验证：清空操作只清理当前 sessionId 的后端历史
  it("clearMessages 应清理当前会话历史", () => {
    mockApi.clearHistory.mockResolvedValue(undefined);
    useAgentStore.setState({
      currentSessionId: "session-2",
      messages: [makeMsg()],
      sendError: "之前的错误",
    });

    getState().clearMessages();

    expect(getState().messages).toEqual([]);
    expect(getState().sendError).toBeNull();
    expect(mockApi.clearHistory).toHaveBeenCalledWith("session-2");
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
    expect(stored.apiKey).toBeUndefined();
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

  /// 测试 — 持久化设置不保存 API Key
  /// 验证：API Key 仅保留在内存态，不写入 localStorage
  it("updateSettings 不应将 API Key 保存到 localStorage", () => {
    const savedSettings: AppSettings = {
      model: "deepseek-v3",
      apiKey: "sk-saved",
      apiBaseUrl: "https://api.deepseek.com/v1",
      maxTokens: 16384,
      temperature: 0.5,
      reasoningEffort: "xhigh",
    };
    localStorage.setItem("app-settings", JSON.stringify(savedSettings));

    const { updateSettings } = getState();
    updateSettings({ apiKey: "sk-new-secret", model: "deepseek-v3" });

    const stored = JSON.parse(localStorage.getItem("app-settings")!);
    expect(stored.model).toBe("deepseek-v3");
    expect(stored.apiKey).toBeUndefined();
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
    await loadHistory("default");

    // 消息不应改变
    expect(getState().messages).toEqual(currentMsgs);
  });

  /// 测试 — 非默认空会话加载为空列表
  /// 验证：切到非 default 会话时，空历史不会继续展示上一会话消息
  it("loadHistory 对非默认空会话应清空当前消息", async () => {
    mockApi.getHistory.mockResolvedValue([]);
    useAgentStore.setState({
      messages: [makeMsg({ id: "previous", content: "上一会话" })],
      chatSessions: [
        {
          id: "default",
          title: "默认会话",
          createdAt: "2026-06-09T00:00:00Z",
          updatedAt: "2026-06-09T00:00:00Z",
        },
        {
          id: "session-empty",
          title: "空会话",
          createdAt: "2026-06-09T01:00:00Z",
          updatedAt: "2026-06-09T01:00:00Z",
        },
      ],
      currentSessionId: "session-empty",
    });

    await getState().loadHistory("session-empty");

    expect(getState().messages).toEqual([]);
    expect(getState().currentSessionId).toBe("session-empty");
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

  /// 测试 — setCurrentSession 切换并加载历史
  /// 验证：切换会话后当前消息被目标会话历史替换
  it("setCurrentSession 应切换当前会话并加载历史", async () => {
    const historyMessages: ChatMessage[] = [
      makeMsg({ id: "session-2-message", content: "第二会话历史" }),
    ];
    mockApi.getHistory.mockResolvedValue(historyMessages);
    useAgentStore.setState({
      messages: [makeMsg({ id: "default-message", content: "默认会话消息" })],
      chatSessions: [
        {
          id: "default",
          title: "默认会话",
          createdAt: "2026-06-09T00:00:00Z",
          updatedAt: "2026-06-09T00:00:00Z",
        },
        {
          id: "session-2",
          title: "第二会话",
          createdAt: "2026-06-09T01:00:00Z",
          updatedAt: "2026-06-09T01:00:00Z",
        },
      ],
      currentSessionId: "default",
    });

    await getState().setCurrentSession("session-2");

    expect(mockApi.getHistory).toHaveBeenCalledWith("session-2");
    expect(getState().currentSessionId).toBe("session-2");
    expect(getState().messages).toEqual(historyMessages);
  });

  /// 测试 — createChatSession 新建空会话
  /// 验证：新建后切换到新 sessionId，并清空当前消息
  it("createChatSession 应创建并切换到新会话", async () => {
    useAgentStore.setState({
      messages: [makeMsg({ id: "before-create", content: "旧消息" })],
      chatSessions: [
        {
          id: "default",
          title: "默认会话",
          createdAt: "2026-06-09T00:00:00Z",
          updatedAt: "2026-06-09T00:00:00Z",
        },
      ],
      currentSessionId: "default",
      sendError: "旧错误",
    });

    await getState().createChatSession();

    expect(getState().currentSessionId).toMatch(/^session-/);
    expect(getState().messages).toEqual([]);
    expect(getState().sendError).toBeNull();
    expect(getState().chatSessions.some((session) => session.title === "新会话")).toBe(true);
  });

  // ==========================================================
  // 项目命令测试
  // ==========================================================

  /// 测试 — runProjectCommand 成功时保存最近一次命令结果
  /// 验证：受控项目命令执行成功后 latestCommandRun 被更新
  it("runProjectCommand 成功时应保存最近一次命令结果", async () => {
    const result = {
      id: "run-1",
      command: "cargo check",
      workingDir: "src-tauri",
      exitCode: 0,
      success: true,
      stdout: "Finished dev",
      stderr: "",
      durationMs: 42,
      timedOut: false,
      stdoutTruncated: false,
      stderrTruncated: false,
      createdAt: "2026-06-09T10:00:00Z",
    };
    mockApi.runProjectCommand.mockResolvedValue(result);

    await getState().runProjectCommand({
      command: "cargo check",
      workingDir: "src-tauri",
    });

    expect(mockApi.runProjectCommand).toHaveBeenCalledWith({
      command: "cargo check",
      workingDir: "src-tauri",
    });
    expect(getState().commandRunLoadingKey).toBeNull();
    expect(getState().commandRunError).toBeNull();
    expect(getState().latestCommandRun).toEqual(result);
    expect(getState().commandRuns).toEqual([result]);
  });

  /// 测试 — runProjectCommand 失败时保存错误提示
  /// 验证：后端拒绝或运行失败时 commandRunError 可用于界面显示
  it("runProjectCommand 失败时应保存错误提示", async () => {
    mockApi.runProjectCommand.mockRejectedValue(new Error("命令不在受控允许列表中"));

    await getState().runProjectCommand({
      command: "cargo clippy",
      workingDir: "src-tauri",
    });

    expect(getState().commandRunLoadingKey).toBeNull();
    expect(getState().latestCommandRun).toBeNull();
    expect(getState().commandRunError).toBe("命令不在受控允许列表中");
  });

  /// 测试 — fetchCommandRuns 成功时保存最近命令记录
  /// 验证：项目面板可读取后端持久化的审计记录
  it("fetchCommandRuns 成功时应保存最近命令记录", async () => {
    const runs = [
      {
        id: "run-2",
        command: "npm test -- --run",
        workingDir: "src-web",
        exitCode: 0,
        success: true,
        stdout: "passed",
        stderr: "",
        durationMs: 88,
        timedOut: false,
        stdoutTruncated: false,
        stderrTruncated: false,
        createdAt: "2026-06-09T11:00:00Z",
      },
    ];
    mockApi.listProjectCommandRuns.mockResolvedValue({ runs });

    await getState().fetchCommandRuns(5);

    expect(mockApi.listProjectCommandRuns).toHaveBeenCalledWith(5);
    expect(getState().commandRuns).toEqual(runs);
    expect(getState().commandRunError).toBeNull();
  });

  /// 测试 — fetchToolInvocations 成功时保存最近工具调用记录
  /// 验证：项目面板可读取 ToolAgent 持久化审计记录
  it("fetchToolInvocations 成功时应保存最近工具调用记录", async () => {
    const invocations = [
      {
        id: "tool-run-1",
        taskId: "task-1",
        stepId: "task-1-1",
        approvalId: "approval-1",
        toolName: "file_read",
        argsSummary: { path: "README.md" },
        success: true,
        error: null,
        durationMs: 12,
        createdAt: "2026-06-09T11:30:00Z",
      },
    ];
    mockApi.listToolInvocations.mockResolvedValue({ invocations });

    await getState().fetchToolInvocations(5);

    expect(mockApi.listToolInvocations).toHaveBeenCalledWith(5);
    expect(getState().toolInvocations).toEqual(invocations);
    expect(getState().toolInvocationError).toBeNull();
  });

  /// 测试 — requestProjectCommandApproval 成功时保存审批请求
  /// 验证：非 allowlist 命令可以进入审批列表
  it("requestProjectCommandApproval 成功时应保存审批请求", async () => {
    const approval = {
      id: "approval-command-1",
      taskId: null,
      stepId: null,
      title: "运行项目命令：cargo clippy",
      reason: "需要审批。",
      risk: "high" as const,
      actionType: "runtime.runProjectCommand",
      actionPayload: {
        command: "cargo clippy",
        workingDir: "src-tauri",
        allowedByDefault: false,
      },
      status: "pending" as const,
      requestedBy: "ProjectPanel",
      decidedBy: null,
      decisionNote: null,
      createdAt: "2026-06-09T12:00:00Z",
      updatedAt: "2026-06-09T12:00:00Z",
      decidedAt: null,
    };
    mockApi.requestProjectCommandApproval.mockResolvedValue(approval);

    await getState().requestProjectCommandApproval({
      command: "cargo clippy",
      workingDir: "src-tauri",
    });

    expect(mockApi.requestProjectCommandApproval).toHaveBeenCalledWith({
      command: "cargo clippy",
      workingDir: "src-tauri",
    });
    expect(getState().commandApprovalLoading).toBe(false);
    expect(getState().commandApprovalError).toBeNull();
    expect(getState().lastCommandApproval).toEqual(approval);
    expect(getState().approvals).toEqual([approval]);
  });

  /// 测试 — requestProjectCommandApproval 失败时保存错误提示
  /// 验证：审批创建失败不会污染最近审批状态
  it("requestProjectCommandApproval 失败时应保存错误提示", async () => {
    mockApi.requestProjectCommandApproval.mockRejectedValue(new Error("该命令已在允许列表"));

    await getState().requestProjectCommandApproval({
      command: "cargo check",
      workingDir: "src-tauri",
    });

    expect(getState().commandApprovalLoading).toBe(false);
    expect(getState().lastCommandApproval).toBeNull();
    expect(getState().commandApprovalError).toBe("该命令已在允许列表");
  });

  /// 测试 — createPatchProposal 成功时保存补丁提案和审批请求
  /// 验证：项目面板生成 diff 后可进入审批队列
  it("createPatchProposal 成功时应保存补丁提案和审批请求", async () => {
    const proposal = {
      id: "patch-1",
      taskId: null,
      stepId: null,
      approvalId: "approval-patch-1",
      summary: "更新 README",
      status: "pendingApproval" as const,
      files: [
        {
          path: "README.md",
          changeType: "modify" as const,
          oldContent: "old",
          newContent: "new",
          diff: "diff --git a/README.md b/README.md\n-old\n+new",
        },
      ],
      unifiedDiff: "diff --git a/README.md b/README.md\n-old\n+new",
      requestedBy: "ProjectPanel",
      createdAt: "2026-06-09T12:00:00Z",
      updatedAt: "2026-06-09T12:00:00Z",
    };
    const approval = {
      id: "approval-patch-1",
      taskId: null,
      stepId: null,
      title: "应用补丁：更新 README",
      reason: "需要审批。",
      risk: "high" as const,
      actionType: "workspace.applyPatch",
      actionPayload: {
        patchId: "patch-1",
        unifiedDiff: proposal.unifiedDiff,
      },
      status: "pending" as const,
      requestedBy: "ProjectPanel",
      decidedBy: null,
      decisionNote: null,
      createdAt: "2026-06-09T12:00:00Z",
      updatedAt: "2026-06-09T12:00:00Z",
      decidedAt: null,
    };
    mockApi.createPatchProposal.mockResolvedValue({ proposal, approval });

    await getState().createPatchProposal({
      summary: "更新 README",
      files: [{ path: "README.md", oldContent: "old", newContent: "new" }],
    });

    expect(mockApi.createPatchProposal).toHaveBeenCalledWith({
      summary: "更新 README",
      files: [{ path: "README.md", oldContent: "old", newContent: "new" }],
      requestedBy: "ProjectPanel",
    });
    expect(getState().patchProposalLoading).toBe(false);
    expect(getState().patchProposalError).toBeNull();
    expect(getState().lastPatchProposal).toEqual(proposal);
    expect(getState().patchProposals).toEqual([proposal]);
    expect(getState().approvals).toEqual([approval]);
  });

  // ==========================================================
  // 审批测试
  // ==========================================================

  /// 测试 — fetchApprovals 成功时保存审批请求
  /// 验证：审批面板可读取 pending 请求列表
  it("fetchApprovals 成功时应保存审批请求", async () => {
    const approvals = [
      {
        id: "approval-1",
        taskId: "task-1",
        stepId: "step-1",
        title: "应用补丁",
        reason: "需要修改工作区文件。",
        risk: "high" as const,
        actionType: "workspace.applyPatch",
        actionPayload: { files: ["src/main.rs"] },
        status: "pending" as const,
        requestedBy: "Planner",
        decidedBy: null,
        decisionNote: null,
        createdAt: "2026-06-09T12:00:00Z",
        updatedAt: "2026-06-09T12:00:00Z",
        decidedAt: null,
      },
    ];
    mockApi.listApprovalRequests.mockResolvedValue({ approvals });

    await getState().fetchApprovals("pending");

    expect(mockApi.listApprovalRequests).toHaveBeenCalledWith("pending", 50);
    expect(getState().approvals).toEqual(approvals);
    expect(getState().approvalsLoading).toBe(false);
    expect(getState().approvalsError).toBeNull();
  });

  /// 测试 — decideApproval 成功时更新审批请求
  /// 验证：通过审批后 store 中对应请求状态被替换
  it("decideApproval 成功时应更新审批请求", async () => {
    const pending = {
      id: "approval-2",
      taskId: null,
      stepId: null,
      title: "运行命令",
      reason: "需要执行高风险命令。",
      risk: "critical" as const,
      actionType: "runtime.runCommand",
      actionPayload: { command: "custom command" },
      status: "pending" as const,
      requestedBy: "Tool",
      decidedBy: null,
      decisionNote: null,
      createdAt: "2026-06-09T12:00:00Z",
      updatedAt: "2026-06-09T12:00:00Z",
      decidedAt: null,
    };
    const approved = {
      ...pending,
      status: "approved" as const,
      decidedBy: "user",
      decisionNote: "同意",
      updatedAt: "2026-06-09T12:01:00Z",
      decidedAt: "2026-06-09T12:01:00Z",
    };
    useAgentStore.setState({ approvals: [pending] });
    mockApi.approveAction.mockResolvedValue(approved);

    await getState().decideApproval("approval-2", true, "同意");

    expect(mockApi.approveAction).toHaveBeenCalledWith({
      approvalId: "approval-2",
      approved: true,
      note: "同意",
      decidedBy: "user",
    });
    expect(getState().approvalDecisionLoadingId).toBeNull();
    expect(getState().approvals).toEqual([approved]);
  });

  /// 测试 — runApprovedCommand 成功时写入最近命令结果
  /// 验证：已审批命令执行后可在命令审计列表中看到关联 approvalId
  it("runApprovedCommand 成功时应保存命令审计结果", async () => {
    const run = {
      id: "run-approved-1",
      approvalId: "approval-2",
      command: "cargo clippy",
      workingDir: "src-tauri",
      exitCode: 0,
      success: true,
      stdout: "ok",
      stderr: "",
      durationMs: 120,
      timedOut: false,
      stdoutTruncated: false,
      stderrTruncated: false,
      createdAt: "2026-06-09T12:02:00Z",
    };
    mockApi.runApprovedProjectCommand.mockResolvedValue(run);

    await getState().runApprovedCommand("approval-2");

    expect(mockApi.runApprovedProjectCommand).toHaveBeenCalledWith({
      approvalId: "approval-2",
    });
    expect(getState().approvalExecutionLoadingId).toBeNull();
    expect(getState().approvalsError).toBeNull();
    expect(getState().latestCommandRun).toEqual(run);
    expect(getState().commandRuns).toEqual([run]);
  });

  /// 测试 — applyApprovedPatch 成功时更新补丁提案状态
  /// 验证：已审批补丁应用后 store 中对应 proposal 进入 applied
  it("applyApprovedPatch 成功时应更新补丁提案状态", async () => {
    const proposal = {
      id: "patch-apply-1",
      taskId: null,
      stepId: null,
      approvalId: "approval-patch-apply-1",
      summary: "更新 README",
      status: "approved" as const,
      files: [
        {
          path: "README.md",
          changeType: "modify" as const,
          oldContent: "old",
          newContent: "new",
          diff: "diff --git a/README.md b/README.md\n-old\n+new",
        },
      ],
      unifiedDiff: "diff --git a/README.md b/README.md\n-old\n+new",
      requestedBy: "ProjectPanel",
      createdAt: "2026-06-09T12:00:00Z",
      updatedAt: "2026-06-09T12:01:00Z",
      appliedAt: null,
      appliedBy: null,
    };
    const result = {
      patchId: "patch-apply-1",
      status: "applied" as const,
      files: ["README.md"],
      appliedAt: "2026-06-09T12:02:00Z",
      alreadyApplied: false,
    };
    useAgentStore.setState({ patchProposals: [proposal], lastPatchProposal: proposal });
    mockApi.applyApprovedPatch.mockResolvedValue(result);
    mockApi.listPatchProposals.mockResolvedValue({
      proposals: [
        {
          ...proposal,
          status: "applied" as const,
          appliedAt: result.appliedAt,
          updatedAt: result.appliedAt,
        },
      ],
    });
    mockApi.listProjectCommandRuns.mockResolvedValue({ runs: [] });

    await getState().applyApprovedPatch("approval-patch-apply-1");

    expect(mockApi.applyApprovedPatch).toHaveBeenCalledWith({
      approvalId: "approval-patch-apply-1",
    });
    expect(mockApi.listProjectCommandRuns).toHaveBeenCalledWith(10);
    expect(getState().patchApplyLoadingId).toBeNull();
    expect(getState().approvalsError).toBeNull();
    expect(getState().lastPatchApplyResult).toEqual(result);
    expect(getState().patchProposals[0]).toMatchObject({
      id: "patch-apply-1",
      status: "applied",
      appliedAt: result.appliedAt,
    });
    expect(getState().lastPatchProposal).toMatchObject({
      id: "patch-apply-1",
      status: "applied",
    });
  });

  /// 测试 — applyApprovedPatch 开启验证失败自动回滚时同步回滚状态
  /// 验证：后端返回 autoRollback 后，store 同步 proposal/revert result
  it("applyApprovedPatch 自动回滚成功时应更新补丁为 reverted", async () => {
    const proposal = {
      id: "patch-auto-rollback-1",
      taskId: null,
      stepId: null,
      approvalId: "approval-auto-rollback-1",
      summary: "失败后回滚 README",
      status: "approved" as const,
      files: [
        {
          path: "README.md",
          changeType: "modify" as const,
          oldContent: "old",
          newContent: "new",
          diff: "diff --git a/README.md b/README.md\n-old\n+new",
        },
      ],
      unifiedDiff: "diff --git a/README.md b/README.md\n-old\n+new",
      requestedBy: "ProjectPanel",
      createdAt: "2026-06-09T12:00:00Z",
      updatedAt: "2026-06-09T12:01:00Z",
      appliedAt: null,
      appliedBy: null,
      revertedAt: null,
      revertedBy: null,
    };
    const rollback = {
      patchId: "patch-auto-rollback-1",
      status: "reverted" as const,
      files: ["README.md"],
      revertedAt: "2026-06-09T12:04:00Z",
      alreadyReverted: false,
    };
    const result = {
      patchId: "patch-auto-rollback-1",
      status: "reverted" as const,
      files: ["README.md"],
      appliedAt: "2026-06-09T12:02:00Z",
      alreadyApplied: false,
      autoRollback: {
        triggeredBy: "verificationFailure",
        reverted: true,
        error: null,
        result: rollback,
      },
    };
    useAgentStore.setState({ patchProposals: [proposal], lastPatchProposal: proposal });
    mockApi.applyApprovedPatch.mockResolvedValue(result);
    mockApi.listPatchProposals.mockResolvedValue({ proposals: [] });
    mockApi.listProjectCommandRuns.mockResolvedValue({ runs: [] });

    await getState().applyApprovedPatch("approval-auto-rollback-1", {
      autoRollbackOnVerificationFailure: true,
    });

    expect(mockApi.applyApprovedPatch).toHaveBeenCalledWith({
      approvalId: "approval-auto-rollback-1",
      autoRollbackOnVerificationFailure: true,
    });
    expect(getState().lastPatchRevertResult).toEqual(rollback);
    expect(getState().patchProposals[0]).toMatchObject({
      id: "patch-auto-rollback-1",
      status: "reverted",
      revertedAt: rollback.revertedAt,
      updatedAt: rollback.revertedAt,
    });
  });

  /// 测试 — applyApprovedPatch 成功时刷新关联任务
  /// 验证：补丁提案带 taskId 时会拉取最新任务和事件，显示 artifact 时间线
  it("applyApprovedPatch 成功时应刷新关联任务和事件", async () => {
    const proposal = {
      id: "patch-task-1",
      taskId: "task-1",
      stepId: "task-1-2",
      approvalId: "approval-patch-task-1",
      summary: "更新任务文件",
      status: "approved" as const,
      files: [
        {
          path: "README.md",
          changeType: "modify" as const,
          oldContent: "old",
          newContent: "new",
          diff: "diff --git a/README.md b/README.md\n-old\n+new",
        },
      ],
      unifiedDiff: "diff --git a/README.md b/README.md\n-old\n+new",
      requestedBy: "ProjectPanel",
      createdAt: "2026-06-09T12:00:00Z",
      updatedAt: "2026-06-09T12:01:00Z",
      appliedAt: null,
      appliedBy: null,
    };
    const task = {
      id: "task-1",
      title: "更新任务文件",
      userGoal: "更新任务文件",
      status: "completed" as const,
      steps: [],
      artifacts: [
        {
          kind: "patchApplied",
          patchId: "patch-task-1",
          summary: "更新任务文件",
          files: ["README.md"],
          appliedAt: "2026-06-09T12:02:00Z",
        },
        {
          kind: "patchVerification",
          patchId: "patch-task-1",
          summary: "更新任务文件",
          status: "passed",
          commandCount: 1,
          successCount: 1,
          failedCount: 0,
          verifiedAt: "2026-06-09T12:03:00Z",
          runs: [
            {
              id: "run-1",
              command: "cargo test",
              workingDir: "src-tauri",
              success: true,
              exitCode: 0,
              durationMs: 1200,
              timedOut: false,
            },
          ],
        },
      ],
      output: "完成",
      error: null,
      createdAt: "2026-06-09T12:00:00Z",
      updatedAt: "2026-06-09T12:02:00Z",
    };
    const event = {
      id: "event-artifact-1",
      taskId: "task-1",
      stepId: "task-1-2",
      kind: "artifactCreated" as const,
      message: "补丁已应用：更新任务文件",
      payload: task.artifacts[0],
      createdAt: "2026-06-09T12:02:00Z",
    };
    const result = {
      patchId: "patch-task-1",
      status: "applied" as const,
      files: ["README.md"],
      appliedAt: "2026-06-09T12:02:00Z",
      alreadyApplied: false,
    };
    useAgentStore.setState({ patchProposals: [proposal], lastPatchProposal: proposal });
    mockApi.applyApprovedPatch.mockResolvedValue(result);
    mockApi.listPatchProposals.mockResolvedValue({ proposals: [] });
    mockApi.listProjectCommandRuns.mockResolvedValue({
      runs: [
        {
          id: "run-1",
          approvalId: null,
          command: "cargo test",
          workingDir: "src-tauri",
          exitCode: 0,
          success: true,
          stdout: "ok",
          stderr: "",
          durationMs: 1200,
          timedOut: false,
          stdoutTruncated: false,
          stderrTruncated: false,
          createdAt: "2026-06-09T12:03:00Z",
        },
      ],
    });
    mockApi.getTask.mockResolvedValue(task);
    mockApi.getTaskEvents.mockResolvedValue([event]);

    await getState().applyApprovedPatch("approval-patch-task-1");

    expect(mockApi.listProjectCommandRuns).toHaveBeenCalledWith(10);
    expect(mockApi.getTask).toHaveBeenCalledWith("task-1");
    expect(mockApi.getTaskEvents).toHaveBeenCalledWith("task-1");
    expect(getState().selectedTask).toEqual(task);
    expect(getState().taskEvents).toEqual([event]);
  });

  /// 测试 — revertAppliedPatch 成功时刷新关联任务
  /// 验证：已应用补丁回滚后 proposal 进入 reverted，并拉取最新 artifact 时间线
  it("revertAppliedPatch 成功时应更新补丁状态并刷新关联任务", async () => {
    const proposal = {
      id: "patch-revert-1",
      taskId: "task-1",
      stepId: "task-1-2",
      approvalId: "approval-patch-revert-1",
      summary: "回滚任务文件",
      status: "applied" as const,
      files: [
        {
          path: "README.md",
          changeType: "modify" as const,
          oldContent: "old",
          newContent: "new",
          diff: "diff --git a/README.md b/README.md\n-old\n+new",
        },
      ],
      unifiedDiff: "diff --git a/README.md b/README.md\n-old\n+new",
      requestedBy: "ProjectPanel",
      createdAt: "2026-06-09T12:00:00Z",
      updatedAt: "2026-06-09T12:02:00Z",
      appliedAt: "2026-06-09T12:02:00Z",
      appliedBy: "user",
      revertedAt: null,
      revertedBy: null,
    };
    const task = {
      id: "task-1",
      title: "回滚任务文件",
      userGoal: "回滚任务文件",
      status: "completed" as const,
      steps: [],
      artifacts: [
        {
          kind: "patchReverted",
          patchId: "patch-revert-1",
          summary: "回滚任务文件",
          files: ["README.md"],
          revertedAt: "2026-06-09T12:04:00Z",
        },
      ],
      output: "完成",
      error: null,
      createdAt: "2026-06-09T12:00:00Z",
      updatedAt: "2026-06-09T12:04:00Z",
    };
    const event = {
      id: "event-revert-1",
      taskId: "task-1",
      stepId: "task-1-2",
      kind: "artifactCreated" as const,
      message: "补丁已回滚：回滚任务文件",
      payload: task.artifacts[0],
      createdAt: "2026-06-09T12:04:00Z",
    };
    const result = {
      patchId: "patch-revert-1",
      status: "reverted" as const,
      files: ["README.md"],
      revertedAt: "2026-06-09T12:04:00Z",
      alreadyReverted: false,
    };
    useAgentStore.setState({ patchProposals: [proposal], lastPatchProposal: proposal });
    mockApi.revertAppliedPatch.mockResolvedValue(result);
    mockApi.listPatchProposals.mockResolvedValue({
      proposals: [
        {
          ...proposal,
          status: "reverted" as const,
          revertedAt: result.revertedAt,
          updatedAt: result.revertedAt,
        },
      ],
    });
    mockApi.getTask.mockResolvedValue(task);
    mockApi.getTaskEvents.mockResolvedValue([event]);

    await getState().revertAppliedPatch("patch-revert-1");

    expect(mockApi.revertAppliedPatch).toHaveBeenCalledWith({
      patchId: "patch-revert-1",
    });
    expect(mockApi.getTask).toHaveBeenCalledWith("task-1");
    expect(mockApi.getTaskEvents).toHaveBeenCalledWith("task-1");
    expect(getState().patchRevertLoadingId).toBeNull();
    expect(getState().lastPatchRevertResult).toEqual(result);
    expect(getState().patchProposals[0]).toMatchObject({
      id: "patch-revert-1",
      status: "reverted",
      revertedAt: result.revertedAt,
    });
    expect(getState().selectedTask).toEqual(task);
    expect(getState().taskEvents).toEqual([event]);
  });
});
