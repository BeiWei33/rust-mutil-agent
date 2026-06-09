/**
 * 审批面板
 * 展示高风险动作审批请求，并允许用户通过或拒绝。
 */

import { useEffect, useMemo, useState } from "react";
import { useAgentStore } from "@/store/useAgentStore";
import type {
  ApprovalRequest,
  ApprovalRisk,
  ApprovalStatus,
  ProjectCommandRunResponse,
} from "@/types";

const RISK_STYLE: Record<ApprovalRisk, { label: string; className: string }> = {
  low: { label: "低", className: "border-zinc-600/30 bg-zinc-600/10 text-zinc-300" },
  medium: { label: "中", className: "border-amber-500/20 bg-amber-500/10 text-amber-300" },
  high: { label: "高", className: "border-red-500/20 bg-red-500/10 text-red-300" },
  critical: { label: "严重", className: "border-fuchsia-500/20 bg-fuchsia-500/10 text-fuchsia-300" },
};

const STATUS_STYLE: Record<ApprovalStatus, { label: string; className: string }> = {
  pending: { label: "待审批", className: "border-amber-500/20 bg-amber-500/10 text-amber-300" },
  approved: { label: "已通过", className: "border-emerald-500/20 bg-emerald-500/10 text-emerald-300" },
  rejected: { label: "已拒绝", className: "border-red-500/20 bg-red-500/10 text-red-300" },
  cancelled: { label: "已取消", className: "border-zinc-600/30 bg-zinc-600/10 text-zinc-400" },
};

function formatTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

function compactJson(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function statusPill(status: ApprovalStatus) {
  const style = STATUS_STYLE[status];
  return (
    <span className={`rounded-md border px-2 py-0.5 text-[11px] ${style.className}`}>
      {style.label}
    </span>
  );
}

function riskPill(risk: ApprovalRisk) {
  const style = RISK_STYLE[risk];
  return (
    <span className={`rounded-md border px-2 py-0.5 text-[11px] ${style.className}`}>
      风险 {style.label}
    </span>
  );
}

function isProjectCommandApproval(approval: ApprovalRequest): boolean {
  return approval.actionType === "runtime.runProjectCommand";
}

function runStatusLabel(run: ProjectCommandRunResponse): string {
  if (run.timedOut) return "超时";
  return run.success ? "执行通过" : "执行失败";
}

function ApprovalItem({
  approval,
  loading,
  executionLoading,
  executedRun,
  onApprove,
  onReject,
  onExecute,
}: {
  approval: ApprovalRequest;
  loading: boolean;
  executionLoading: boolean;
  executedRun?: ProjectCommandRunResponse;
  onApprove: () => void;
  onReject: () => void;
  onExecute: () => void;
}) {
  const pending = approval.status === "pending";
  const canExecuteCommand =
    approval.status === "approved" && isProjectCommandApproval(approval) && !executedRun;

  return (
    <article className="rounded-lg border border-zinc-800 bg-zinc-900/40 p-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="text-sm font-semibold text-zinc-100">{approval.title}</h3>
            {statusPill(approval.status)}
            {riskPill(approval.risk)}
          </div>
          <p className="mt-2 text-sm leading-relaxed text-zinc-400">{approval.reason}</p>
        </div>
        {(pending || canExecuteCommand || executedRun) && (
          <div className="flex shrink-0 gap-2">
            {pending && (
              <>
                <button
                  type="button"
                  onClick={onReject}
                  disabled={loading}
                  className="rounded-md border border-red-500/20 bg-red-500/10 px-3 py-1.5 text-xs text-red-200 transition-colors hover:border-red-400/40 hover:bg-red-500/15 disabled:cursor-not-allowed disabled:opacity-60"
                >
                  {loading ? "处理中..." : "拒绝"}
                </button>
                <button
                  type="button"
                  onClick={onApprove}
                  disabled={loading}
                  className="rounded-md border border-emerald-500/20 bg-emerald-500/10 px-3 py-1.5 text-xs text-emerald-200 transition-colors hover:border-emerald-400/40 hover:bg-emerald-500/15 disabled:cursor-not-allowed disabled:opacity-60"
                >
                  {loading ? "处理中..." : "通过"}
                </button>
              </>
            )}
            {canExecuteCommand && (
              <button
                type="button"
                onClick={onExecute}
                disabled={executionLoading}
                className="rounded-md border border-primary-500/20 bg-primary-500/10 px-3 py-1.5 text-xs text-primary-200 transition-colors hover:border-primary-400/40 hover:bg-primary-500/15 disabled:cursor-not-allowed disabled:opacity-60"
              >
                {executionLoading ? "执行中..." : "执行"}
              </button>
            )}
            {executedRun && (
              <span
                className={`rounded-md border px-3 py-1.5 text-xs ${
                  executedRun.success
                    ? "border-emerald-500/20 bg-emerald-500/10 text-emerald-200"
                    : "border-amber-500/20 bg-amber-500/10 text-amber-200"
                }`}
              >
                {runStatusLabel(executedRun)}
              </span>
            )}
          </div>
        )}
      </div>

      <div className="mt-3 grid gap-2 text-[11px] text-zinc-500 sm:grid-cols-2">
        <div>动作：{approval.actionType}</div>
        <div>请求方：{approval.requestedBy}</div>
        {approval.taskId && <div>任务：{approval.taskId}</div>}
        {approval.stepId && <div>步骤：{approval.stepId}</div>}
        <div>创建：{formatTime(approval.createdAt)}</div>
        {approval.decidedAt && <div>决策：{formatTime(approval.decidedAt)}</div>}
      </div>

      <pre className="mt-3 max-h-48 overflow-auto whitespace-pre-wrap rounded-md bg-zinc-950 p-3 text-[11px] leading-relaxed text-zinc-300">
        {compactJson(approval.actionPayload)}
      </pre>

      {approval.decisionNote && (
        <div className="mt-3 rounded-md border border-zinc-800 bg-zinc-950 px-3 py-2 text-xs text-zinc-300">
          {approval.decisionNote}
        </div>
      )}

      {executedRun && (
        <div className="mt-3 rounded-md border border-zinc-800 bg-zinc-950 px-3 py-2 text-xs text-zinc-300">
          命令审计：{executedRun.command} · {executedRun.durationMs}ms
          {executedRun.exitCode !== null &&
            executedRun.exitCode !== undefined &&
            ` · exit ${executedRun.exitCode}`}
        </div>
      )}
    </article>
  );
}

export default function ApprovalPanel() {
  const approvals = useAgentStore((s) => s.approvals);
  const approvalsLoading = useAgentStore((s) => s.approvalsLoading);
  const approvalsError = useAgentStore((s) => s.approvalsError);
  const approvalDecisionLoadingId = useAgentStore((s) => s.approvalDecisionLoadingId);
  const approvalExecutionLoadingId = useAgentStore((s) => s.approvalExecutionLoadingId);
  const commandRuns = useAgentStore((s) => s.commandRuns);
  const fetchApprovals = useAgentStore((s) => s.fetchApprovals);
  const decideApproval = useAgentStore((s) => s.decideApproval);
  const runApprovedCommand = useAgentStore((s) => s.runApprovedCommand);
  const [filter, setFilter] = useState<ApprovalStatus | undefined>("pending");

  useEffect(() => {
    fetchApprovals(filter);
  }, [fetchApprovals, filter]);

  const pendingCount = useMemo(
    () => approvals.filter((approval) => approval.status === "pending").length,
    [approvals]
  );

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-zinc-800/50 px-6 py-3">
        <div>
          <h2 className="text-lg font-semibold text-zinc-200">审批</h2>
          <p className="mt-0.5 text-xs text-zinc-500">
            {filter === "pending" ? `${pendingCount} 个待处理` : `${approvals.length} 条记录`}
          </p>
        </div>
        <button
          type="button"
          onClick={() => fetchApprovals(filter)}
          disabled={approvalsLoading}
          className="btn-ghost px-3 py-1.5 text-xs"
          title="刷新审批"
        >
          {approvalsLoading ? "刷新中..." : "刷新"}
        </button>
      </div>

      <div className="border-b border-zinc-800/50 px-6 py-3">
        <div className="inline-flex rounded-lg border border-zinc-800 bg-zinc-900/40 p-1">
          <button
            type="button"
            onClick={() => setFilter("pending")}
            className={`rounded-md px-3 py-1.5 text-xs ${
              filter === "pending" ? "bg-primary-500/15 text-primary-200" : "text-zinc-400"
            }`}
          >
            待审批
          </button>
          <button
            type="button"
            onClick={() => setFilter(undefined)}
            className={`rounded-md px-3 py-1.5 text-xs ${
              filter === undefined ? "bg-primary-500/15 text-primary-200" : "text-zinc-400"
            }`}
          >
            全部
          </button>
        </div>
      </div>

      {approvalsError && (
        <div className="mx-6 mt-4 rounded-lg border border-red-500/20 bg-red-500/10 p-3 text-sm text-red-300">
          {approvalsError}
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
        {approvals.length > 0 ? (
          <div className="space-y-3">
            {approvals.map((approval) => (
              <ApprovalItem
                key={approval.id}
                approval={approval}
                loading={approvalDecisionLoadingId === approval.id}
                executionLoading={approvalExecutionLoadingId === approval.id}
                executedRun={commandRuns.find((run) => run.approvalId === approval.id)}
                onApprove={() => decideApproval(approval.id, true)}
                onReject={() => decideApproval(approval.id, false)}
                onExecute={() => runApprovedCommand(approval.id)}
              />
            ))}
          </div>
        ) : (
          <div className="flex h-full items-center justify-center text-sm text-zinc-500">
            {approvalsLoading ? "加载中..." : "暂无审批请求"}
          </div>
        )}
      </div>
    </div>
  );
}
