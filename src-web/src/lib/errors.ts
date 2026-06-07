import type { ApiErrorPayload } from "@/types";

/** 将 Tauri/Rust/JS 的各种错误统一转换成用户能看懂的中文提示。 */
export function getErrorMessage(err: unknown, fallback = "操作失败"): string {
  if (typeof err === "string") return normalizeKnownError(err, fallback);

  if (err instanceof Error && err.message) {
    return normalizeKnownError(err.message, fallback);
  }

  if (err && typeof err === "object") {
    const payload = err as ApiErrorPayload;
    const raw = payload.message ?? payload.error ?? payload.reason;
    if (raw) return normalizeKnownError(raw, fallback, payload);

    try {
      return normalizeKnownError(JSON.stringify(err), fallback, payload);
    } catch {
      return fallback;
    }
  }

  return fallback;
}

/** 获取可折叠的技术详情。 */
export function getErrorDetail(err: unknown): string | undefined {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.stack ?? err.message;
  if (err && typeof err === "object") {
    const payload = err as ApiErrorPayload;
    if (payload.detail) return payload.detail;
    try {
      return JSON.stringify(err);
    } catch {
      return undefined;
    }
  }
  return undefined;
}

function normalizeKnownError(
  message: string,
  fallback: string,
  payload?: ApiErrorPayload
): string {
  const raw = String(message || "").trim();
  if (!raw) return fallback;

  switch (payload?.code) {
    case "INVALID_ARGUMENT":
    case "AGENT_NOT_FOUND":
    case "AGENT_NOT_SELECTABLE":
    case "ROUTE_FAILED":
      return raw;
    default:
      break;
  }

  const lower = raw.toLowerCase();

  if (lower.includes("agent") && (lower.includes("not found") || lower.includes("未注册"))) {
    return "找不到指定的 AI 成员。它可能已下线或不存在，请刷新团队成员列表，或切换为“自动分配”后重试。";
  }

  if (lower.includes("not selectable") || lower.includes("selectable=false")) {
    return "这个 AI 成员暂不支持直接对话，请切换为“自动分配”或选择协调员。";
  }

  if (lower.includes("offline") || lower.includes("离线")) {
    return "目标 AI 成员当前离线，请选择其他成员或使用自动分配。";
  }

  if (lower.includes("timeout") || lower.includes("timed out") || lower.includes("超时")) {
    return "请求超时。AI 成员可能仍在处理，请稍后重试或查看任务状态。";
  }

  if (lower.includes("connection") || lower.includes("network")) {
    return "应用连接后端失败，请确认桌面应用后端已正常启动。";
  }

  if (lower.includes("api key") || lower.includes("apikey")) {
    return "模型 API Key 配置有误或缺失，请前往设置页检查。";
  }

  return raw;
}
