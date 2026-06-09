/**
 * 主应用组件
 * 侧边栏 + 页面路由（简易状态切换）
 */

import { useEffect } from "react";
import { useAgentStore } from "@/store/useAgentStore";
import ChatWindow from "@/components/ChatWindow";
import AgentPanel from "@/components/AgentPanel";
import SettingsPanel from "@/components/SettingsPanel";
import TaskBoard from "@/components/TaskBoard";
import ProjectPanel from "@/components/ProjectPanel";
import ApprovalPanel from "@/components/ApprovalPanel";
import type { PageRoute } from "@/types";

// ============ 侧边栏导航配置 ============

interface NavItem {
  route: PageRoute;
  label: string;
  icon: string;
}

const NAV_ITEMS: NavItem[] = [
  { route: "chat", label: "对话", icon: "💬" },
  { route: "tasks", label: "任务", icon: "📋" },
  { route: "project", label: "项目", icon: "🗂️" },
  { route: "approvals", label: "审批", icon: "✅" },
  { route: "agents", label: "Agent", icon: "🤖" },
  { route: "settings", label: "设置", icon: "⚙️" },
];

// ============ 侧边栏组件 ============

function Sidebar() {
  const currentPage = useAgentStore((s) => s.currentPage);
  const setCurrentPage = useAgentStore((s) => s.setCurrentPage);
  const healthy = useAgentStore((s) => s.healthy);
  const healthVersion = useAgentStore((s) => s.healthVersion);
  const checkHealth = useAgentStore((s) => s.checkHealth);

  // 启动时检查健康状态
  useEffect(() => {
    checkHealth();
    // 每 30 秒检查一次
    const timer = setInterval(checkHealth, 30000);
    return () => clearInterval(timer);
  }, [checkHealth]);

  return (
    <aside className="w-64 h-full bg-sidebar flex flex-col border-r border-zinc-800/50 select-none">
      {/* Logo 区域 */}
      <div className="px-5 py-5 border-b border-zinc-800/50">
        <div className="flex items-center gap-3">
          <span className="text-2xl">🧠</span>
          <div>
            <h1 className="text-base font-bold text-zinc-100 leading-tight">
              多 Agent 协同
            </h1>
            <p className="text-[10px] text-zinc-500">Multi-Agent Co-op</p>
          </div>
        </div>
      </div>

      {/* 导航菜单 */}
      <nav className="flex-1 px-3 py-4 space-y-1">
        {NAV_ITEMS.map((item) => (
          <div
            key={item.route}
            onClick={() => setCurrentPage(item.route)}
            className={`nav-item ${currentPage === item.route ? "active" : ""}`}
          >
            <span className="text-lg">{item.icon}</span>
            <span>{item.label}</span>
          </div>
        ))}
      </nav>

      {/* 底部状态栏 */}
      <div className="px-4 py-3 border-t border-zinc-800/50">
        <div className="flex items-center gap-2 text-xs">
          <span
            className={`status-dot ${
              healthy === true
                ? "online"
                : healthy === false
                  ? "error"
                  : "offline"
            }`}
          />
          <span className="text-zinc-500">
            {healthy === true
              ? "后端已连接"
              : healthy === false
                ? "后端离线"
                : "检测中..."}
          </span>
        </div>
        {healthVersion && (
          <p className="text-[10px] text-zinc-600 mt-1">
            版本: {healthVersion}
          </p>
        )}
      </div>
    </aside>
  );
}

// ============ 页面内容映射 ============

function PageContent() {
  const currentPage = useAgentStore((s) => s.currentPage);

  switch (currentPage) {
    case "chat":
      return <ChatWindow />;
    case "tasks":
      return <TaskBoard />;
    case "project":
      return <ProjectPanel />;
    case "approvals":
      return <ApprovalPanel />;
    case "agents":
      return <AgentPanel />;
    case "settings":
      return <SettingsPanel />;
    default:
      return <ChatWindow />;
  }
}

// ============ 主应用 ============

export default function App() {
  return (
    <div className="flex h-full w-full overflow-hidden">
      {/* 侧边栏 */}
      <Sidebar />

      {/* 主内容区 */}
      <main className="flex-1 flex flex-col overflow-hidden bg-[#0f0f1a]">
        <PageContent />
      </main>
    </div>
  );
}
