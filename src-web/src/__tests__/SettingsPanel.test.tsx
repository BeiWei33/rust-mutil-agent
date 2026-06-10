/**
 * SettingsPanel 组件测试
 * 
 * 测试设置面板的 localStorage 读写、表单交互和设置持久化
 */

import { describe, it, expect, beforeEach, vi, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useAgentStore } from "@/store/useAgentStore";
import SettingsPanel from "@/components/SettingsPanel";
import type { AppSettings } from "@/types";

// 模拟 useAgentStore
vi.mock("@/store/useAgentStore", () => ({
  useAgentStore: vi.fn(),
}));

const mockUseAgentStore = useAgentStore as unknown as ReturnType<typeof vi.fn>;

let storeState: Record<string, any>;
let mockUpdateSettings: ReturnType<typeof vi.fn>;

/**
 * 默认设置值
 */
const DEFAULT_SETTINGS: AppSettings = {
  model: "deepseek-v4-pro",
  apiKey: "",
  apiBaseUrl: "https://api.deepseek.com/v1",
  maxTokens: 4096,
  temperature: 0.7,
  reasoningEffort: "",
};

/**
 * 设置模拟的 store 状态
 */
function setupMockStore(overrides: Record<string, any> = {}) {
  mockUpdateSettings = vi.fn();

  storeState = {
    settings: { ...DEFAULT_SETTINGS },
    updateSettings: mockUpdateSettings,
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

describe("SettingsPanel", () => {
  beforeEach(() => {
    // 清除 localStorage
    localStorage.clear();
    // 模拟 localStorage
    vi.stubGlobal("localStorage", {
      getItem: vi.fn(),
      setItem: vi.fn(),
      removeItem: vi.fn(),
      clear: vi.fn(),
      length: 0,
      key: vi.fn(),
    });
    setupMockStore();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  /// 测试 — 组件正确渲染标题
  /// 验证：设置面板的标题和各个区块正确显示
  it("应正确渲染设置面板标题和各设置区块", () => {
    render(<SettingsPanel />);

    expect(screen.getByText("⚙️ 设置")).toBeDefined();
    expect(screen.getByText("模型选择")).toBeDefined();
    expect(screen.getByText("API Key")).toBeDefined();
    expect(screen.getByText("API 基础 URL")).toBeDefined();
    expect(screen.getByText("高级参数")).toBeDefined();
  });

  /// 测试 — 默认模型选择显示正确
  /// 验证：模型下拉框默认值为 deepseek-v4-pro
  it("应显示默认选择的模型", () => {
    setupMockStore({ settings: { ...DEFAULT_SETTINGS, model: "deepseek-v4-pro" } });
    render(<SettingsPanel />);

    const select = screen.getByLabelText("模型选择") as HTMLSelectElement;
    expect(select.value).toBe("deepseek-v4-pro");
  });

  // ============================================================
  // localStorage 读写测试
  // ============================================================

  /// 测试 — updateSettings 将设置保存到 localStorage
  /// 验证：更新设置时调用 updateSettings 将新值保存到 localStorage
  it("更新模型选择应保存到 localStorage", async () => {
    const user = userEvent.setup();
    setupMockStore();
    render(<SettingsPanel />);

    // 选择新模型
    const select = screen.getByLabelText("模型选择");
    await user.selectOptions(select, "gpt-4-turbo");

    // 验证 updateSettings 被调用
    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith({ model: "gpt-4-turbo" });
    });
  });

  /// 测试 — 从 localStorage 读取设置
  /// 验证：组件初始化时从 store 读取已保存的设置
  it("应从 store 读取已保存的设置", () => {
    setupMockStore({
      settings: {
        model: "claude-3.5-sonnet",
        apiKey: "sk-saved-key",
        apiBaseUrl: "https://custom.api.com",
        maxTokens: 8192,
        temperature: 0.3,
        reasoningEffort: "high",
      },
    });
    render(<SettingsPanel />);

    // 模型下拉框显示已保存的值
    const select = screen.getByLabelText("模型选择") as HTMLSelectElement;
    expect(select.value).toBe("claude-3.5-sonnet");

    // API Base URL 显示已保存的值
    const urlInput = screen.getByPlaceholderText("https://api.deepseek.com/v1") as HTMLInputElement;
    expect(urlInput.value).toBe("https://custom.api.com");
  });

  // ============================================================
  // API Key 输入测试
  // ============================================================

  /// 测试 — API Key 输入框默认隐藏
  /// 验证：API Key 输入框类型默认为 password
  it("API Key 输入框默认应为密码类型", () => {
    render(<SettingsPanel />);

    const apiKeyInput = screen.getByPlaceholderText("sk-... / 留空使用 DEEPSEEK_API_KEY") as HTMLInputElement;
    expect(apiKeyInput.type).toBe("password");
  });

  /// 测试 — 切换 API Key 可见性
  /// 验证：点击眼睛图标切换 input type 在 password 和 text 之间
  it("点击眼睛图标应切换 API Key 显示/隐藏", async () => {
    const user = userEvent.setup();
    render(<SettingsPanel />);

    // 初始状态为密码
    const apiKeyInput = screen.getByPlaceholderText("sk-... / 留空使用 DEEPSEEK_API_KEY") as HTMLInputElement;
    expect(apiKeyInput.type).toBe("password");

    // 点击显示按钮（标题为"显示"）
    const showButton = screen.getByTitle("显示");
    await user.click(showButton);

    // 现在应为文本类型
    expect(apiKeyInput.type).toBe("text");
  });

  /// 测试 — API Key 输入后显示状态指示
  /// 验证：输入 API Key 后状态指示器从离线变为在线
  it("输入 API Key 后应显示'已设置'状态", () => {
    setupMockStore({
      settings: { ...DEFAULT_SETTINGS, apiKey: "sk-test123" },
    });
    render(<SettingsPanel />);

    expect(screen.getByText(/已设置 \(sk-test/)).toBeDefined();
  });

  /// 测试 — 空 API Key 显示未设置
  /// 验证：API Key 为空时显示"未设置"
  it("空 API Key 应显示'未设置'", () => {
    setupMockStore({ settings: { ...DEFAULT_SETTINGS, apiKey: "" } });
    render(<SettingsPanel />);

    expect(screen.getByText("未设置")).toBeDefined();
  });

  /// 测试 — API Key 失焦时保存
  /// 验证：在 API Key 输入框失焦时调用 updateSettings
  it("API Key 输入框失焦时应保存", async () => {
    const user = userEvent.setup();
    setupMockStore();
    render(<SettingsPanel />);

    const apiKeyInput = screen.getByPlaceholderText("sk-... / 留空使用 DEEPSEEK_API_KEY");
    await user.type(apiKeyInput, "sk-new-key");
    await user.tab(); // 失焦

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ apiKey: "sk-new-key" })
      );
    });
  });

  // ============================================================
  // API Base URL 测试
  // ============================================================

  /// 测试 — API Base URL 失焦时保存
  /// 验证：修改 API Base URL 失焦后调用 updateSettings
  it("API Base URL 输入框失焦时应保存", async () => {
    const user = userEvent.setup();
    setupMockStore();
    render(<SettingsPanel />);

    const urlInput = screen.getByPlaceholderText("https://api.deepseek.com/v1");
    await user.clear(urlInput);
    await user.type(urlInput, "https://my-proxy.example.com/v1");
    await user.tab();

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ apiBaseUrl: "https://my-proxy.example.com/v1" })
      );
    });
  });

  // ============================================================
  // 高级参数测试
  // ============================================================

  /// 测试 — Max Tokens 滑块调整正确
  /// 验证：拖动 Max Tokens 滑块更新值并保存
  it("调整 Max Tokens 滑块应在释放时保存", () => {
    setupMockStore({
      settings: { ...DEFAULT_SETTINGS, maxTokens: 4096 },
    });
    render(<SettingsPanel />);

    // 找到 maxTokens 滑块
    const sliders = screen.getAllByRole("slider");
    const maxTokensSlider = sliders[0]; // 第一个滑块

    // 修改值
    fireEvent.change(maxTokensSlider, { target: { value: "8192" } });
    // 释放鼠标
    fireEvent.mouseUp(maxTokensSlider);

    expect(mockUpdateSettings).toHaveBeenCalledWith(
      expect.objectContaining({ maxTokens: 8192 })
    );
  });

  /// 测试 — Temperature 滑块调整正确
  /// 验证：拖动 Temperature 滑块更新值并保存
  it("调整 Temperature 滑块应在释放时保存", () => {
    setupMockStore({
      settings: { ...DEFAULT_SETTINGS, temperature: 0.7 },
    });
    render(<SettingsPanel />);

    const sliders = screen.getAllByRole("slider");
    const tempSlider = sliders[1]; // 第二个滑块

    // 修改值
    fireEvent.change(tempSlider, { target: { value: "1.5" } });
    fireEvent.mouseUp(tempSlider);

    expect(mockUpdateSettings).toHaveBeenCalledWith(
      expect.objectContaining({ temperature: 1.5 })
    );
  });

  /// 测试 — 参数滑块值的显示标签
  /// 验证：滑块旁的标签显示当前值
  it("应正确显示参数滑块的当前值标签", () => {
    setupMockStore({
      settings: { ...DEFAULT_SETTINGS, maxTokens: 8192, temperature: 1.2, reasoningEffort: "xhigh" },
    });
    render(<SettingsPanel />);

    expect(screen.getByText("8192")).toBeDefined();
    expect(screen.getByText("1.2")).toBeDefined();
    expect(screen.getByText("xhigh")).toBeDefined();
  });

  it("调整推理强度应立即保存", async () => {
    const user = userEvent.setup();
    setupMockStore();
    render(<SettingsPanel />);

    const select = screen.getByLabelText("推理强度");
    await user.selectOptions(select, "xhigh");

    await waitFor(() => {
      expect(mockUpdateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ reasoningEffort: "xhigh" })
      );
    });
  });

  // ============================================================
  // 保存状态提示测试
  // ============================================================

  /// 测试 — 保存后显示"已保存"提示
  /// 验证：更新设置后短暂显示"✅ 已保存"
  it("保存设置后应短暂显示'已保存'提示", async () => {
    const user = userEvent.setup();
    setupMockStore();
    render(<SettingsPanel />);

    const select = screen.getByLabelText("模型选择");
    await user.selectOptions(select, "deepseek-v3");

    // 一瞬间应显示"已保存"
    await waitFor(() => {
      expect(screen.getByText("✅ 已保存")).toBeDefined();
    });
  });

  // ============================================================
  // 设置各区块的渲染测试
  // ============================================================

  /// 测试 — 模型选择下拉框包含所有选项
  /// 验证：下拉框中包含 GPT、Claude、DeepSeek 等模型
  it("模型选择下拉框应包含所有选项", () => {
    render(<SettingsPanel />);

    const select = screen.getByLabelText("模型选择") as HTMLSelectElement;
    const options = Array.from(select.options).map((o) => o.value);

    expect(options).toContain("deepseek-v4-pro");
    expect(options).toContain("gpt5.5");
    expect(options).toContain("gpt-4o");
    expect(options).toContain("gpt-4o-mini");
    expect(options).toContain("gpt-4-turbo");
    expect(options).toContain("claude-3.5-sonnet");
    expect(options).toContain("claude-3-opus");
    expect(options).toContain("deepseek-v3");
    expect(options).toContain("deepseek-r1");
    expect(options).toContain("qwen-max");
    expect(options).toContain("glm-4");
  });

  /// 测试 — 隐私提示文案
  /// 验证：底部显示隐私提示文案
  it("应显示本地存储隐私提示", () => {
    render(<SettingsPanel />);

    expect(
      screen.getByText("所有设置仅存储在浏览器本地，不会上传到服务器")
    ).toBeDefined();
  });

  /// 测试 — 设置同步 useEffect
  /// 验证：外部 settings 变化时表单状态同步更新
  it("外部 settings 变化时表单应同步更新", async () => {
    const { rerender } = render(<SettingsPanel />);

    // 初始模型
    const select = screen.getByLabelText("模型选择") as HTMLSelectElement;
    expect(select.value).toBe("deepseek-v4-pro");

    // 更新 store 中的 settings
    setupMockStore({
      settings: { ...DEFAULT_SETTINGS, model: "claude-3-opus" },
    });
    rerender(<SettingsPanel />);

    await waitFor(() => {
      const updatedSelect = screen.getByLabelText("模型选择") as HTMLSelectElement;
      expect(updatedSelect.value).toBe("claude-3-opus");
    });
  });
});
