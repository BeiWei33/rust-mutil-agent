/**
 * 设置面板
 * 模型切换 / API Key 配置（localStorage 持久化）
 */

import { useState, useEffect } from "react";
import { useAgentStore } from "@/store/useAgentStore";
import type { AppSettings } from "@/types";

/** 可选模型列表 */
const MODEL_OPTIONS = [
  { value: "deepseek-v4-pro", label: "DeepSeek V4 Pro" },
  { value: "gpt-4o", label: "GPT-4o" },
  { value: "gpt-4o-mini", label: "GPT-4o Mini" },
  { value: "gpt-4-turbo", label: "GPT-4 Turbo" },
  { value: "claude-3.5-sonnet", label: "Claude 3.5 Sonnet" },
  { value: "claude-3-opus", label: "Claude 3 Opus" },
  { value: "deepseek-v3", label: "DeepSeek V3" },
  { value: "deepseek-r1", label: "DeepSeek R1" },
  { value: "qwen-max", label: "Qwen Max" },
  { value: "glm-4", label: "GLM-4" },
];

/** 表单区块 */
function SettingSection({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="card">
      <h3 className="text-sm font-medium text-zinc-200 mb-1">{title}</h3>
      {description && (
        <p className="text-xs text-zinc-500 mb-3">{description}</p>
      )}
      {children}
    </div>
  );
}

export default function SettingsPanel() {
  const settings = useAgentStore((s) => s.settings);
  const updateSettings = useAgentStore((s) => s.updateSettings);

  // 本地表单状态（允许用户自由编辑，失焦/提交时保存）
  const [form, setForm] = useState<AppSettings>(settings);
  const [saved, setSaved] = useState(false);
  const [showApiKey, setShowApiKey] = useState(false);

  // 同步外部 settings 变化
  useEffect(() => {
    setForm(settings);
  }, [settings]);

  // 保存设置
  const handleSave = (partial: Partial<AppSettings>) => {
    const updated = { ...form, ...partial };
    setForm(updated);
    updateSettings(partial);
    // 显示"已保存"提示
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  return (
    <div className="flex flex-col h-full">
      {/* 顶部工具栏 */}
      <div className="flex items-center justify-between px-6 py-3 border-b border-zinc-800/50">
        <h2 className="text-lg font-semibold text-zinc-200">⚙️ 设置</h2>
        {saved && (
          <span className="text-xs text-green-400 animate-fade-in">
            ✅ 已保存
          </span>
        )}
      </div>

      {/* 设置表单 */}
      <div className="flex-1 overflow-y-auto px-6 py-4 space-y-4">
        {/* 模型选择 */}
        <SettingSection
          title="模型选择"
          description="选择要使用的 AI 模型"
        >
          <select
            className="input-field"
            value={form.model}
            onChange={(e) => handleSave({ model: e.target.value })}
          >
            {MODEL_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
        </SettingSection>

        {/* API Key */}
        <SettingSection
          title="API Key"
          description="可留空；桌面端后端会优先从用户环境变量 DEEPSEEK_API_KEY 读取"
        >
          <div className="flex gap-2">
            <div className="relative flex-1">
              <input
                type={showApiKey ? "text" : "password"}
                className="input-field pr-10"
                value={form.apiKey}
                onChange={(e) =>
                  setForm((prev) => ({ ...prev, apiKey: e.target.value }))
                }
                onBlur={() => handleSave({ apiKey: form.apiKey })}
                placeholder="sk-... / 留空使用 DEEPSEEK_API_KEY"
              />
              <button
                onClick={() => setShowApiKey(!showApiKey)}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-zinc-500 hover:text-zinc-300"
                title={showApiKey ? "隐藏" : "显示"}
              >
                {showApiKey ? (
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M17.94 17.94A10.07 10.07 0 0112 20c-7 0-11-8-11-8a18.45 18.45 0 015.06-5.94M9.9 4.24A9.12 9.12 0 0112 4c7 0 11 8 11 8a18.5 18.5 0 01-2.16 3.19m-6.72-1.07a3 3 0 11-4.24-4.24" />
                    <line x1="1" y1="1" x2="23" y2="23" />
                  </svg>
                ) : (
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
                    <circle cx="12" cy="12" r="3" />
                  </svg>
                )}
              </button>
            </div>
          </div>
          {/* API Key 状态指示 */}
          <div className="mt-2 flex items-center gap-2">
            <span
              className={`status-dot ${form.apiKey ? "online" : "offline"}`}
            />
            <span className="text-xs text-zinc-500">
              {form.apiKey
                ? `已设置 (${form.apiKey.slice(0, 7)}...)`
                : "未设置"}
            </span>
          </div>
        </SettingSection>

        {/* API 基础 URL */}
        <SettingSection
          title="API 基础 URL"
          description="自定义 API 端点地址"
        >
          <input
            type="text"
            className="input-field"
            value={form.apiBaseUrl}
            onChange={(e) =>
              setForm((prev) => ({ ...prev, apiBaseUrl: e.target.value }))
            }
            onBlur={() => handleSave({ apiBaseUrl: form.apiBaseUrl })}
            placeholder="https://api.deepseek.com/v1"
          />
        </SettingSection>

        {/* 参数设置 */}
        <SettingSection title="高级参数" description="调整模型生成参数">
          <div className="space-y-4">
            {/* Max Tokens */}
            <div>
              <div className="flex items-center justify-between mb-1">
                <label className="text-xs text-zinc-400">最大 Token 数</label>
                <span className="text-xs text-primary-400 font-mono">
                  {form.maxTokens}
                </span>
              </div>
              <input
                type="range"
                min={256}
                max={32768}
                step={256}
                value={form.maxTokens}
                onChange={(e) =>
                  setForm((prev) => ({
                    ...prev,
                    maxTokens: parseInt(e.target.value),
                  }))
                }
                onMouseUp={() => handleSave({ maxTokens: form.maxTokens })}
                onTouchEnd={() => handleSave({ maxTokens: form.maxTokens })}
                className="w-full h-2 bg-zinc-700 rounded-lg appearance-none cursor-pointer
                           accent-primary-500"
              />
              <div className="flex justify-between text-[10px] text-zinc-600 mt-0.5">
                <span>256</span>
                <span>32,768</span>
              </div>
            </div>

            {/* Temperature */}
            <div>
              <div className="flex items-center justify-between mb-1">
                <label className="text-xs text-zinc-400">温度 (Temperature)</label>
                <span className="text-xs text-primary-400 font-mono">
                  {form.temperature.toFixed(1)}
                </span>
              </div>
              <input
                type="range"
                min={0}
                max={2}
                step={0.1}
                value={form.temperature}
                onChange={(e) =>
                  setForm((prev) => ({
                    ...prev,
                    temperature: parseFloat(e.target.value),
                  }))
                }
                onMouseUp={() => handleSave({ temperature: form.temperature })}
                onTouchEnd={() =>
                  handleSave({ temperature: form.temperature })
                }
                className="w-full h-2 bg-zinc-700 rounded-lg appearance-none cursor-pointer
                           accent-primary-500"
              />
              <div className="flex justify-between text-[10px] text-zinc-600 mt-0.5">
                <span>0 (精确)</span>
                <span>2 (创意)</span>
              </div>
            </div>
          </div>
        </SettingSection>

        {/* 存储信息 */}
        <div className="text-center py-2">
          <p className="text-xs text-zinc-600">
            所有设置仅存储在浏览器本地，不会上传到服务器
          </p>
        </div>
      </div>
    </div>
  );
}
