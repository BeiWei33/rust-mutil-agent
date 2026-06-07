/**
 * Agent 状态监控面板
 * 展示各 Agent 名称、状态、能力，支持手动刷新 + 自动轮询
 */

import { useEffect, useState } from "react";
import { useAgentStore } from "@/store/useAgentStore";
import type { AgentStatus } from "@/types";
import {
  formatAgentDescription,
  formatAgentName,
  formatAgentRole,
  formatCapabilityDescription,
  formatCapabilityName,
} from "@/lib/agentDisplay";

/** Agent 状态对应的颜色和图标 */
const STATUS_MAP: Record<
  AgentStatus["status"],
  { dotClass: string; label: string; icon: string }
> = {
  idle: { dotClass: "status-dot online", label: "空闲", icon: "🟢" },
  busy: { dotClass: "status-dot busy", label: "忙碌", icon: "🟡" },
  error: { dotClass: "status-dot error", label: "异常", icon: "🔴" },
  offline: { dotClass: "status-dot offline", label: "离线", icon: "⚫" },
};

/** 单个 Agent 卡片 */
function AgentCard({ agent }: { agent: AgentStatus }) {
  const [expanded, setExpanded] = useState(false);
  const statusInfo = STATUS_MAP[agent.status];

  return (
    <div className="card hover:border-zinc-700/50 transition-colors">
      {/* 头部信息 */}
      <div
        className="flex items-center gap-3 cursor-pointer"
        onClick={() => setExpanded(!expanded)}
      >
        {/* 状态指示 */}
        <span className={statusInfo.dotClass} title={statusInfo.label} />

        {/* 名称和角色 */}
        <div className="flex-1 min-w-0">
          <h3 className="text-sm font-medium text-zinc-200 truncate">
            {formatAgentName(agent)}
          </h3>
          <p className="text-xs text-zinc-500">{formatAgentRole(agent)}</p>
        </div>

        {/* 状态标签 */}
        <span className="text-xs text-zinc-500 inline-flex items-center gap-1">
          <span aria-hidden="true">{statusInfo.icon}</span>
          <span>{statusInfo.label}</span>
        </span>

        {/* 展开按钮 */}
        <svg
          className={`w-4 h-4 text-zinc-500 transition-transform duration-200 ${
            expanded ? "rotate-180" : ""
          }`}
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={2}
        >
          <path strokeLinecap="round" strokeLinejoin="round" d="M19 9l-7 7-7-7" />
        </svg>
      </div>

      {/* 展开详情 */}
      {expanded && (
        <div className="mt-3 pt-3 border-t border-zinc-800 animate-slide-up">
          <p className="text-xs text-zinc-400 leading-relaxed mb-3">
            {formatAgentDescription(agent)}
          </p>

          {/* 当前任务 */}
          {agent.currentTask && (
            <div className="mb-3">
              <span className="text-xs text-zinc-500">当前任务</span>
              <p className="text-xs text-zinc-300 mt-1 bg-zinc-800/50 rounded-lg p-2">
                {agent.currentTask}
              </p>
            </div>
          )}

          {/* 能力列表 */}
          <div>
            <span className="text-xs text-zinc-500">能力</span>
            <span className="text-xs text-zinc-500 ml-1">/ 擅长能力</span>
            <div className="grid gap-2 mt-2">
              {agent.capabilities.map((cap) => (
                <div
                  key={cap.name}
                  className={`rounded-lg border px-3 py-2 text-xs ${
                    cap.available
                      ? "bg-primary-500/10 text-primary-200 border-primary-500/20"
                      : "bg-zinc-800/50 text-zinc-500 border-zinc-700/30"
                  }`}
                >
                  <div className="font-medium inline-flex items-center gap-1">
                    <span aria-hidden="true">{cap.available ? "✅" : "暂不可用"}</span>
                    <span>{formatCapabilityName(cap)}</span>
                  </div>
                  <div className="mt-1 text-[11px] text-zinc-500 leading-relaxed">
                    {formatCapabilityDescription(cap)}
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* 最后活跃时间 */}
          <p className="text-[10px] text-zinc-600 mt-3">
            最后活跃: {new Date(agent.lastActive).toLocaleTimeString("zh-CN")}
          </p>
        </div>
      )}
    </div>
  );
}

/** Agent 面板主组件 */
export default function AgentPanel() {
  const agents = useAgentStore((s) => s.agents);
  const loading = useAgentStore((s) => s.agentsLoading);
  const error = useAgentStore((s) => s.agentsError);
  const fetchAgents = useAgentStore((s) => s.fetchAgents);
  const startPolling = useAgentStore((s) => s.startPolling);

  // 自动轮询（每 5 秒）
  useEffect(() => {
    const stop = startPolling(5000);
    return stop;
  }, [startPolling]);

  // 统计信息
  const onlineCount = agents.filter((a) => a.online).length;
  const busyCount = agents.filter((a) => a.status === "busy").length;

  return (
    <div className="flex flex-col h-full">
      {/* 顶部工具栏 */}
      <div className="flex items-center justify-between px-6 py-3 border-b border-zinc-800/50">
        <h2 className="text-lg font-semibold text-zinc-200">🤖 AI 团队成员</h2>
        <button
          onClick={fetchAgents}
          disabled={loading}
          className="btn-ghost text-xs px-3 py-1.5"
          title="手动刷新"
          aria-label="刷新"
        >
          <span aria-hidden="true">🔄 </span><span>{loading ? "刷新中..." : "刷新"}</span>
        </button>
      </div>

      {/* 统计概览 */}
      <div className="grid grid-cols-3 gap-3 px-6 py-4 border-b border-zinc-800/50">
        <div className="bg-zinc-900/50 rounded-lg p-3 text-center">
          <p className="text-2xl font-bold text-zinc-200">{agents.length}</p>
          <p className="text-xs text-zinc-500 mt-0.5">成员总数</p>
        </div>
        <div className="bg-zinc-900/50 rounded-lg p-3 text-center">
          <p className="text-2xl font-bold text-green-400">{onlineCount}</p>
          <p className="text-xs text-zinc-500 mt-0.5">可工作</p>
        </div>
        <div className="bg-zinc-900/50 rounded-lg p-3 text-center">
          <p className="text-2xl font-bold text-yellow-400">{busyCount}</p>
          <p className="text-xs text-zinc-500 mt-0.5">正在处理</p>
        </div>
      </div>

      {/* Agent 列表 */}
      <div className="flex-1 overflow-y-auto px-6 py-4 space-y-3">
        {error && (
          <div className="bg-red-500/10 border border-red-500/20 rounded-lg p-3 text-sm text-red-400">
            ⚠️ {error}
          </div>
        )}

        {!loading && agents.length === 0 && !error && (
          <div className="flex flex-col items-center justify-center h-full text-zinc-500">
            <span className="text-4xl mb-3">📡</span>
            <p className="text-sm">暂无 Agent 连接</p>
            <p className="text-xs mt-1 text-zinc-600">点击刷新按钮重新获取</p>
          </div>
        )}

        {agents.map((agent) => (
          <AgentCard key={agent.id} agent={agent} />
        ))}

        {loading && agents.length === 0 && (
          <div className="flex items-center justify-center py-12">
            <div className="flex gap-2">
              {[0, 1, 2].map((i) => (
                <span
                  key={i}
                  className="w-2.5 h-2.5 bg-primary-400 rounded-full animate-bounce"
                  style={{ animationDelay: `${i * 150}ms` }}
                />
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
