/**
 * 任务看板
 * 展示软件工程任务、步骤状态和事件时间线。
 */

import { useEffect, useMemo, useState } from "react";
import { useAgentStore } from "@/store/useAgentStore";
import type { StepStatus, Task, TaskEvent, TaskStatus } from "@/types";

const TASK_STATUS: Record<TaskStatus, { label: string; className: string }> = {
  draft: { label: "草稿", className: "bg-zinc-500/10 text-zinc-300 border-zinc-500/20" },
  planning: { label: "规划中", className: "bg-sky-500/10 text-sky-300 border-sky-500/20" },
  waitingApproval: { label: "待审批", className: "bg-amber-500/10 text-amber-300 border-amber-500/20" },
  running: { label: "运行中", className: "bg-primary-500/10 text-primary-300 border-primary-500/20" },
  reviewing: { label: "审查中", className: "bg-violet-500/10 text-violet-300 border-violet-500/20" },
  failed: { label: "失败", className: "bg-red-500/10 text-red-300 border-red-500/20" },
  completed: { label: "完成", className: "bg-green-500/10 text-green-300 border-green-500/20" },
  cancelled: { label: "已取消", className: "bg-zinc-500/10 text-zinc-400 border-zinc-500/20" },
};

const STEP_STATUS: Record<StepStatus, { label: string; dot: string; className: string }> = {
  pending: {
    label: "等待",
    dot: "bg-zinc-500",
    className: "bg-zinc-500/10 text-zinc-300 border-zinc-500/20",
  },
  waitingApproval: {
    label: "待审批",
    dot: "bg-amber-400",
    className: "bg-amber-500/10 text-amber-300 border-amber-500/20",
  },
  running: {
    label: "运行",
    dot: "bg-primary-400 animate-pulse",
    className: "bg-primary-500/10 text-primary-300 border-primary-500/20",
  },
  failed: {
    label: "失败",
    dot: "bg-red-400",
    className: "bg-red-500/10 text-red-300 border-red-500/20",
  },
  completed: {
    label: "完成",
    dot: "bg-green-400",
    className: "bg-green-500/10 text-green-300 border-green-500/20",
  },
  skipped: {
    label: "跳过",
    dot: "bg-zinc-600",
    className: "bg-zinc-500/10 text-zinc-400 border-zinc-500/20",
  },
};

function formatTime(value: string): string {
  return new Date(value).toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function statusPill(status: TaskStatus) {
  const item = TASK_STATUS[status];
  return (
    <span className={`inline-flex items-center rounded-md border px-2 py-1 text-[11px] ${item.className}`}>
      {item.label}
    </span>
  );
}

function progressForTask(task: Task): number {
  if (task.steps.length === 0) return task.status === "completed" ? 100 : 0;
  const completed = task.steps.filter((step) => step.status === "completed").length;
  return Math.round((completed / task.steps.length) * 100);
}

function canCancelTask(task: Task): boolean {
  return ["draft", "planning", "waitingApproval", "running", "reviewing"].includes(task.status);
}

function canRetryTask(task: Task): boolean {
  return ["failed", "cancelled"].includes(task.status);
}

function TaskListItem({
  task,
  selected,
  onSelect,
}: {
  task: Task;
  selected: boolean;
  onSelect: () => void;
}) {
  const progress = progressForTask(task);

  return (
    <button
      type="button"
      onClick={onSelect}
      className={`w-full rounded-lg border p-3 text-left transition-colors ${
        selected
          ? "border-primary-500/50 bg-primary-500/10"
          : "border-zinc-800 bg-zinc-900/40 hover:border-zinc-700"
      }`}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="truncate text-sm font-medium text-zinc-200">{task.title}</div>
          <div className="mt-1 text-[11px] text-zinc-500">
            {formatTime(task.createdAt)} · {task.steps.length} 步
          </div>
        </div>
        {statusPill(task.status)}
      </div>
      <div className="mt-3 h-1.5 rounded-full bg-zinc-800">
        <div
          className="h-full rounded-full bg-primary-500 transition-all"
          style={{ width: `${progress}%` }}
        />
      </div>
    </button>
  );
}

function StepRow({ step }: { step: Task["steps"][number] }) {
  const status = STEP_STATUS[step.status];
  return (
    <div className="rounded-lg border border-zinc-800 bg-zinc-900/35 px-4 py-3">
      <div className="flex items-start gap-3">
        <span className={`mt-1.5 h-2 w-2 rounded-full ${status.dot}`} />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs text-zinc-500">#{step.order}</span>
            <h4 className="text-sm font-medium text-zinc-200">{step.title}</h4>
            <span className={`rounded-md border px-2 py-0.5 text-[11px] ${status.className}`}>
              {status.label}
            </span>
          </div>
          <p className="mt-1 text-xs leading-relaxed text-zinc-400">{step.instruction}</p>
          <div className="mt-2 flex flex-wrap items-center gap-2 text-[11px] text-zinc-500">
            <span>Agent: {step.agentId}</span>
            <span>尝试: {step.attempts}</span>
            {step.completedAt && <span>完成: {formatTime(step.completedAt)}</span>}
          </div>
          {step.error && (
            <p className="mt-2 rounded-md border border-red-500/20 bg-red-500/10 px-2 py-1 text-xs text-red-200">
              {step.error}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}

function EventRow({ event }: { event: TaskEvent }) {
  return (
    <div className="flex gap-3 border-l border-zinc-800 pl-3">
      <span className="mt-1.5 h-2 w-2 rounded-full bg-zinc-500" />
      <div className="min-w-0 pb-3">
        <div className="text-xs text-zinc-500">{formatTime(event.createdAt)}</div>
        <div className="mt-0.5 text-sm text-zinc-300">{event.message}</div>
      </div>
    </div>
  );
}

interface PatchAppliedArtifact {
  kind: "patchApplied";
  patchId: string;
  approvalId?: string;
  summary?: string;
  files?: string[];
  appliedAt?: string;
}

function patchAppliedArtifact(value: unknown): PatchAppliedArtifact | null {
  if (!value || typeof value !== "object") return null;
  const artifact = value as {
    kind?: unknown;
    patchId?: unknown;
    approvalId?: unknown;
    summary?: unknown;
    files?: unknown;
    appliedAt?: unknown;
  };
  if (artifact.kind !== "patchApplied" || typeof artifact.patchId !== "string") {
    return null;
  }
  return {
    kind: "patchApplied",
    patchId: artifact.patchId,
    approvalId: typeof artifact.approvalId === "string" ? artifact.approvalId : undefined,
    summary: typeof artifact.summary === "string" ? artifact.summary : undefined,
    files: Array.isArray(artifact.files)
      ? artifact.files.filter((file): file is string => typeof file === "string")
      : undefined,
    appliedAt: typeof artifact.appliedAt === "string" ? artifact.appliedAt : undefined,
  };
}

function ArtifactRow({ artifact }: { artifact: unknown }) {
  const patchArtifact = patchAppliedArtifact(artifact);
  if (patchArtifact) {
    return (
      <div className="rounded-lg border border-zinc-800 bg-zinc-900/35 p-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="min-w-0">
            <div className="truncate text-sm font-medium text-zinc-200">
              {patchArtifact.summary || patchArtifact.patchId}
            </div>
            <div className="mt-1 text-[11px] text-zinc-500">
              {patchArtifact.files?.length ?? 0} 文件
              {patchArtifact.appliedAt && ` · ${formatTime(patchArtifact.appliedAt)}`}
            </div>
          </div>
          <span className="rounded-md border border-emerald-500/20 bg-emerald-500/10 px-2 py-0.5 text-[11px] text-emerald-200">
            Patch
          </span>
        </div>
        {patchArtifact.files && patchArtifact.files.length > 0 && (
          <div className="mt-2 flex flex-wrap gap-2">
            {patchArtifact.files.map((file) => (
              <span
                key={file}
                className="rounded-md border border-zinc-700 px-2 py-0.5 text-[11px] text-zinc-300"
              >
                {file}
              </span>
            ))}
          </div>
        )}
      </div>
    );
  }

  return (
    <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded-lg border border-zinc-800 bg-zinc-950 p-3 text-xs text-zinc-300">
      {JSON.stringify(artifact, null, 2)}
    </pre>
  );
}

export default function TaskBoard() {
  const tasks = useAgentStore((s) => s.tasks);
  const tasksLoading = useAgentStore((s) => s.tasksLoading);
  const tasksError = useAgentStore((s) => s.tasksError);
  const selectedTaskId = useAgentStore((s) => s.selectedTaskId);
  const selectedTask = useAgentStore((s) => s.selectedTask);
  const taskEvents = useAgentStore((s) => s.taskEvents);
  const taskEventsLoading = useAgentStore((s) => s.taskEventsLoading);
  const createTask = useAgentStore((s) => s.createTask);
  const cancelTask = useAgentStore((s) => s.cancelTask);
  const retryTask = useAgentStore((s) => s.retryTask);
  const fetchTasks = useAgentStore((s) => s.fetchTasks);
  const setSelectedTaskId = useAgentStore((s) => s.setSelectedTaskId);
  const startTaskPolling = useAgentStore((s) => s.startTaskPolling);

  const [draft, setDraft] = useState("");
  const [creating, setCreating] = useState(false);
  const [cancellingTaskId, setCancellingTaskId] = useState<string | null>(null);
  const [retryingTaskId, setRetryingTaskId] = useState<string | null>(null);

  useEffect(() => {
    const stop = startTaskPolling(2500);
    return stop;
  }, [startTaskPolling]);

  const orderedEvents = useMemo(
    () => [...taskEvents].sort((a, b) => a.createdAt.localeCompare(b.createdAt)),
    [taskEvents]
  );

  const handleCreate = async () => {
    const content = draft.trim();
    if (!content || creating) return;
    setCreating(true);
    setDraft("");
    await createTask(content);
    setCreating(false);
  };

  const handleCancel = async (task: Task) => {
    if (!canCancelTask(task) || cancellingTaskId) return;
    setCancellingTaskId(task.id);
    await cancelTask(task.id);
    setCancellingTaskId(null);
  };

  const handleRetry = async (task: Task) => {
    if (!canRetryTask(task) || retryingTaskId) return;
    setRetryingTaskId(task.id);
    await retryTask(task.id);
    setRetryingTaskId(null);
  };

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-zinc-800/50 px-6 py-3">
        <div>
          <h2 className="text-lg font-semibold text-zinc-200">任务看板</h2>
          <p className="mt-0.5 text-xs text-zinc-500">软件工程任务执行闭环 v1</p>
        </div>
        <button
          onClick={fetchTasks}
          disabled={tasksLoading}
          className="btn-ghost px-3 py-1.5 text-xs"
          title="刷新任务"
        >
          {tasksLoading ? "刷新中..." : "刷新"}
        </button>
      </div>

      <div className="border-b border-zinc-800/50 px-6 py-4">
        <div className="flex gap-3 rounded-lg border border-zinc-800 bg-zinc-900/40 p-2">
          <input
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                handleCreate();
              }
            }}
            placeholder="输入一个软件任务目标"
            className="min-w-0 flex-1 bg-transparent px-2 text-sm text-zinc-200 outline-none placeholder:text-zinc-500"
            disabled={creating}
          />
          <button
            type="button"
            onClick={handleCreate}
            disabled={!draft.trim() || creating}
            className="btn-primary px-3 py-2 text-xs"
          >
            {creating ? "创建中..." : "创建任务"}
          </button>
        </div>
      </div>

      {tasksError && (
        <div className="mx-6 mt-4 rounded-lg border border-red-500/20 bg-red-500/10 p-3 text-sm text-red-300">
          {tasksError}
        </div>
      )}

      <div className="grid min-h-0 flex-1 grid-cols-[360px_minmax(0,1fr)] overflow-hidden">
        <aside className="min-h-0 overflow-y-auto border-r border-zinc-800/50 px-4 py-4">
          <div className="space-y-2">
            {tasks.map((task) => (
              <TaskListItem
                key={task.id}
                task={task}
                selected={task.id === selectedTaskId}
                onSelect={() => setSelectedTaskId(task.id)}
              />
            ))}
          </div>
          {!tasksLoading && tasks.length === 0 && (
            <div className="py-12 text-center text-sm text-zinc-500">暂无任务</div>
          )}
        </aside>

        <section className="min-h-0 overflow-y-auto px-6 py-5">
          {selectedTask ? (
            <div className="space-y-6">
              <div className="flex flex-wrap items-start justify-between gap-4">
                <div className="min-w-0">
                  <div className="flex flex-wrap items-center gap-2">
                    <h3 className="text-base font-semibold text-zinc-100">{selectedTask.title}</h3>
                    {statusPill(selectedTask.status)}
                  </div>
                  <p className="mt-2 max-w-3xl text-sm leading-relaxed text-zinc-400">
                    {selectedTask.userGoal}
                  </p>
                </div>
                <div className="flex shrink-0 flex-col items-end gap-3">
                  <div className="text-right text-xs text-zinc-500">
                    <div>创建 {formatTime(selectedTask.createdAt)}</div>
                    <div className="mt-1">更新 {formatTime(selectedTask.updatedAt)}</div>
                  </div>
                  <div className="flex flex-wrap justify-end gap-2">
                    {canRetryTask(selectedTask) && (
                      <button
                        type="button"
                        onClick={() => handleRetry(selectedTask)}
                        disabled={retryingTaskId === selectedTask.id}
                        className="rounded-md border border-primary-500/20 bg-primary-500/10 px-3 py-1.5 text-xs text-primary-200 transition-colors hover:border-primary-400/40 hover:bg-primary-500/15 disabled:cursor-not-allowed disabled:opacity-60"
                      >
                        {retryingTaskId === selectedTask.id ? "重试中..." : "重试任务"}
                      </button>
                    )}
                    {canCancelTask(selectedTask) && (
                      <button
                        type="button"
                        onClick={() => handleCancel(selectedTask)}
                        disabled={cancellingTaskId === selectedTask.id}
                        className="rounded-md border border-red-500/20 bg-red-500/10 px-3 py-1.5 text-xs text-red-200 transition-colors hover:border-red-400/40 hover:bg-red-500/15 disabled:cursor-not-allowed disabled:opacity-60"
                      >
                        {cancellingTaskId === selectedTask.id ? "取消中..." : "取消任务"}
                      </button>
                    )}
                  </div>
                </div>
              </div>

              <div>
                <div className="mb-3 flex items-center justify-between">
                  <h4 className="text-sm font-medium text-zinc-300">步骤</h4>
                  <span className="text-xs text-zinc-500">
                    {progressForTask(selectedTask)}%
                  </span>
                </div>
                <div className="space-y-3">
                  {selectedTask.steps.map((step) => (
                    <StepRow key={step.id} step={step} />
                  ))}
                </div>
              </div>

              {selectedTask.output && (
                <div>
                  <h4 className="mb-2 text-sm font-medium text-zinc-300">输出</h4>
                  <div className="rounded-lg border border-green-500/20 bg-green-500/10 p-3 text-sm text-green-100">
                    {selectedTask.output}
                  </div>
                </div>
              )}

              {selectedTask.artifacts.length > 0 && (
                <div>
                  <h4 className="mb-2 text-sm font-medium text-zinc-300">产物</h4>
                  <div className="space-y-2">
                    {selectedTask.artifacts.map((artifact, index) => (
                      <ArtifactRow key={index} artifact={artifact} />
                    ))}
                  </div>
                </div>
              )}

              <div>
                <div className="mb-3 flex items-center gap-2">
                  <h4 className="text-sm font-medium text-zinc-300">事件</h4>
                  {taskEventsLoading && <span className="text-xs text-zinc-500">更新中...</span>}
                </div>
                <div>
                  {orderedEvents.map((event) => (
                    <EventRow key={event.id} event={event} />
                  ))}
                  {!taskEventsLoading && orderedEvents.length === 0 && (
                    <div className="text-sm text-zinc-500">暂无事件</div>
                  )}
                </div>
              </div>
            </div>
          ) : (
            <div className="flex h-full items-center justify-center text-sm text-zinc-500">
              选择或创建一个任务
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
