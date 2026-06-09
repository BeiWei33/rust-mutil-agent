//! 审查 Agent（ReviewAgent）
//!
//! 负责把执行结果整理成可追踪的 ReviewReport。当前实现保持确定性和只读，
//! 不直接修改文件，也不绕过既有 patch/approval/verification 闭环。

use async_trait::async_trait;

use super::traits::{Agent, AgentMessage, Capability};
use crate::error::AgentError;

/// 审查 Agent。
pub struct ReviewAgent {
    count: u64,
}

impl ReviewAgent {
    /// 创建新的 ReviewAgent。
    pub fn new() -> Self {
        Self { count: 0 }
    }
}

impl Default for ReviewAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for ReviewAgent {
    fn name(&self) -> &str {
        "Review"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![Capability::review()]
    }

    async fn handle_message(&mut self, msg: AgentMessage) -> Result<Vec<AgentMessage>, AgentError> {
        self.count += 1;

        let findings = review_findings(&msg);
        let passed = findings
            .iter()
            .all(|finding| !matches!(finding.severity, "high" | "critical"));
        let summary = if passed {
            "审查通过，未发现阻断性风险。"
        } else {
            "审查未通过，存在需要返工的高风险问题。"
        };

        let context = serde_json::json!({
            "kind": "ReviewReport",
            "passed": passed,
            "summary": summary,
            "findingCount": findings.len(),
            "findings": findings.iter().map(ReviewFinding::to_json).collect::<Vec<_>>(),
            "diffReviewed": review_has_real_diff(&msg.context),
            "reviewedAt": chrono::Utc::now(),
            "taskId": msg.task_id,
            "stepId": msg.context.get("stepId").cloned().unwrap_or(serde_json::Value::Null),
            "dependencyCount": msg.context
                .get("dependencyResults")
                .and_then(serde_json::Value::as_array)
                .map(|items| items.len())
                .unwrap_or(0),
        });

        let reply = msg
            .reply_to(summary)
            .with_type("review_report")
            .with_context(context);

        Ok(vec![reply])
    }
}

#[derive(Debug, Clone)]
struct ReviewFinding {
    severity: &'static str,
    title: &'static str,
    detail: String,
    recommendation: &'static str,
}

impl ReviewFinding {
    fn new(
        severity: &'static str,
        title: &'static str,
        detail: impl Into<String>,
        recommendation: &'static str,
    ) -> Self {
        Self {
            severity,
            title,
            detail: detail.into(),
            recommendation,
        }
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "severity": self.severity,
            "title": self.title,
            "detail": self.detail,
            "recommendation": self.recommendation,
        })
    }
}

fn review_findings(msg: &AgentMessage) -> Vec<ReviewFinding> {
    let mut findings = Vec::new();
    let dependency_results = msg
        .context
        .get("dependencyResults")
        .and_then(serde_json::Value::as_array);

    if dependency_results.map(Vec::is_empty).unwrap_or(true) {
        findings.push(ReviewFinding::new(
            "low",
            "缺少前置结果",
            "ReviewAgent 没有收到 dependencyResults，审查只能基于当前指令文本进行。",
            "在计划中让 Review 依赖 Executor 或 Tool 步骤，以便审查具体输出。",
        ));
    }

    if let Some(items) = dependency_results {
        for item in items {
            let agent = item
                .get("agentId")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let result = item.get("result").unwrap_or(&serde_json::Value::Null);
            if value_contains_false_success(result) || value_contains_error(result) {
                findings.push(ReviewFinding::new(
                    "high",
                    "前置步骤存在失败信号",
                    format!("{agent} 的结果包含 success=false 或 error 字段。"),
                    "先修复失败步骤，再重新进入 Review。",
                ));
            }
        }
    }

    let text = review_text(msg);
    if text.contains("TODO") || text.contains("todo") {
        findings.push(ReviewFinding::new(
            "medium",
            "输出包含未收束事项",
            "审查上下文中出现 TODO 标记。",
            "在任务完成前明确处理、转交或记录 TODO。",
        ));
    }
    if text.contains("不直接修改文件") || text.contains("模拟执行") {
        findings.push(ReviewFinding::new(
            "low",
            "当前结果偏方案或模拟",
            "前置执行结果表明本轮尚未直接产生代码 diff。",
            "需要实际修改时继续使用 patch proposal、审批和验证闭环。",
        ));
    }
    if msg
        .context
        .get("requiresRealDiffReview")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
        && !review_has_real_diff(&msg.context)
    {
        findings.push(ReviewFinding::new(
            "high",
            "缺少真实 diff",
            "审查上下文要求基于真实 unified diff，但没有发现补丁 diff。",
            "先创建 patch proposal，确保 Review 依赖包含 unifiedDiff 的补丁审批或应用 artifact。",
        ));
    }

    findings
}

fn review_has_real_diff(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(map) => {
            if map
                .get("unifiedDiff")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|diff| diff.contains("diff --git"))
            {
                return true;
            }
            map.values().any(review_has_real_diff)
        }
        serde_json::Value::Array(items) => items.iter().any(review_has_real_diff),
        _ => false,
    }
}

fn value_contains_false_success(value: &serde_json::Value) -> bool {
    value.get("success").and_then(serde_json::Value::as_bool) == Some(false)
        || value
            .pointer("/context/success")
            .and_then(serde_json::Value::as_bool)
            == Some(false)
}

fn value_contains_error(value: &serde_json::Value) -> bool {
    is_meaningful_error(value.get("error")) || is_meaningful_error(value.pointer("/context/error"))
}

fn is_meaningful_error(value: Option<&serde_json::Value>) -> bool {
    match value {
        Some(serde_json::Value::Null) | None => false,
        Some(serde_json::Value::String(value)) => !value.trim().is_empty(),
        Some(serde_json::Value::Array(items)) => !items.is_empty(),
        Some(serde_json::Value::Object(items)) => !items.is_empty(),
        Some(_) => true,
    }
}

fn review_text(msg: &AgentMessage) -> String {
    let context = serde_json::to_string(&msg.context).unwrap_or_default();
    format!("{}\n{}", msg.content, context)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn review_agent_returns_report() {
        let mut agent = ReviewAgent::new();
        let msg = AgentMessage::new("Planner", "Review", "审查执行结果").with_context(
            serde_json::json!({
                "dependencyResults": [
                    {
                        "agentId": "Executor",
                        "result": {
                            "success": true,
                            "output": "已执行"
                        }
                    }
                ]
            }),
        );

        let replies = agent.handle_message(msg).await.unwrap();

        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].msg_type, "review_report");
        assert_eq!(
            replies[0].context["kind"],
            serde_json::json!("ReviewReport")
        );
        assert_eq!(replies[0].context["passed"], serde_json::json!(true));
    }

    #[tokio::test]
    async fn review_agent_flags_failed_dependency() {
        let mut agent = ReviewAgent::new();
        let msg = AgentMessage::new("Planner", "Review", "审查执行结果").with_context(
            serde_json::json!({
                "dependencyResults": [
                    {
                        "agentId": "Executor",
                        "result": {
                            "success": false,
                            "error": "cargo test failed"
                        }
                    }
                ]
            }),
        );

        let replies = agent.handle_message(msg).await.unwrap();

        assert_eq!(replies[0].context["passed"], serde_json::json!(false));
        assert_eq!(
            replies[0].context["findings"][0]["severity"],
            serde_json::json!("high")
        );
    }

    #[tokio::test]
    async fn review_agent_marks_real_diff_reviewed() {
        let mut agent = ReviewAgent::new();
        let msg =
            AgentMessage::new("Planner", "Review", "审查补丁").with_context(serde_json::json!({
                "dependencyResults": [
                    {
                        "agentId": "Coder",
                        "result": {
                            "unifiedDiff": "diff --git a/README.md b/README.md\n"
                        }
                    }
                ],
                "requiresRealDiffReview": true
            }));

        let replies = agent.handle_message(msg).await.unwrap();

        assert_eq!(replies[0].context["passed"], serde_json::json!(true));
        assert_eq!(replies[0].context["diffReviewed"], serde_json::json!(true));
    }

    #[tokio::test]
    async fn review_agent_fails_when_required_diff_missing() {
        let mut agent = ReviewAgent::new();
        let msg =
            AgentMessage::new("Planner", "Review", "审查补丁").with_context(serde_json::json!({
                "dependencyResults": [
                    { "agentId": "Coder", "result": { "summary": "draft only" } }
                ],
                "requiresRealDiffReview": true
            }));

        let replies = agent.handle_message(msg).await.unwrap();

        assert_eq!(replies[0].context["passed"], serde_json::json!(false));
        assert_eq!(replies[0].context["diffReviewed"], serde_json::json!(false));
    }

    #[test]
    fn review_agent_declares_review_capability() {
        let agent = ReviewAgent::new();
        let caps = agent.capabilities();

        assert_eq!(agent.name(), "Review");
        assert_eq!(caps[0].name, "review");
    }
}
