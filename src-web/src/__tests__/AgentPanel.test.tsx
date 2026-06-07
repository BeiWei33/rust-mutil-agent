/**
 * AgentPanel 组件测试
 * 
 * 测试 Agent 面板的列表渲染、状态显示、轮询和数据更新功能
 */

import { describe, it, expect, beforeEach, vi, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useAgentStore } from "@/store/useAgentStore";
import AgentPanel from "@/components/AgentPanel";
import type { AgentStatus } from "@/types";

// 模拟 useAgentStore
vi.mock("@/store/useAgentStore", () => ({
  useAgentStore: vi.fn(),
}));

const mockUseAgentStore = useAgentStore as unknown as ReturnType<typeof vi.fn>;

let storeState: Record<string, any>;
let mockFetchAgents: ReturnType<typeof vi.fn>;
let mockStartPolling: ReturnType<typeof vi.fn>;
let mockStopPolling: ReturnType<typeof vi.fn>;

/**
 * 创建模拟 Agent 数据
 */
function createMockAgent(overrides: Partial<AgentStatus> = {}): AgentStatus {
  return {
    id: "agent-1",
    name: "测试 Agent",
    role: "tester",
    online: true,
    status: "idle",
    currentTask: null,
    capabilities: [
      { name: "测试", description: "执行测试任务", available: true },
    ],
    lastActive: new Date().toISOString(),
    ...overrides,
  };
}

/**
 * 设置模拟的 store 状态
 */
function setupMockStore(overrides: Record<string, any> = {}) {
  mockFetchAgents = vi.fn().mockResolvedValue(undefined);
  mockStopPolling = vi.fn();
  mockStartPolling = vi.fn(() => mockStopPolling);

  storeState = {
    agents: [],
    agentsLoading: false,
    agentsError: null,
    fetchAgents: mockFetchAgents,
    startPolling: mockStartPolling,
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

describe("AgentPanel", () => {
  beforeEach(() => {
    setupMockStore();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  /// 测试 — 组件正确渲染标题和统计信息
  /// 验证：面板标题、Agent 数量统计和刷新按钮正确渲染
  it("应正确渲染标题和统计概览", () => {
    render(<AgentPanel />);

    // 标题
    expect(screen.getByText("🤖 AI 团队成员")).toBeDefined();
    // 统计
    expect(screen.getByText("成员总数")).toBeDefined();
    expect(screen.getByText("可工作")).toBeDefined();
    expect(screen.getByText("正在处理")).toBeDefined();
    // 刷新按钮
    expect(screen.getByRole("button", { name: "刷新" })).toBeDefined();
  });

  /// 测试 — 统计数值显示正确
  /// 验证：Agent 总数、在线数、忙碌数正确显示
  it("应正确显示 Agent 统计数值", () => {
    setupMockStore({
      agents: [
        createMockAgent({ id: "a1", name: "A1", online: true, status: "idle" }),
        createMockAgent({ id: "a2", name: "A2", online: true, status: "busy" }),
        createMockAgent({ id: "a3", name: "A3", online: false, status: "offline" }),
      ],
    });
    render(<AgentPanel />);

    // 总数 = 3
    expect(screen.getByText("3")).toBeDefined();
    // 在线 = 2
    const onlineValues = screen.getAllByText("2");
    expect(onlineValues.length).toBeGreaterThan(0);
    // 忙碌 = 1
    expect(screen.getByText("1")).toBeDefined();
  });

  // ============================================================
  // Agent 列表测试
  // ============================================================

  /// 测试 — 渲染 Agent 列表
  /// 验证：所有 Agent 的名称和状态标签正确显示
  it("应正确渲染 Agent 列表", () => {
    setupMockStore({
      agents: [
        createMockAgent({ id: "1", name: "Echo Agent", role: "echo", status: "idle" }),
        createMockAgent({ id: "2", name: "Tool Agent", role: "tool", status: "busy" }),
      ],
    });
    render(<AgentPanel />);

    expect(screen.getByText("Echo Agent")).toBeDefined();
    expect(screen.getByText("Tool Agent")).toBeDefined();
    // 状态标签
    expect(screen.getByText("空闲")).toBeDefined();
    expect(screen.getByText("忙碌")).toBeDefined();
  });

  /// 测试 — 空 Agent 列表显示空状态
  /// 验证：无 Agent 时显示"暂无 Agent 连接"
  it("空 Agent 列表应显示空状态提示", () => {
    setupMockStore({ agents: [], agentsLoading: false });
    render(<AgentPanel />);

    expect(screen.getByText("暂无 Agent 连接")).toBeDefined();
    expect(screen.getByText("点击刷新按钮重新获取")).toBeDefined();
  });

  /// 测试 — 加载中时显示加载动画
  /// 验证：agentsLoading=true 且列表为空时显示加载动画
  it("加载中且无 Agent 时应显示加载动画", () => {
    setupMockStore({ agents: [], agentsLoading: true });
    render(<AgentPanel />);

    // 刷新按钮文字变为"刷新中..."
    expect(screen.getByText("刷新中...")).toBeDefined();
  });

  /// 测试 — 错误状态显示错误消息
  /// 验证：agentsError 不为空时显示红色错误提示
  it("有错误时应显示错误消息", () => {
    setupMockStore({
      agents: [],
      agentsError: "无法连接后端服务",
    });
    render(<AgentPanel />);

    expect(screen.getByText("⚠️ 无法连接后端服务")).toBeDefined();
  });

  // ============================================================
  // Agent 卡片交互测试
  // ============================================================

  /// 测试 — 点击 Agent 卡片可展开详情
  /// 验证：点击 Agent 头部后显示能力列表和最后活跃时间
  it("点击 Agent 卡片应展开能力详情", async () => {
    const user = userEvent.setup();
    setupMockStore({
      agents: [
        createMockAgent({
          id: "agent-1",
          name: "规划 Agent",
          role: "planner",
          status: "idle",
          currentTask: "正在分析任务需求",
          capabilities: [
            { name: "任务分解", description: "分解任务", available: true },
            { name: "依赖分析", description: "分析依赖", available: false },
          ],
        }),
      ],
    });
    render(<AgentPanel />);

    // 点击 Agent 名称（展开）
    await user.click(screen.getByText("规划 Agent"));

    // 应显示当前任务
    expect(screen.getByText("当前任务")).toBeDefined();
    expect(screen.getByText("正在分析任务需求")).toBeDefined();

    // 应显示能力标签
    expect(screen.getByText("能力")).toBeDefined();
    expect(screen.getByText("任务分解")).toBeDefined();
    expect(screen.getByText("依赖分析")).toBeDefined();

    // 应显示最后活跃时间
    expect(screen.getByText(/最后活跃/)).toBeDefined();
  });

  /// 测试 — 再次点击可折叠详情
  /// 验证：展开后再次点击 Agent 头部应隐藏详情
  it("再次点击已展开的 Agent 卡片应折叠详情", async () => {
    const user = userEvent.setup();
    setupMockStore({
      agents: [
        createMockAgent({
          id: "agent-1",
          name: "代码 Agent",
          role: "coder",
          capabilities: [
            { name: "代码生成", description: "生成代码", available: true },
          ],
        }),
      ],
    });
    render(<AgentPanel />);

    // 第一次点击展开
    await user.click(screen.getByText("代码 Agent"));
    expect(screen.getByText("代码生成")).toBeDefined();

    // 第二次点击折叠
    await user.click(screen.getByText("代码 Agent"));
    // 详情应隐藏
    expect(screen.queryByText("代码生成")).toBeNull();
  });

  // ============================================================
  // 轮询测试
  // ============================================================

  /// 测试 — 组件挂载时启动轮询
  /// 验证：useEffect 中调用 startPolling(5000)
  it("组件挂载时应启动 Agent 状态轮询", () => {
    setupMockStore();
    render(<AgentPanel />);

    expect(mockStartPolling).toHaveBeenCalledWith(5000);
  });

  /// 测试 — 组件卸载时停止轮询
  /// 验证：返回的清理函数在组件卸载时被调用
  it("组件卸载时应停止轮询", () => {
    setupMockStore();
    const { unmount } = render(<AgentPanel />);

    unmount();

    expect(mockStopPolling).toHaveBeenCalled();
  });

  // ============================================================
  // 刷新按钮测试
  // ============================================================

  /// 测试 — 点击刷新按钮调用 fetchAgents
  /// 验证：手动点击刷新按钮触发数据获取
  it("点击刷新按钮应调用 fetchAgents", async () => {
    const user = userEvent.setup();
    setupMockStore();
    render(<AgentPanel />);

    const refreshButton = screen.getByText("刷新");
    await user.click(refreshButton);

    expect(mockFetchAgents).toHaveBeenCalled();
  });

  /// 测试 — 加载中时刷新按钮显示刷新中
  /// 验证：agentsLoading=true 时按钮显示"刷新中..."
  it("加载中时刷新按钮应显示加载状态", () => {
    setupMockStore({ agentsLoading: true });
    render(<AgentPanel />);

    const refreshButton = screen.getByTitle("手动刷新");
    expect(refreshButton).toBeDisabled();
  });

  // ============================================================
  // Agent 状态显示测试
  // ============================================================

  /// 测试 — 正确显示各状态 Agent
  /// 验证：idle/busy/error/offline 状态的 Agent 都显示对应标签
  it("应正确显示各状态的 Agent", () => {
    setupMockStore({
      agents: [
        createMockAgent({ id: "1", name: "空闲Agent", status: "idle", online: true }),
        createMockAgent({ id: "2", name: "忙碌Agent", status: "busy", online: true }),
        createMockAgent({ id: "3", name: "异常Agent", status: "error", online: false }),
        createMockAgent({ id: "4", name: "离线Agent", status: "offline", online: false }),
      ],
    });
    render(<AgentPanel />);

    expect(screen.getByText("空闲Agent")).toBeDefined();
    expect(screen.getByText("忙碌Agent")).toBeDefined();
    expect(screen.getByText("异常Agent")).toBeDefined();
    expect(screen.getByText("离线Agent")).toBeDefined();

    // 状态标签
    expect(screen.getByText("空闲")).toBeDefined();
    expect(screen.getByText("忙碌")).toBeDefined();
    expect(screen.getByText("异常")).toBeDefined();
    expect(screen.getByText("离线")).toBeDefined();
  });

  /// 测试 — 没有 currentTask 的 Agent 不显示当前任务
  /// 验证：Agent currentTask 为 null 时展开卡片不显示"当前任务"区块
  it("当前任务为 null 时不显示任务区块", async () => {
    const user = userEvent.setup();
    setupMockStore({
      agents: [createMockAgent({ currentTask: null })],
    });
    render(<AgentPanel />);

    await user.click(screen.getByText("测试 Agent"));

    // "当前任务"标签不应该出现
    expect(screen.queryByText("当前任务")).toBeNull();
  });
});
