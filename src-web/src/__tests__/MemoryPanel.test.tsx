/**
 * MemoryPanel 组件测试
 *
 * 测试长期知识的检索、搜索和写入流程。
 */

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import MemoryPanel from "@/components/MemoryPanel";
import { api } from "@/lib/tauri";
import type { KnowledgeItem, SearchKnowledgeRequest } from "@/types";

vi.mock("@/lib/tauri", () => ({
  api: {
    searchKnowledge: vi.fn(),
    storeKnowledge: vi.fn(),
  },
}));

const mockApi = api as unknown as {
  searchKnowledge: ReturnType<typeof vi.fn>;
  storeKnowledge: ReturnType<typeof vi.fn>;
};

function createKnowledgeItem(overrides: Partial<KnowledgeItem> = {}): KnowledgeItem {
  return {
    id: "knowledge-1",
    title: "构建失败经验",
    content: "验证失败后优先查看命令审计中的 stderr。",
    source: "task-1",
    tags: ["FailureCase", "verification"],
    createdAt: "2026-06-09T08:00:00.000Z",
    ...overrides,
  };
}

describe("MemoryPanel", () => {
  beforeEach(() => {
    mockApi.searchKnowledge.mockResolvedValue({
      query: "FailureCase",
      items: [createKnowledgeItem()],
    });
    mockApi.storeKnowledge.mockResolvedValue({ id: "knowledge-new" });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("挂载时应检索默认 FailureCase 记忆并渲染结果", async () => {
    render(<MemoryPanel />);

    expect(mockApi.searchKnowledge).toHaveBeenCalledWith({
      query: "FailureCase",
      limit: 50,
    });
    expect(await screen.findByText("构建失败经验")).toBeDefined();
    expect(screen.getByText("验证失败后优先查看命令审计中的 stderr。")).toBeDefined();
    expect(screen.getByText("FailureCase / verification")).toBeDefined();
    expect(screen.getByText("来源：task-1")).toBeDefined();
    expect(screen.getByText("1 条")).toBeDefined();
  });

  it("提交搜索表单时应按输入关键词重新检索", async () => {
    const user = userEvent.setup();
    mockApi.searchKnowledge
      .mockResolvedValueOnce({ query: "FailureCase", items: [] })
      .mockResolvedValueOnce({
        query: "ProjectFact",
        items: [createKnowledgeItem({ title: "项目事实", tags: ["ProjectFact"] })],
      });

    render(<MemoryPanel />);
    await waitFor(() => {
      expect(mockApi.searchKnowledge).toHaveBeenCalledWith({
        query: "FailureCase",
        limit: 50,
      });
    });

    const searchInput = screen.getByPlaceholderText("搜索长期记忆");
    await user.clear(searchInput);
    await user.type(searchInput, "ProjectFact");
    await user.click(screen.getByRole("button", { name: "搜索" }));

    await waitFor(() => {
      expect(mockApi.searchKnowledge).toHaveBeenLastCalledWith({
        query: "ProjectFact",
        limit: 50,
      });
    });
    expect(await screen.findByText("项目事实")).toBeDefined();
  });

  it("保存知识后应写入标签并按首个标签刷新列表", async () => {
    const user = userEvent.setup();
    const savedItem = createKnowledgeItem({
      id: "knowledge-new",
      title: "自动回滚经验",
      content: "补丁验证失败时应优先生成回滚记录。",
      source: "manual-note",
      tags: ["ProjectFact", "rollback"],
    });
    mockApi.searchKnowledge.mockImplementation(async (request: SearchKnowledgeRequest) => ({
      query: request.query,
      items: request.query === "ProjectFact" ? [savedItem] : [],
    }));
    mockApi.storeKnowledge.mockResolvedValue({ id: "knowledge-new" });

    render(<MemoryPanel />);
    await waitFor(() => {
      expect(mockApi.searchKnowledge).toHaveBeenCalledWith({
        query: "FailureCase",
        limit: 50,
      });
    });

    await user.type(screen.getByPlaceholderText("ProjectFact 或 FailureCase"), "自动回滚经验");
    await user.type(
      screen.getByPlaceholderText("记录项目事实、失败原因、修复经验或成功模式"),
      "补丁验证失败时应优先生成回滚记录。"
    );
    await user.type(screen.getByPlaceholderText("taskId、patchId 或人工记录"), "manual-note");
    const tagsInput = screen.getByPlaceholderText("ProjectFact, FailureCase");
    await user.clear(tagsInput);
    await user.type(tagsInput, "ProjectFact, rollback");
    await user.click(screen.getByRole("button", { name: "保存记忆" }));

    await waitFor(() => {
      expect(mockApi.storeKnowledge).toHaveBeenCalledWith({
        title: "自动回滚经验",
        content: "补丁验证失败时应优先生成回滚记录。",
        source: "manual-note",
        tags: ["ProjectFact", "rollback"],
      });
    });
    await waitFor(() => {
      expect(mockApi.searchKnowledge).toHaveBeenLastCalledWith({
        query: "ProjectFact",
        limit: 50,
      });
    });
    expect(await screen.findByText("已保存：knowledge-new")).toBeDefined();
    expect(screen.getByText("自动回滚经验")).toBeDefined();
  });
});
