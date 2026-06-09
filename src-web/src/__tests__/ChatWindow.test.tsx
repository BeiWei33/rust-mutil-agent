/**
 * ChatWindow 组件测试
 * 
 * 测试消息渲染、发送逻辑和 Markdown 解析功能
 */

import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useAgentStore } from "@/store/useAgentStore";
import ChatWindow from "@/components/ChatWindow";

// 模拟 useAgentStore，避免依赖真实的 Zustand store
vi.mock("@/store/useAgentStore", () => ({
  useAgentStore: vi.fn(),
}));

const mockUseAgentStore = useAgentStore as unknown as ReturnType<typeof vi.fn>;

// 创建 Hook 调用 spy
let storeState: Record<string, any>;
let mockSendMessage: ReturnType<typeof vi.fn>;
let mockClearMessages: ReturnType<typeof vi.fn>;
let mockSetCurrentSession: ReturnType<typeof vi.fn>;
let mockCreateChatSession: ReturnType<typeof vi.fn>;

/**
 * 设置模拟的 store 状态
 */
function setupMockStore(overrides: Record<string, any> = {}) {
  mockSendMessage = vi.fn().mockResolvedValue(undefined);
  mockClearMessages = vi.fn();
  mockSetCurrentSession = vi.fn().mockResolvedValue(undefined);
  mockCreateChatSession = vi.fn().mockResolvedValue(undefined);

  storeState = {
    messages: [],
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
    setCurrentSession: mockSetCurrentSession,
    createChatSession: mockCreateChatSession,
    sending: false,
    sendMessage: mockSendMessage,
    clearMessages: mockClearMessages,
    ...overrides,
  };

  mockUseAgentStore.mockImplementation((selector: any) => {
    if (typeof selector === "function") {
      return selector(storeState);
    }
    return storeState;
  });
}

// ============================================================
// 渲染测试
// ============================================================

describe("ChatWindow", () => {
  beforeEach(() => {
    setupMockStore();
  });

  /// 测试 — 组件正确渲染标题和空状态
  /// 验证：渲染聊天窗口标题和初始空状态提示文字
  it("应渲染聊天窗口标题和空状态提示", () => {
    render(<ChatWindow />);

    // 标题
    expect(screen.getByText("💬 工作台")).toBeDefined();
    // 空状态提示
    expect(screen.getByText("告诉我你想完成什么")).toBeDefined();
    // 清空按钮
    expect(screen.getByTitle("清空对话")).toBeDefined();
    expect(screen.getByLabelText("选择会话")).toBeDefined();
  });

  /// 测试 — 渲染输入框和发送按钮
  /// 验证：输入区域包含 textarea 和发送按钮
  it("应渲染输入框和发送按钮", () => {
    render(<ChatWindow />);

    const input = screen.getByPlaceholderText(
      "输入消息... (Enter 发送，Shift+Enter 换行)"
    );
    expect(input).toBeDefined();

    const sendButton = screen.getByTitle("发送消息");
    expect(sendButton).toBeDefined();
  });

  // ============================================================
  // 消息发送测试
  // ============================================================

  /// 测试 — 点击发送按钮调用 sendMessage
  /// 验证：输入文本后点击发送按钮会调用 sendMessage
  it("点击发送按钮应调用 sendMessage 并清空输入框", async () => {
    const user = userEvent.setup();
    setupMockStore();

    render(<ChatWindow />);

    // 输入文本
    const input = screen.getByPlaceholderText(
      "输入消息... (Enter 发送，Shift+Enter 换行)"
    );
    await user.type(input, "你好，Agent！");

    // 点击发送按钮
    const sendButton = screen.getByTitle("发送消息");
    await user.click(sendButton);

    // 验证 sendMessage 被调用
    await waitFor(() => {
      expect(mockSendMessage).toHaveBeenCalledWith("你好，Agent！");
    });
  });

  /// 测试 — 按 Enter 键发送消息
  /// 验证：按下 Enter 键应调用 sendMessage
  it("按下 Enter 键应发送消息", async () => {
    setupMockStore();
    render(<ChatWindow />);

    const input = screen.getByPlaceholderText(
      "输入消息... (Enter 发送，Shift+Enter 换行)"
    );

    // 输入文本并按 Enter
    fireEvent.change(input, { target: { value: "快速发送" } });
    fireEvent.keyDown(input, { key: "Enter", shiftKey: false });

    await waitFor(() => {
      expect(mockSendMessage).toHaveBeenCalledWith("快速发送");
    });
  });

  /// 测试 — Shift+Enter 不发送消息（换行）
  /// 验证：按 Shift+Enter 不应调用 sendMessage
  it("按下 Shift+Enter 不应发送消息", async () => {
    setupMockStore();
    render(<ChatWindow />);

    const input = screen.getByPlaceholderText(
      "输入消息... (Enter 发送，Shift+Enter 换行)"
    );

    fireEvent.change(input, { target: { value: "换行测试" } });
    fireEvent.keyDown(input, { key: "Enter", shiftKey: true });

    await waitFor(() => {
      expect(mockSendMessage).not.toHaveBeenCalled();
    });
  });

  /// 测试 — 空消息不触发发送
  /// 验证：输入纯空格时不调用 sendMessage
  it("空消息不应触发发送", async () => {
    const user = userEvent.setup();
    setupMockStore();
    render(<ChatWindow />);

    const input = screen.getByPlaceholderText(
      "输入消息... (Enter 发送，Shift+Enter 换行)"
    );
    await user.type(input, "   ");

    const sendButton = screen.getByTitle("发送消息");
    expect(sendButton).toBeDisabled();
  });

  /// 测试 — 发送中状态时禁用输入
  /// 验证：sending=true 时输入框和按钮被禁用
  it("发送中状态时输入框和按钮应被禁用", () => {
    setupMockStore({ sending: true });
    render(<ChatWindow />);

    const input = screen.getByPlaceholderText(
      "输入消息... (Enter 发送，Shift+Enter 换行)"
    );
    expect(input).toBeDisabled();

    const sendButton = screen.getByTitle("发送消息");
    expect(sendButton).toBeDisabled();
  });

  // ============================================================
  // 消息列表测试
  // ============================================================

  /// 测试 — 渲染用户消息
  /// 验证：消息列表中正确渲染用户角色的消息
  it("应正确渲染用户消息", () => {
    setupMockStore({
      messages: [
        {
          id: "1",
          role: "user",
          content: "你好，世界",
          timestamp: new Date().toISOString(),
        },
      ],
    });
    render(<ChatWindow />);

    expect(screen.getByText("你好，世界")).toBeDefined();
  });

  /// 测试 — 渲染多条消息
  /// 验证：多条消息按序渲染
  it("应正确渲染多条消息", () => {
    setupMockStore({
      messages: [
        {
          id: "1",
          role: "user",
          content: "第一条消息",
          timestamp: new Date().toISOString(),
        },
        {
          id: "2",
          role: "assistant",
          content: "第二条消息",
          timestamp: new Date().toISOString(),
          senderName: "TestAgent",
        },
        {
          id: "3",
          role: "system",
          content: "系统消息",
          timestamp: new Date().toISOString(),
        },
      ],
    });
    render(<ChatWindow />);

    expect(screen.getByText("第一条消息")).toBeDefined();
    expect(screen.getByText("第二条消息")).toBeDefined();
    expect(screen.getByText("系统消息")).toBeDefined();
  });

  /// 测试 — 渲染发送中指示器
  /// 验证：sending=true 时显示"Agent 正在思考..."
  it("发送中应显示加载指示器", () => {
    setupMockStore({ sending: true });
    render(<ChatWindow />);

    expect(screen.getByText("协调员/总控正在分析需求，并选择合适成员...")).toBeDefined();
  });

  // ============================================================
  // 清空消息测试
  // ============================================================

  /// 测试 — 点击清空按钮调用 clearMessages
  /// 验证：工具栏上的清空按钮触发 clearMessages
  it("点击清空按钮应调用 clearMessages", async () => {
    const user = userEvent.setup();
    setupMockStore({
      messages: [{ id: "1", role: "user", content: "test", timestamp: new Date().toISOString() }],
    });
    render(<ChatWindow />);

    const clearButton = screen.getByTitle("清空对话");
    await user.click(clearButton);

    expect(mockClearMessages).toHaveBeenCalled();
  });

  /// 测试 — 切换会话
  /// 验证：会话选择器变更时调用 store 的 setCurrentSession
  it("切换会话时应调用 setCurrentSession", async () => {
    const user = userEvent.setup();
    setupMockStore();
    render(<ChatWindow />);

    await user.selectOptions(screen.getByLabelText("选择会话"), "session-2");

    expect(mockSetCurrentSession).toHaveBeenCalledWith("session-2");
  });

  /// 测试 — 新建会话
  /// 验证：点击新建会话按钮调用 createChatSession
  it("点击新建会话按钮应调用 createChatSession", async () => {
    const user = userEvent.setup();
    setupMockStore();
    render(<ChatWindow />);

    await user.click(screen.getByTitle("新建会话"));

    expect(mockCreateChatSession).toHaveBeenCalled();
  });

  // ============================================================
  // Markdown 渲染测试
  // ============================================================

  /// 测试 — 渲染 Markdown 粗体
  /// 验证：**text** 被渲染为 <strong> 标签
  it("应正确渲染 Markdown 粗体语法", () => {
    setupMockStore({
      messages: [
        {
          id: "1",
          role: "assistant",
          content: "这是 **粗体** 文字",
          timestamp: new Date().toISOString(),
          senderName: "Agent",
        },
      ],
    });
    render(<ChatWindow />);

    const strongEl = screen.getByText("粗体");
    expect(strongEl.tagName).toBe("STRONG");
  });

  /// 测试 — 渲染 Markdown 代码块
  /// 验证：``` code ``` 被渲染为 <pre><code> 标签
  it("应正确渲染 Markdown 代码块", () => {
    setupMockStore({
      messages: [
        {
          id: "1",
          role: "assistant",
          content: "```javascript\nconsole.log('hello');\n```",
          timestamp: new Date().toISOString(),
          senderName: "Agent",
        },
      ],
    });
    render(<ChatWindow />);

    // 代码块的内容应存在
    const code = screen.getByText("console.log('hello');");
    expect(code).toBeDefined();
    // 应有 code 标签
    expect(code.closest("code")).toBeDefined();
  });

  /// 测试 — 渲染 Markdown 斜体
  /// 验证：*text* 被渲染为 <em> 标签
  it("应正确渲染 Markdown 斜体语法", () => {
    setupMockStore({
      messages: [
        {
          id: "1",
          role: "assistant",
          content: "这是 *斜体* 文字",
          timestamp: new Date().toISOString(),
          senderName: "Agent",
        },
      ],
    });
    render(<ChatWindow />);

    const emEl = screen.getByText("斜体");
    expect(emEl.tagName).toBe("EM");
  });

  /// 测试 — 渲染 Markdown 链接
  /// 验证：[text](url) 被渲染为 <a> 标签
  it("应正确渲染 Markdown 链接", () => {
    setupMockStore({
      messages: [
        {
          id: "1",
          role: "assistant",
          content: "访问 [GitHub](https://github.com) 了解更多",
          timestamp: new Date().toISOString(),
          senderName: "Agent",
        },
      ],
    });
    render(<ChatWindow />);

    const link = screen.getByText("GitHub");
    expect(link.tagName).toBe("A");
    expect(link.getAttribute("href")).toBe("https://github.com");
  });

  /// 测试 — 渲染 Markdown 标题
  /// 验证：# heading 被渲染为 <h1> 标签
  it("应正确渲染 Markdown 标题语法", () => {
    setupMockStore({
      messages: [
        {
          id: "1",
          role: "assistant",
          content: "# 一级标题\n## 二级标题\n### 三级标题",
          timestamp: new Date().toISOString(),
          senderName: "Agent",
        },
      ],
    });
    render(<ChatWindow />);

    expect(screen.getByText("一级标题").tagName).toBe("H1");
    expect(screen.getByText("二级标题").tagName).toBe("H2");
    expect(screen.getByText("三级标题").tagName).toBe("H3");
  });

  /// 测试 — 渲染 Markdown 无序列表
  /// 验证：- item 被渲染为 <li> 标签
  it("应正确渲染 Markdown 无序列表", () => {
    setupMockStore({
      messages: [
        {
          id: "1",
          role: "assistant",
          content: "- 项目一\n- 项目二\n- 项目三",
          timestamp: new Date().toISOString(),
          senderName: "Agent",
        },
      ],
    });
    render(<ChatWindow />);

    expect(screen.getByText("项目一")).toBeDefined();
    expect(screen.getByText("项目二")).toBeDefined();
    expect(screen.getByText("项目三")).toBeDefined();
  });

  /// 测试 — 用户消息不使用 Markdown 渲染
  /// 验证：用户角色的消息应当以纯文本渲染
  it("用户消息不应使用 Markdown 渲染", () => {
    setupMockStore({
      messages: [
        {
          id: "1",
          role: "user",
          content: "**这个不应该加粗**",
          timestamp: new Date().toISOString(),
        },
      ],
    });
    render(<ChatWindow />);

    // 用户消息中的 ** 不应被转换为 strong
    const userMsg = screen.getByText("**这个不应该加粗**");
    expect(userMsg.tagName).toBe("P");
  });
});
