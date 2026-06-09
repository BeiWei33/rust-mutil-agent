/**
 * 长期记忆面板
 * 管理结构化知识条目的写入和检索。
 */

import { FormEvent, useEffect, useMemo, useState } from "react";
import { api } from "@/lib/tauri";
import { getErrorMessage } from "@/lib/errors";
import type { KnowledgeItem } from "@/types";

function formatTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

function splitTags(value: string): string[] {
  return value
    .split(/[,，\n]/)
    .map((tag) => tag.trim())
    .filter(Boolean);
}

function tagLabel(item: KnowledgeItem): string {
  return item.tags.length > 0 ? item.tags.join(" / ") : "未标记";
}

export default function MemoryPanel() {
  const [query, setQuery] = useState("FailureCase");
  const [items, setItems] = useState<KnowledgeItem[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);

  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [source, setSource] = useState("");
  const [tags, setTags] = useState("ProjectFact");
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [lastSavedId, setLastSavedId] = useState<string | null>(null);

  const currentTags = useMemo(() => splitTags(tags), [tags]);

  const runSearch = async (nextQuery = query) => {
    const trimmed = nextQuery.trim();
    if (!trimmed) {
      setSearchError("搜索关键词不能为空。");
      return;
    }
    setSearching(true);
    setSearchError(null);
    try {
      const result = await api.searchKnowledge({ query: trimmed, limit: 50 });
      setItems(result.items);
      setQuery(result.query);
    } catch (err: unknown) {
      setSearchError(getErrorMessage(err, "搜索长期记忆失败"));
    } finally {
      setSearching(false);
    }
  };

  useEffect(() => {
    runSearch("FailureCase");
  }, []);

  const handleSearch = (event: FormEvent) => {
    event.preventDefault();
    runSearch();
  };

  const handleSave = async (event: FormEvent) => {
    event.preventDefault();
    setSaving(true);
    setSaveError(null);
    setLastSavedId(null);
    try {
      const result = await api.storeKnowledge({
        title,
        content,
        source: source.trim() || null,
        tags: currentTags,
      });
      setLastSavedId(result.id);
      setTitle("");
      setContent("");
      await runSearch(currentTags[0] || title || query);
    } catch (err: unknown) {
      setSaveError(getErrorMessage(err, "保存长期记忆失败"));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-zinc-800/50 px-6 py-3">
        <div>
          <h2 className="text-lg font-semibold text-zinc-200">记忆</h2>
          <p className="mt-0.5 text-xs text-zinc-500">KnowledgeBase</p>
        </div>
        <form onSubmit={handleSearch} className="flex w-full max-w-xl items-center gap-2">
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            className="input-field flex-1 px-3 py-1.5 text-sm"
            placeholder="搜索长期记忆"
          />
          <button type="submit" disabled={searching} className="btn-primary px-3 py-1.5 text-xs">
            {searching ? "搜索中..." : "搜索"}
          </button>
        </form>
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-[380px_minmax(0,1fr)] overflow-hidden">
        <aside className="min-h-0 overflow-y-auto border-r border-zinc-800/50 px-5 py-5">
          <form onSubmit={handleSave} className="space-y-4">
            <div>
              <label className="mb-1.5 block text-xs font-medium text-zinc-400">标题</label>
              <input
                value={title}
                onChange={(event) => setTitle(event.target.value)}
                className="input-field w-full px-3 py-2 text-sm"
                placeholder="ProjectFact 或 FailureCase"
              />
            </div>

            <div>
              <label className="mb-1.5 block text-xs font-medium text-zinc-400">内容</label>
              <textarea
                value={content}
                onChange={(event) => setContent(event.target.value)}
                className="input-field min-h-[180px] w-full resize-y px-3 py-2 text-sm leading-6"
                placeholder="记录项目事实、失败原因、修复经验或成功模式"
              />
            </div>

            <div>
              <label className="mb-1.5 block text-xs font-medium text-zinc-400">来源</label>
              <input
                value={source}
                onChange={(event) => setSource(event.target.value)}
                className="input-field w-full px-3 py-2 text-sm"
                placeholder="taskId、patchId 或人工记录"
              />
            </div>

            <div>
              <label className="mb-1.5 block text-xs font-medium text-zinc-400">标签</label>
              <input
                value={tags}
                onChange={(event) => setTags(event.target.value)}
                className="input-field w-full px-3 py-2 text-sm"
                placeholder="ProjectFact, FailureCase"
              />
              {currentTags.length > 0 && (
                <div className="mt-2 flex flex-wrap gap-2">
                  {currentTags.map((tag) => (
                    <span
                      key={tag}
                      className="rounded-md border border-primary-500/20 bg-primary-500/10 px-2 py-1 text-xs text-primary-200"
                    >
                      {tag}
                    </span>
                  ))}
                </div>
              )}
            </div>

            {saveError && (
              <div className="rounded-lg border border-red-500/20 bg-red-500/10 p-3 text-sm text-red-300">
                {saveError}
              </div>
            )}
            {lastSavedId && (
              <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/10 p-3 text-sm text-emerald-300">
                已保存：{lastSavedId}
              </div>
            )}

            <button
              type="submit"
              disabled={saving}
              className="btn-primary w-full px-3 py-2 text-sm"
            >
              {saving ? "保存中..." : "保存记忆"}
            </button>
          </form>
        </aside>

        <section className="min-h-0 overflow-y-auto px-6 py-5">
          {searchError && (
            <div className="mb-4 rounded-lg border border-red-500/20 bg-red-500/10 p-3 text-sm text-red-300">
              {searchError}
            </div>
          )}

          <div className="mb-4 flex items-center justify-between">
            <h3 className="text-sm font-medium text-zinc-300">检索结果</h3>
            <span className="text-xs text-zinc-500">{items.length} 条</span>
          </div>

          {items.length > 0 ? (
            <div className="space-y-3">
              {items.map((item) => (
                <article
                  key={item.id}
                  className="rounded-lg border border-zinc-800 bg-zinc-900/40 p-4"
                >
                  <div className="flex items-start justify-between gap-4">
                    <div className="min-w-0">
                      <h4 className="truncate text-sm font-medium text-zinc-100">{item.title}</h4>
                      <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-zinc-500">
                        <span>{tagLabel(item)}</span>
                        {item.source && <span>来源：{item.source}</span>}
                        <span>{formatTime(item.createdAt)}</span>
                      </div>
                    </div>
                  </div>
                  <p className="mt-3 whitespace-pre-wrap text-sm leading-6 text-zinc-300">
                    {item.content}
                  </p>
                </article>
              ))}
            </div>
          ) : (
            <div className="flex h-64 items-center justify-center rounded-lg border border-dashed border-zinc-800 text-sm text-zinc-500">
              {searching ? "正在搜索..." : "暂无匹配记忆"}
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
