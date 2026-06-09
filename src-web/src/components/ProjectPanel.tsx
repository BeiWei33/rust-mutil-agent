/**
 * 项目理解面板
 * 展示项目快照、文件索引、文本搜索和只读文件预览。
 */

import { useEffect, useMemo, useState } from "react";
import { useAgentStore } from "@/store/useAgentStore";
import type {
  PatchProposal,
  ProjectCommandRunResponse,
  ToolInvocationRecord,
  WorkspaceEntry,
} from "@/types";

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

function isReadable(entry: WorkspaceEntry): boolean {
  if (entry.isDir) return false;
  return [
    "rs",
    "ts",
    "tsx",
    "js",
    "jsx",
    "json",
    "toml",
    "md",
    "css",
    "html",
    "yml",
    "yaml",
    "txt",
  ].includes((entry.extension || "").toLowerCase());
}

function commandKey(command: { command: string; workingDir: string }): string {
  return `${command.workingDir}:${command.command}`;
}

function commandStatusLabel(result: ProjectCommandRunResponse): string {
  if (result.timedOut) return "超时";
  return result.success ? "通过" : "失败";
}

function toolStatusLabel(invocation: ToolInvocationRecord): string {
  return invocation.success ? "通过" : "失败";
}

function formatArgsSummary(value: unknown): string {
  if (value === null || value === undefined) return "-";
  if (typeof value === "string") return value;
  try {
    const text = JSON.stringify(value);
    return text.length > 96 ? `${text.slice(0, 96)}...` : text;
  } catch {
    return String(value);
  }
}

function formatRunTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

function patchStatusLabel(proposal: PatchProposal): string {
  if (proposal.status === "reverted") return "已回滚";
  if (proposal.status === "applied") return "已应用";
  if (proposal.status === "approved") return "已通过";
  if (proposal.status === "rejected") return "已拒绝";
  if (proposal.status === "pendingApproval") return "待审批";
  return "草稿";
}

export default function ProjectPanel() {
  const snapshot = useAgentStore((s) => s.projectSnapshot);
  const files = useAgentStore((s) => s.projectFiles);
  const loading = useAgentStore((s) => s.projectLoading);
  const error = useAgentStore((s) => s.projectError);
  const selectedFile = useAgentStore((s) => s.selectedProjectFile);
  const fileLoading = useAgentStore((s) => s.fileLoading);
  const searchResults = useAgentStore((s) => s.searchResults);
  const searchTruncated = useAgentStore((s) => s.searchTruncated);
  const searchLoading = useAgentStore((s) => s.searchLoading);
  const commandRunLoadingKey = useAgentStore((s) => s.commandRunLoadingKey);
  const commandRunError = useAgentStore((s) => s.commandRunError);
  const commandApprovalLoading = useAgentStore((s) => s.commandApprovalLoading);
  const commandApprovalError = useAgentStore((s) => s.commandApprovalError);
  const lastCommandApproval = useAgentStore((s) => s.lastCommandApproval);
  const latestCommandRun = useAgentStore((s) => s.latestCommandRun);
  const commandRuns = useAgentStore((s) => s.commandRuns);
  const toolInvocations = useAgentStore((s) => s.toolInvocations);
  const toolInvocationError = useAgentStore((s) => s.toolInvocationError);
  const patchProposals = useAgentStore((s) => s.patchProposals);
  const patchProposalLoading = useAgentStore((s) => s.patchProposalLoading);
  const patchProposalError = useAgentStore((s) => s.patchProposalError);
  const lastPatchProposal = useAgentStore((s) => s.lastPatchProposal);
  const setCurrentPage = useAgentStore((s) => s.setCurrentPage);
  const fetchProjectOverview = useAgentStore((s) => s.fetchProjectOverview);
  const readProjectFile = useAgentStore((s) => s.readProjectFile);
  const searchProjectText = useAgentStore((s) => s.searchProjectText);
  const runProjectCommand = useAgentStore((s) => s.runProjectCommand);
  const requestProjectCommandApproval = useAgentStore((s) => s.requestProjectCommandApproval);
  const createPatchProposal = useAgentStore((s) => s.createPatchProposal);
  const clearProjectError = useAgentStore((s) => s.clearProjectError);

  const [filter, setFilter] = useState("");
  const [query, setQuery] = useState("");
  const [customCommand, setCustomCommand] = useState("");
  const [customWorkingDir, setCustomWorkingDir] = useState("src-tauri");
  const [patchSummary, setPatchSummary] = useState("");
  const [patchDraft, setPatchDraft] = useState("");

  useEffect(() => {
    fetchProjectOverview();
  }, [fetchProjectOverview]);

  useEffect(() => {
    if (!selectedFile) return;
    setPatchSummary(`修改 ${selectedFile.path}`);
    setPatchDraft(selectedFile.content);
  }, [selectedFile?.path]);

  const visibleFiles = useMemo(() => {
    const text = filter.trim().toLowerCase();
    const candidates = files.filter(isReadable);
    if (!text) return candidates.slice(0, 120);
    return candidates
      .filter((file) => file.path.toLowerCase().includes(text))
      .slice(0, 120);
  }, [files, filter]);

  const handleSearch = () => {
    searchProjectText(query);
  };

  const handleRunCommand = (command: { command: string; workingDir: string }) => {
    runProjectCommand({
      command: command.command,
      workingDir: command.workingDir,
    });
  };

  const handleRequestCommandApproval = () => {
    requestProjectCommandApproval({
      command: customCommand,
      workingDir: customWorkingDir,
    });
  };

  const handleCreatePatchProposal = () => {
    if (!selectedFile) return;
    createPatchProposal({
      summary: patchSummary.trim() || `修改 ${selectedFile.path}`,
      files: [
        {
          path: selectedFile.path,
          oldContent: selectedFile.content,
          newContent: patchDraft,
        },
      ],
    });
  };

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-zinc-800/50 px-6 py-3">
        <div>
          <h2 className="text-lg font-semibold text-zinc-200">项目</h2>
          <p className="mt-0.5 text-xs text-zinc-500">
            {snapshot ? snapshot.name : "workspace"}
          </p>
        </div>
        <button
          onClick={fetchProjectOverview}
          disabled={loading}
          className="btn-ghost px-3 py-1.5 text-xs"
          title="刷新项目"
        >
          {loading ? "刷新中..." : "刷新"}
        </button>
      </div>

      {error && (
        <div className="mx-6 mt-4 flex items-center justify-between rounded-lg border border-red-500/20 bg-red-500/10 p-3 text-sm text-red-300">
          <span>{error}</span>
          <button className="btn-ghost px-2 py-1 text-xs" onClick={clearProjectError}>
            关闭
          </button>
        </div>
      )}

      <div className="grid min-h-0 flex-1 grid-cols-[380px_minmax(0,1fr)] overflow-hidden">
        <aside className="min-h-0 overflow-y-auto border-r border-zinc-800/50 px-5 py-5">
          {snapshot ? (
            <div className="space-y-5">
              <section>
                <h3 className="text-sm font-medium text-zinc-300">技术栈</h3>
                <div className="mt-3 flex flex-wrap gap-2">
                  {snapshot.techStack.map((item) => (
                    <span
                      key={item}
                      className="rounded-md border border-primary-500/20 bg-primary-500/10 px-2 py-1 text-xs text-primary-200"
                    >
                      {item}
                    </span>
                  ))}
                </div>
              </section>

              <section>
                <h3 className="text-sm font-medium text-zinc-300">Manifest</h3>
                <div className="mt-3 space-y-2">
                  {snapshot.manifests.map((manifest) => (
                    <button
                      key={manifest.path}
                      type="button"
                      onClick={() => readProjectFile(manifest.path)}
                      className="w-full rounded-lg border border-zinc-800 bg-zinc-900/40 p-3 text-left hover:border-zinc-700"
                    >
                      <div className="text-xs text-zinc-500">{manifest.kind}</div>
                      <div className="mt-1 text-sm text-zinc-200">{manifest.path}</div>
                      <div className="mt-1 text-xs text-zinc-500">{manifest.summary}</div>
                    </button>
                  ))}
                </div>
              </section>

              <section>
                <h3 className="text-sm font-medium text-zinc-300">推荐命令</h3>
                <div className="mt-3 space-y-2">
                  {snapshot.recommendedCommands.map((command) => (
                    <div
                      key={`${command.workingDir}-${command.command}`}
                      className="rounded-lg border border-zinc-800 bg-zinc-900/40 p-3"
                    >
                      <div className="flex items-center justify-between gap-2">
                        <span className="text-sm text-zinc-200">{command.label}</span>
                        <div className="flex shrink-0 items-center gap-2">
                          <span className="rounded-md border border-zinc-700 px-2 py-0.5 text-[11px] text-zinc-400">
                            {command.kind}
                          </span>
                          <button
                            type="button"
                            onClick={() => handleRunCommand(command)}
                            disabled={commandRunLoadingKey !== null}
                            className="btn-ghost px-2 py-1 text-[11px]"
                            title="运行命令"
                          >
                            {commandRunLoadingKey === commandKey(command) ? "运行中..." : "运行"}
                          </button>
                        </div>
                      </div>
                      <code className="mt-2 block rounded-md bg-zinc-950 px-2 py-1 text-xs text-primary-200">
                        {command.command}
                      </code>
                      <div className="mt-1 text-[11px] text-zinc-500">
                        {command.workingDir}
                      </div>
                    </div>
                  ))}
                </div>
                <div className="mt-3 rounded-lg border border-zinc-800 bg-zinc-900/35 p-3">
                  <div className="flex items-center justify-between gap-2">
                    <h4 className="text-xs font-medium text-zinc-300">命令审批</h4>
                    {lastCommandApproval && (
                      <button
                        type="button"
                        onClick={() => setCurrentPage("approvals")}
                        className="btn-ghost px-2 py-1 text-[11px]"
                        title="查看审批"
                      >
                        查看
                      </button>
                    )}
                  </div>
                  <div className="mt-3 grid gap-2">
                    <input
                      value={customCommand}
                      onChange={(event) => setCustomCommand(event.target.value)}
                      placeholder="cargo clippy"
                      className="w-full rounded-md border border-zinc-800 bg-zinc-950 px-2 py-2 text-xs text-zinc-200 outline-none placeholder:text-zinc-600 focus:border-primary-500/50"
                    />
                    <div className="flex gap-2">
                      <input
                        value={customWorkingDir}
                        onChange={(event) => setCustomWorkingDir(event.target.value)}
                        placeholder="src-tauri"
                        className="min-w-0 flex-1 rounded-md border border-zinc-800 bg-zinc-950 px-2 py-2 text-xs text-zinc-200 outline-none placeholder:text-zinc-600 focus:border-primary-500/50"
                      />
                      <button
                        type="button"
                        onClick={handleRequestCommandApproval}
                        disabled={
                          commandApprovalLoading ||
                          !customCommand.trim() ||
                          !customWorkingDir.trim()
                        }
                        className="btn-primary shrink-0 px-3 py-2 text-xs disabled:cursor-not-allowed disabled:opacity-60"
                      >
                        {commandApprovalLoading ? "提交中..." : "申请审批"}
                      </button>
                    </div>
                  </div>
                  {commandApprovalError && (
                    <div className="mt-3 text-xs text-red-300">{commandApprovalError}</div>
                  )}
                  {lastCommandApproval && (
                    <div className="mt-3 rounded-md border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-200">
                      已创建：{lastCommandApproval.title}
                    </div>
                  )}
                </div>
                {(commandRunError || latestCommandRun) && (
                  <div className="mt-3 rounded-lg border border-zinc-800 bg-zinc-950 p-3">
                    {commandRunError && (
                      <div className="text-sm text-red-300">{commandRunError}</div>
                    )}
                    {latestCommandRun && (
                      <div className="space-y-3">
                        <div className="flex items-center justify-between gap-3">
                          <div>
                            <div
                              className={`text-sm font-medium ${
                                latestCommandRun.success
                                  ? "text-emerald-300"
                                  : "text-amber-300"
                              }`}
                            >
                              {commandStatusLabel(latestCommandRun)}
                            </div>
                            <div className="mt-1 text-[11px] text-zinc-500">
                              {latestCommandRun.workingDir} · {latestCommandRun.durationMs}ms
                              {latestCommandRun.exitCode !== null &&
                                latestCommandRun.exitCode !== undefined &&
                                ` · exit ${latestCommandRun.exitCode}`}
                            </div>
                          </div>
                          {(latestCommandRun.stdoutTruncated ||
                            latestCommandRun.stderrTruncated) && (
                            <span className="text-[11px] text-amber-300">输出已截断</span>
                          )}
                        </div>
                        <code className="block rounded-md bg-zinc-900 px-2 py-1 text-xs text-primary-200">
                          {latestCommandRun.command}
                        </code>
                        {latestCommandRun.stdout && (
                          <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded-md bg-zinc-900 p-2 text-[11px] leading-relaxed text-zinc-300">
                            {latestCommandRun.stdout}
                          </pre>
                        )}
                        {latestCommandRun.stderr && (
                          <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded-md bg-red-950/30 p-2 text-[11px] leading-relaxed text-red-200">
                            {latestCommandRun.stderr}
                          </pre>
                        )}
                      </div>
                    )}
                  </div>
                )}
                {commandRuns.length > 0 && (
                  <div className="mt-3 rounded-lg border border-zinc-800 bg-zinc-900/35 p-3">
                    <div className="mb-2 flex items-center justify-between">
                      <h4 className="text-xs font-medium text-zinc-300">最近运行</h4>
                      <span className="text-[11px] text-zinc-500">{commandRuns.length}</span>
                    </div>
                    <div className="space-y-2">
                      {commandRuns.slice(0, 5).map((run) => (
                        <div
                          key={run.id}
                          className="rounded-md border border-zinc-800 bg-zinc-950 px-2 py-2"
                        >
                          <div className="flex items-center justify-between gap-2">
                            <code className="truncate text-[11px] text-primary-200">
                              {run.command}
                            </code>
                            <span
                              className={`shrink-0 text-[11px] ${
                                run.success ? "text-emerald-300" : "text-amber-300"
                              }`}
                            >
                              {commandStatusLabel(run)}
                            </span>
                          </div>
                          <div className="mt-1 flex items-center justify-between gap-2 text-[11px] text-zinc-500">
                            <span className="truncate">{run.workingDir}</span>
                            <span className="shrink-0">
                              {run.durationMs}ms · {formatRunTime(run.createdAt)}
                            </span>
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
                {(toolInvocationError || toolInvocations.length > 0) && (
                  <div className="mt-3 rounded-lg border border-zinc-800 bg-zinc-900/35 p-3">
                    <div className="mb-2 flex items-center justify-between">
                      <h4 className="text-xs font-medium text-zinc-300">最近工具</h4>
                      <span className="text-[11px] text-zinc-500">
                        {toolInvocations.length}
                      </span>
                    </div>
                    {toolInvocationError && (
                      <div className="mb-2 text-xs text-red-300">{toolInvocationError}</div>
                    )}
                    <div className="space-y-2">
                      {toolInvocations.slice(0, 5).map((invocation) => (
                        <div
                          key={invocation.id}
                          className="rounded-md border border-zinc-800 bg-zinc-950 px-2 py-2"
                        >
                          <div className="flex items-center justify-between gap-2">
                            <code className="truncate text-[11px] text-primary-200">
                              {invocation.toolName}
                            </code>
                            <span
                              className={`shrink-0 text-[11px] ${
                                invocation.success ? "text-emerald-300" : "text-amber-300"
                              }`}
                            >
                              {toolStatusLabel(invocation)}
                            </span>
                          </div>
                          <div className="mt-1 truncate text-[11px] text-zinc-500">
                            {formatArgsSummary(invocation.argsSummary)}
                          </div>
                          <div className="mt-1 flex items-center justify-between gap-2 text-[11px] text-zinc-500">
                            <span className="truncate">
                              {invocation.approvalId ? `审批 ${invocation.approvalId}` : "无审批"}
                            </span>
                            <span className="shrink-0">
                              {invocation.durationMs}ms · {formatRunTime(invocation.createdAt)}
                            </span>
                          </div>
                          {invocation.error && (
                            <div className="mt-1 truncate text-[11px] text-red-300">
                              {invocation.error}
                            </div>
                          )}
                        </div>
                      ))}
                    </div>
                  </div>
                )}
                {patchProposals.length > 0 && (
                  <div className="mt-3 rounded-lg border border-zinc-800 bg-zinc-900/35 p-3">
                    <div className="mb-2 flex items-center justify-between">
                      <h4 className="text-xs font-medium text-zinc-300">最近补丁</h4>
                      <button
                        type="button"
                        onClick={() => setCurrentPage("approvals")}
                        className="btn-ghost px-2 py-1 text-[11px]"
                        title="查看审批"
                      >
                        审批
                      </button>
                    </div>
                    <div className="space-y-2">
                      {patchProposals.slice(0, 4).map((proposal) => (
                        <div
                          key={proposal.id}
                          className="rounded-md border border-zinc-800 bg-zinc-950 px-2 py-2"
                        >
                          <div className="flex items-center justify-between gap-2">
                            <span className="truncate text-[11px] text-zinc-200">
                              {proposal.summary}
                            </span>
                            <span className="shrink-0 text-[11px] text-amber-300">
                              {patchStatusLabel(proposal)}
                            </span>
                          </div>
                          <div className="mt-1 text-[11px] text-zinc-500">
                            {proposal.files.length} 文件 · {formatRunTime(proposal.updatedAt)}
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </section>

              <section>
                <h3 className="text-sm font-medium text-zinc-300">关键文件</h3>
                <div className="mt-3 space-y-2">
                  {snapshot.importantFiles.map((file) => (
                    <button
                      key={file.path}
                      type="button"
                      onClick={() => readProjectFile(file.path)}
                      className="w-full rounded-lg border border-zinc-800 bg-zinc-900/40 p-3 text-left hover:border-zinc-700"
                    >
                      <div className="text-xs text-zinc-500">{file.kind}</div>
                      <div className="mt-1 text-sm text-zinc-200">{file.path}</div>
                      <div className="mt-1 text-xs leading-relaxed text-zinc-500">
                        {file.description}
                      </div>
                    </button>
                  ))}
                </div>
              </section>
            </div>
          ) : (
            <div className="py-12 text-center text-sm text-zinc-500">
              {loading ? "正在扫描项目..." : "暂无项目快照"}
            </div>
          )}
        </aside>

        <section className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden">
          <div className="border-b border-zinc-800/50 px-5 py-4">
            <div className="grid gap-3 lg:grid-cols-2">
              <div className="rounded-lg border border-zinc-800 bg-zinc-900/40 p-2">
                <input
                  value={filter}
                  onChange={(event) => setFilter(event.target.value)}
                  placeholder="过滤文件路径"
                  className="w-full bg-transparent px-2 py-2 text-sm text-zinc-200 outline-none placeholder:text-zinc-500"
                />
              </div>
              <div className="flex gap-2 rounded-lg border border-zinc-800 bg-zinc-900/40 p-2">
                <input
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") handleSearch();
                  }}
                  placeholder="搜索源码文本"
                  className="min-w-0 flex-1 bg-transparent px-2 py-2 text-sm text-zinc-200 outline-none placeholder:text-zinc-500"
                />
                <button
                  type="button"
                  onClick={handleSearch}
                  disabled={!query.trim() || searchLoading}
                  className="btn-primary px-3 py-2 text-xs"
                >
                  {searchLoading ? "搜索中..." : "搜索"}
                </button>
              </div>
            </div>
          </div>

          <div className="grid min-h-0 grid-cols-[360px_minmax(0,1fr)] overflow-hidden">
            <div className="min-h-0 overflow-y-auto border-r border-zinc-800/50 px-4 py-4">
              <div className="mb-3 flex items-center justify-between">
                <h3 className="text-sm font-medium text-zinc-300">文件</h3>
                <span className="text-xs text-zinc-500">{visibleFiles.length}</span>
              </div>
              <div className="space-y-2">
                {visibleFiles.map((file) => (
                  <button
                    key={file.path}
                    type="button"
                    onClick={() => readProjectFile(file.path)}
                    className={`w-full rounded-lg border p-3 text-left transition-colors ${
                      selectedFile?.path === file.path
                        ? "border-primary-500/50 bg-primary-500/10"
                        : "border-zinc-800 bg-zinc-900/35 hover:border-zinc-700"
                    }`}
                  >
                    <div className="truncate text-sm text-zinc-200">{file.path}</div>
                    <div className="mt-1 text-[11px] text-zinc-500">
                      {formatBytes(file.sizeBytes)}
                    </div>
                  </button>
                ))}
              </div>
            </div>

            <div className="min-h-0 overflow-y-auto px-5 py-4">
              {searchResults.length > 0 && (
                <section className="mb-5">
                  <div className="mb-3 flex items-center gap-2">
                    <h3 className="text-sm font-medium text-zinc-300">搜索结果</h3>
                    {searchTruncated && (
                      <span className="text-xs text-amber-300">结果已截断</span>
                    )}
                  </div>
                  <div className="space-y-2">
                    {searchResults.map((match) => (
                      <button
                        key={`${match.path}-${match.line}-${match.column}`}
                        type="button"
                        onClick={() => readProjectFile(match.path)}
                        className="w-full rounded-lg border border-zinc-800 bg-zinc-900/40 p-3 text-left hover:border-zinc-700"
                      >
                        <div className="text-xs text-zinc-500">
                          {match.path}:{match.line}:{match.column}
                        </div>
                        <div className="mt-1 text-sm text-zinc-300">{match.preview}</div>
                      </button>
                    ))}
                  </div>
                </section>
              )}

              <section>
                <div className="mb-3 flex items-center justify-between">
                  <h3 className="text-sm font-medium text-zinc-300">预览</h3>
                  {selectedFile && (
                    <span className="text-xs text-zinc-500">
                      {formatBytes(selectedFile.sizeBytes)}
                    </span>
                  )}
                </div>
                {fileLoading ? (
                  <div className="py-16 text-center text-sm text-zinc-500">读取中...</div>
                ) : selectedFile ? (
                  <div className="space-y-4">
                    <div className="overflow-hidden rounded-lg border border-zinc-800 bg-zinc-950">
                      <div className="border-b border-zinc-800 px-3 py-2 text-xs text-zinc-400">
                        {selectedFile.path}
                      </div>
                      <pre className="max-h-[42vh] overflow-auto p-4 text-xs leading-relaxed text-zinc-300">
                        <code>{selectedFile.content}</code>
                      </pre>
                    </div>
                    <div className="rounded-lg border border-zinc-800 bg-zinc-900/40 p-3">
                      <div className="flex items-center justify-between gap-3">
                        <h4 className="text-sm font-medium text-zinc-300">补丁提案</h4>
                        <button
                          type="button"
                          onClick={handleCreatePatchProposal}
                          disabled={
                            patchProposalLoading ||
                            !patchSummary.trim() ||
                            patchDraft === selectedFile.content
                          }
                          className="btn-primary px-3 py-1.5 text-xs disabled:cursor-not-allowed disabled:opacity-60"
                        >
                          {patchProposalLoading ? "提交中..." : "提交审批"}
                        </button>
                      </div>
                      <input
                        value={patchSummary}
                        onChange={(event) => setPatchSummary(event.target.value)}
                        className="mt-3 w-full rounded-md border border-zinc-800 bg-zinc-950 px-2 py-2 text-xs text-zinc-200 outline-none placeholder:text-zinc-600 focus:border-primary-500/50"
                      />
                      <textarea
                        value={patchDraft}
                        onChange={(event) => setPatchDraft(event.target.value)}
                        spellCheck={false}
                        className="mt-3 h-64 w-full resize-y rounded-md border border-zinc-800 bg-zinc-950 p-3 font-mono text-xs leading-relaxed text-zinc-300 outline-none focus:border-primary-500/50"
                      />
                      {patchProposalError && (
                        <div className="mt-3 text-xs text-red-300">{patchProposalError}</div>
                      )}
                      {lastPatchProposal && (
                        <div className="mt-3 rounded-md border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-xs text-emerald-200">
                          已创建：{lastPatchProposal.summary}
                        </div>
                      )}
                      {lastPatchProposal?.unifiedDiff && (
                        <pre className="mt-3 max-h-72 overflow-auto whitespace-pre-wrap rounded-md bg-zinc-950 p-3 text-[11px] leading-relaxed text-zinc-300">
                          {lastPatchProposal.unifiedDiff}
                        </pre>
                      )}
                    </div>
                  </div>
                ) : (
                  <div className="py-16 text-center text-sm text-zinc-500">
                    选择一个文本文件
                  </div>
                )}
              </section>
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}
