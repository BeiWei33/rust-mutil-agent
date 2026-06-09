import type { AgentStatus, Capability } from "@/types";

const FALLBACK_AGENT_NAMES: Record<string, string> = {
  Echo: "回声测试员",
  Planner: "协调员/总控",
  Executor: "执行工程师",
  Review: "代码评审员",
  Evolution: "演进顾问",
  Memory: "记忆管理员",
  Tool: "工具操作员",
};

const FALLBACK_ROLE_LABELS: Record<string, string> = {
  echo: "连接测试",
  planner: "任务规划与调度",
  executor: "任务执行",
  review: "风险审查",
  evolution: "经验沉淀",
  memory: "记忆与检索",
  tool: "工具调用",
};

const FALLBACK_CAPABILITY_NAMES: Record<string, string> = {
  chat: "自然语言对话",
  planning: "任务拆解",
  code_execution: "代码/命令执行",
  retrieval: "信息检索",
  tool_use: "工具调用",
  memory: "上下文记忆",
  review: "风险审查",
  evolution: "经验沉淀",
};

export function formatAgentName(agent: AgentStatus): string {
  return agent.name || FALLBACK_AGENT_NAMES[agent.runtimeName ?? ""] || FALLBACK_AGENT_NAMES[agent.name] || agent.id;
}

export function formatAgentRole(agent: AgentStatus): string {
  return agent.roleLabel || FALLBACK_ROLE_LABELS[agent.role] || agent.role || "自定义能力";
}

export function formatAgentDescription(agent: AgentStatus): string {
  return agent.description || `${formatAgentRole(agent)} AI 成员`;
}

export function formatCapabilityName(capability: Capability): string {
  return FALLBACK_CAPABILITY_NAMES[capability.name] || capability.name;
}

export function formatCapabilityDescription(capability: Capability): string {
  return capability.description || "暂无能力说明";
}

export function isAgentSelectable(agent: AgentStatus): boolean {
  return agent.online && agent.selectable !== false;
}

export function findAgentById(agents: AgentStatus[], agentId: string): AgentStatus | undefined {
  return agents.find((agent) => agent.id === agentId || agent.runtimeName === agentId || agent.name === agentId);
}
