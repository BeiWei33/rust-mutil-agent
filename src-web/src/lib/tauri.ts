/**
 * Tauri IPC 封装工具函数
 * 提供与 Rust 后端通信的统一接口
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  SendMessageRequest,
  SendMessageResponse,
  AgentStatus,
  AgentListResponse,
  HealthCheckResponse,
  Message,
} from "@/types";

/**
 * Tauri IPC 命令前缀。
 *
 * 当前 Rust 后端注册的是普通 `#[tauri::command]`，不是 `plugin:agent|...` 插件命令。
 */
const CMD_PREFIX = "";

/**
 * 发送聊天消息到指定 Agent
 * @param request 消息请求
 * @returns Agent 响应
 */
export async function sendMessage(
  request: SendMessageRequest
): Promise<SendMessageResponse> {
  return invoke<SendMessageResponse>(`${CMD_PREFIX}send_message`, {
    request,
  });
}

/**
 * 获取单个 Agent 的状态
 * @param agentId Agent ID
 * @returns Agent 状态
 */
export async function getAgentStatus(
  agentId: string
): Promise<AgentStatus> {
  return invoke<AgentStatus>(`${CMD_PREFIX}get_agent_status`, {
    agentId,
  });
}

/**
 * 获取所有 Agent 列表及状态
 * @returns Agent 列表
 */
export async function listAgents(): Promise<AgentListResponse> {
  return invoke<AgentListResponse>(`${CMD_PREFIX}list_agents`);
}

/**
 * 后端健康检查
 * @returns 健康检查结果
 */
export async function healthCheck(): Promise<HealthCheckResponse> {
  return invoke<HealthCheckResponse>(`${CMD_PREFIX}health_check`);
}

/**
 * 获取对话历史
 * @param sessionId 会话 ID
 * @returns 消息列表
 */
export async function getHistory(sessionId: string): Promise<Message[]> {
  return invoke<Message[]>(`${CMD_PREFIX}get_history`, {
    sessionId,
  });
}

/**
 * 清空对话历史
 * @param sessionId 会话 ID
 */
export async function clearHistory(sessionId: string): Promise<void> {
  return invoke<void>(`${CMD_PREFIX}clear_history`, {
    sessionId,
  });
}

// ============ 浏览器环境降级（非 Tauri 环境下的模拟接口） ============

/** 检测是否运行在 Tauri 环境中 */
let _isTauri: boolean | null = null;

export function isTauri(): boolean {
  if (_isTauri !== null) return _isTauri;
  try {
    _isTauri = !!(window as any).__TAURI_INTERNALS__;
  } catch {
    _isTauri = false;
  }
  return _isTauri;
}

/** 生成唯一 ID */
function generateId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

/** 模拟 Agent 数据 */
const MOCK_AGENTS: AgentStatus[] = [
  {
    id: "coordinator",
    runtimeName: "Planner",
    name: "协调员/总控",
    role: "planner",
    roleLabel: "任务规划与调度",
    description: "理解你的需求，拆解任务，并安排合适的 AI 成员协同处理。",
    online: true,
    status: "idle",
    statusLabel: "空闲",
    currentTask: null,
    selectable: true,
    recommended: true,
    isInternal: false,
    capabilities: [
      { name: "任务拆解", description: "将复杂任务拆分为可执行步骤", available: true },
      { name: "成员调度", description: "根据任务类型安排合适的 AI 成员", available: true },
    ],
    lastActive: new Date().toISOString(),
  },
  {
    id: "executor",
    runtimeName: "Executor",
    name: "执行工程师",
    role: "executor",
    roleLabel: "任务执行",
    description: "负责执行明确任务，例如代码处理、命令运行、文件操作和问题修复。",
    online: true,
    status: "busy",
    statusLabel: "忙碌",
    currentTask: "正在处理示例任务",
    selectable: true,
    recommended: false,
    isInternal: false,
    capabilities: [
      { name: "代码/命令执行", description: "执行代码、脚本和命令", available: true },
      { name: "工具调用", description: "调用工具完成具体操作", available: true },
    ],
    lastActive: new Date().toISOString(),
  },
  {
    id: "memory",
    runtimeName: "Memory",
    name: "记忆管理员",
    role: "memory",
    roleLabel: "记忆与检索",
    description: "负责保存、查找和整理历史上下文，通常由协调员自动调用。",
    online: true,
    status: "idle",
    statusLabel: "空闲",
    currentTask: null,
    selectable: false,
    recommended: false,
    isInternal: true,
    capabilities: [
      { name: "上下文记忆", description: "管理对话历史和长期记忆", available: true },
      { name: "信息检索", description: "从知识库或历史记录中检索信息", available: true },
    ],
    lastActive: new Date().toISOString(),
  },
];

/** 浏览器环境下降级的 sendMessage */
async function mockSendMessage(
  request: SendMessageRequest
): Promise<SendMessageResponse> {
  // 模拟网络延迟
  await new Promise((r) => setTimeout(r, 500 + Math.random() * 1000));

  const mockReplies: Record<string, string> = {
    hello: "你好！我是多 Agent 协同系统，有什么可以帮助你的？",
    help: "我可以帮你：\n1. **代码编写** - 生成高质量代码\n2. **任务规划** - 分解复杂任务\n3. **数据分析** - 分析数据并生成报告\n\n请输入你的需求！",
    code: "```typescript\n// 这是一个示例代码\nfunction greet(name: string): string {\n  return `你好，${name}！`;\n}\n```",
    default: `收到你的消息：「${request.content}」\n\n这是一个模拟回复。在 Tauri 环境中，这里会显示 Agent 的真实响应。\n\n当前可用的 Agent 包括：\n- 🧠 规划 Agent\n- 💻 代码 Agent\n- 📊 分析 Agent`,
  };

  const lower = request.content.toLowerCase();
  let replyContent = mockReplies.default;
  for (const [key, val] of Object.entries(mockReplies)) {
    if (lower.includes(key)) {
      replyContent = val;
      break;
    }
  }

  const agents = MOCK_AGENTS.filter((a) => a.online);
  const picked = agents[Math.floor(Math.random() * agents.length)];

  return {
    message: {
      id: generateId(),
      role: "assistant",
      content: replyContent,
      timestamp: new Date().toISOString(),
      senderName: picked.name,
    },
    handledBy: picked.id,
  };
}

/** 浏览器环境下降级的 listAgents */
async function mockListAgents(): Promise<AgentListResponse> {
  await new Promise((r) => setTimeout(r, 200));
  // 随机刷新状态以模拟轮询
  const agents = MOCK_AGENTS.map((a) => ({
    ...a,
    lastActive: new Date().toISOString(),
    status: a.status === "busy" && Math.random() > 0.7 ? "idle" as const : a.status,
  }));
  return { agents };
}

/** 浏览器环境下降级的 healthCheck */
async function mockHealthCheck(): Promise<HealthCheckResponse> {
  return {
    healthy: true,
    version: "0.1.0-dev",
    agentCount: MOCK_AGENTS.filter((a) => a.online).length,
  };
}

/** 浏览器环境下降级的 getHistory */
async function mockGetHistory(_sessionId: string): Promise<Message[]> {
  return [
    {
      id: "welcome-1",
      role: "assistant",
      content: "欢迎使用多 Agent 协同智能体！我可以协调多个 AI Agent 来完成复杂的任务。",
      timestamp: new Date(Date.now() - 60000).toISOString(),
      senderName: "系统",
    },
  ];
}

/**
 * 自动降级：Tauri 环境使用 invoke，浏览器环境使用模拟数据
 */
export const api = {
  sendMessage: isTauri() ? sendMessage : mockSendMessage,
  getAgentStatus: isTauri()
    ? getAgentStatus
    : async (id: string) => {
        const agent = MOCK_AGENTS.find((a) => a.id === id);
        if (!agent) throw new Error(`Agent ${id} not found`);
        return { ...agent, lastActive: new Date().toISOString() };
      },
  listAgents: isTauri() ? listAgents : mockListAgents,
  healthCheck: isTauri() ? healthCheck : mockHealthCheck,
  getHistory: isTauri() ? getHistory : mockGetHistory,
  clearHistory: isTauri()
    ? clearHistory
    : async () => { /* no-op in browser */ },
};
