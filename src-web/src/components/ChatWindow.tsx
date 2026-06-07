/**
 * 聊天窗口组件
 * 显示消息列表，支持简易 Markdown 渲染，含 Agent 选择器 + 输入框 + 发送按钮
 */

import { useState, useRef, useEffect, useCallback, useMemo } from "react";
import { useAgentStore } from "@/store/useAgentStore";
import type { ChatMessage } from "@/types";
import {
  findAgentById,
  formatAgentDescription,
  formatAgentName,
  formatAgentRole,
  isAgentSelectable,
} from "@/lib/agentDisplay";

// ============ 简易 Markdown 渲染 ============

/**
 * 将 Markdown 文本转换为 HTML（仅支持常用语法）
 */
function renderMarkdown(text: string): string {
  let html = text
    // 转义 HTML 特殊字符
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");

  // 代码块 ```...```
  html = html.replace(
    /```(\w*)\n([\s\S]*?)```/g,
    (_match, lang, code) =>
      `<pre><code class="language-${lang || "plaintext"}">${code}</code></pre>`
  );

  // 行内代码 `...`
  html = html.replace(/`([^`]+)`/g, "<code>$1</code>");

  // 粗体 **...**
  html = html.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");

  // 斜体 *...*
  html = html.replace(/\*([^*]+)\*/g, "<em>$1</em>");

  // 标题 ### ... / ## ... / # ...
  html = html.replace(/^### (.+)$/gm, "<h3>$1</h3>");
  html = html.replace(/^## (.+)$/gm, "<h2>$1</h2>");
  html = html.replace(/^# (.+)$/gm, "<h1>$1</h1>");

  // 无序列表 - item
  html = html.replace(/^- (.+)$/gm, "<li>$1</li>");
  html = html.replace(/((?:<li>.*<\/li>\n?)+)/g, "<ul>$1</ul>");

  // 有序列表 1. item
  html = html.replace(/^\d+\. (.+)$/gm, "<li>$1</li>");

  // 引用 > text
  html = html.replace(/^&gt; (.+)$/gm, "<blockquote>$1</blockquote>");

  // 链接 [text](url)
  html = html.replace(
    /\[([^\]]+)\]\(([^)]+)\)/g,
    '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>'
  );

  // 段落（连续文本）
  html = html.replace(/\n\n/g, "</p><p>");
  html = `<p>${html}</p>`;
  // 清理空白段落
  html = html.replace(/<p>\s*<\/p>/g, "");
  // 清理嵌套 P 标签
  html = html.replace(/<p><(h[123]|ul|ol|pre|blockquote)/g, "<$1");
  html = html.replace(/<\/(h[123]|ul|ol|pre|blockquote)><\/p>/g, "</$1>");

  return html;
}

// ============ 消息气泡 ============

/** 单条消息渲染 */
function MessageBubble({ msg }: { msg: ChatMessage }) {
  const bubbleClass =
    msg.role === "user"
      ? "message-bubble user"
      : msg.role === "system"
        ? "message-bubble system"
        : "message-bubble assistant";

  return (
    <div className={`flex flex-col mb-4 ${msg.role === "user" ? "items-end" : "items-start"}`}>
      {/* 发送者信息 */}
      {msg.senderName && msg.role !== "user" && (
        <span className="text-xs text-zinc-500 mb-1 ml-1">{msg.senderName}</span>
      )}
      <div className={bubbleClass}>
        {msg.role === "user" ? (
          <p className="whitespace-pre-wrap break-words">{msg.content}</p>
        ) : (
          <div
            className="markdown-content"
            dangerouslySetInnerHTML={{ __html: renderMarkdown(msg.content) }}
          />
        )}
      </div>
      {/* 时间戳 */}
      <span className="text-[10px] text-zinc-600 mt-1 mx-1">
        {new Date(msg.timestamp).toLocaleTimeString("zh-CN", {
          hour: "2-digit",
          minute: "2-digit",
        })}
      </span>
    </div>
  );
}

// ============ 聊天窗口主组件 ============

export default function ChatWindow() {
  const messages = useAgentStore((s) => s.messages);
  const sending = useAgentStore((s) => s.sending);
  const sendError = useAgentStore((s) => s.sendError);
  const lastFailedSend = useAgentStore((s) => s.lastFailedSend);
  const retryLastFailedSend = useAgentStore((s) => s.retryLastFailedSend);
  const clearSendError = useAgentStore((s) => s.clearSendError);
  const sendMessage = useAgentStore((s) => s.sendMessage);
  const clearMessages = useAgentStore((s) => s.clearMessages);
  const agents = useAgentStore((s) => s.agents ?? []);
  const fetchAgents = useAgentStore((s) => s.fetchAgents ?? (() => Promise.resolve()));
  const selectedAgentId = useAgentStore((s) => s.selectedAgentId ?? "");
  const setSelectedAgentId = useAgentStore((s) => s.setSelectedAgentId ?? (() => undefined));

  const [input, setInput] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  const selectableAgents = useMemo(
    () => agents.filter((agent) => isAgentSelectable(agent)),
    [agents]
  );
  const selectedAgent = selectedAgentId ? findAgentById(agents, selectedAgentId) : undefined;

  // 首次进入聊天页时获取团队成员，保证选择器有数据。
  useEffect(() => {
    if (agents.length === 0) {
      fetchAgents();
    }
  }, [agents.length, fetchAgents]);

  // 自动滚动到底部
  const scrollToBottom = useCallback(() => {
    const node = messagesEndRef.current;
    if (node && typeof node.scrollIntoView === "function") {
      node.scrollIntoView({ behavior: "smooth" });
    }
  }, []);

  useEffect(() => {
    scrollToBottom();
  }, [messages, scrollToBottom]);

  // 发送消息
  const handleSend = useCallback(async () => {
    const trimmed = input.trim();
    if (!trimmed || sending) return;
    setInput("");
    if (selectedAgentId) {
      await sendMessage(trimmed, selectedAgentId);
    } else {
      await sendMessage(trimmed);
    }
    inputRef.current?.focus();
  }, [input, selectedAgentId, sending, sendMessage]);

  // 键盘提交（Enter 发送，Shift+Enter 换行）
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
    },
    [handleSend]
  );

  const targetHelp = selectedAgent
    ? `当前优先交给「${formatAgentName(selectedAgent)}」：${formatAgentDescription(selectedAgent)}`
    : "当前发送给：自动分配（推荐）。协调员/总控会理解需求并安排合适成员。";

  const sendingLabel = selectedAgent
    ? `${formatAgentName(selectedAgent)} 正在处理你的任务...`
    : "协调员/总控正在分析需求，并选择合适成员...";

  return (
    <div className="flex flex-col h-full">
      {/* 顶部工具栏 */}
      <div className="flex items-center justify-between px-6 py-3 border-b border-zinc-800/50">
        <h2 className="text-lg font-semibold text-zinc-200">💬 工作台</h2>
        <button
          onClick={clearMessages}
          className="btn-ghost text-xs px-3 py-1.5"
          title="清空对话"
        >
          🗑️ 清空
        </button>
      </div>

      {/* 消息列表 */}
      <div className="flex-1 overflow-y-auto px-6 py-4">
        {messages.length === 0 && !sending && (
          <div className="flex flex-col items-center justify-center h-full text-zinc-500">
            <span className="text-6xl mb-4">🤖</span>
            <p className="text-sm">告诉我你想完成什么</p>
            <p className="text-xs mt-2 text-zinc-600 text-center leading-relaxed">
              例如：帮我检查这个项目为什么发送失败<br />
              或者：帮我规划一个新功能，并安排成员执行
            </p>
          </div>
        )}

        {messages.map((msg) => (
          <MessageBubble key={msg.id} msg={msg} />
        ))}

        {/* 发送中指示器 */}
        {sending && (
          <div className="flex items-center gap-2 text-zinc-500 text-sm mb-4 animate-fade-in">
            <div className="flex gap-1">
              <span className="w-2 h-2 bg-primary-400 rounded-full animate-bounce" style={{ animationDelay: "0ms" }} />
              <span className="w-2 h-2 bg-primary-400 rounded-full animate-bounce" style={{ animationDelay: "150ms" }} />
              <span className="w-2 h-2 bg-primary-400 rounded-full animate-bounce" style={{ animationDelay: "300ms" }} />
            </div>
            <span>{sendingLabel}</span>
          </div>
        )}

        <div ref={messagesEndRef} />
      </div>

      {/* 输入区域 */}
      <div className="px-6 py-4 border-t border-zinc-800/50 space-y-3">
        {sendError && (
          <div className="bg-red-500/10 border border-red-500/20 rounded-xl p-3 text-sm text-red-200">
            <div className="font-medium">消息发送失败</div>
            <div className="mt-1 text-red-300/90">{sendError}</div>
            <div className="flex gap-2 mt-3">
              {lastFailedSend && (
                <button className="btn-ghost text-xs px-3 py-1.5" onClick={retryLastFailedSend}>
                  重试
                </button>
              )}
              <button className="btn-ghost text-xs px-3 py-1.5" onClick={() => setSelectedAgentId("")}>改为自动分配</button>
              <button className="btn-ghost text-xs px-3 py-1.5" onClick={clearSendError}>关闭</button>
            </div>
          </div>
        )}

        <div className="bg-zinc-900/40 border border-zinc-800/70 rounded-xl p-3 space-y-2">
          <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
            <label className="text-xs text-zinc-400" htmlFor="target-agent">
              发送给
            </label>
            <select
              id="target-agent"
              aria-label="选择发送目标"
              value={selectedAgentId}
              onChange={(e) => setSelectedAgentId(e.target.value)}
              disabled={sending}
              className="bg-zinc-950 border border-zinc-700 rounded-lg px-3 py-2 text-sm text-zinc-200 outline-none focus:border-primary-500/60"
            >
              <option value="">自动分配（推荐）</option>
              {selectableAgents.map((agent) => (
                <option key={agent.id} value={agent.id}>
                  {formatAgentName(agent)} · {formatAgentRole(agent)}
                </option>
              ))}
            </select>
          </div>
          <p className="text-xs text-zinc-500 leading-relaxed">{targetHelp}</p>
          {selectedAgent && selectedAgent.capabilities.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {selectedAgent.capabilities.slice(0, 4).map((cap) => (
                <span key={cap.name} className="px-2 py-1 rounded-md bg-primary-500/10 text-primary-300 text-[10px] border border-primary-500/20">
                  {cap.name}
                </span>
              ))}
            </div>
          )}
        </div>

        <div className="flex items-end gap-3 bg-zinc-900/50 border border-zinc-700/50 rounded-xl p-2 focus-within:border-primary-500/50 transition-colors">
          <textarea
            ref={inputRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="输入消息... (Enter 发送，Shift+Enter 换行)"
            rows={1}
            className="flex-1 bg-transparent border-none outline-none resize-none
                       text-zinc-200 placeholder:text-zinc-500 text-sm
                       max-h-32 py-2 px-2"
            disabled={sending}
          />
          <button
            onClick={handleSend}
            disabled={!input.trim() || sending}
            className="btn-primary !px-3 !py-2 !rounded-lg flex-shrink-0"
            title="发送消息"
          >
            {sending ? (
              <span className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
            ) : (
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <line x1="22" y1="2" x2="11" y2="13" />
                <polygon points="22,2 15,22 11,13 2,9 22,2" />
              </svg>
            )}
          </button>
        </div>
      </div>
    </div>
  );
}
