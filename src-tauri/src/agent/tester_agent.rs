//! 测试 Agent（TesterAgent）
//!
//! 负责整理验证计划和分析前置步骤失败信号。它不直接执行命令；
//! 真实命令仍由受控 command runner 和审批链路处理。

use async_trait::async_trait;

use super::traits::{Agent, AgentMessage, Capability};
use crate::error::AgentError;

/// 测试 Agent。
pub struct TesterAgent {
    count: u64,
}

impl TesterAgent {
    /// 创建新的 TesterAgent。
    pub fn new() -> Self {
        Self { count: 0 }
    }
}

impl Default for TesterAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for TesterAgent {
    fn name(&self) -> &str {
        "Tester"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![Capability::testing()]
    }

    async fn handle_message(&mut self, msg: AgentMessage) -> Result<Vec<AgentMessage>, AgentError> {
        self.count += 1;

        let dependency_results = msg
            .context
            .get("dependencyResults")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let failure_count = dependency_results
            .iter()
            .filter(|value| value_contains_failure_signal(value))
            .count();
        let recommended_commands = collect_recommended_commands(&msg);
        let status = if failure_count > 0 {
            "blocked"
        } else {
            "ready"
        };
        let summary = if failure_count > 0 {
            "测试计划已生成，但前置步骤包含失败信号，需要先返工。"
        } else {
            "测试计划已生成，等待通过受控命令执行验证。"
        };

        let context = serde_json::json!({
            "kind": "TestReport",
            "status": status,
            "summary": summary,
            "recommendedCommands": recommended_commands,
            "dependencyFailureCount": failure_count,
            "requiresControlledCommandRunner": true,
            "directCommandExecutionAllowed": false,
            "testedAt": chrono::Utc::now(),
            "taskId": msg.task_id,
            "stepId": msg.context.get("stepId").cloned().unwrap_or(serde_json::Value::Null),
        });

        let reply = msg
            .reply_to(summary)
            .with_type("test_report")
            .with_context(context);

        Ok(vec![reply])
    }
}

fn collect_recommended_commands(msg: &AgentMessage) -> Vec<serde_json::Value> {
    let mut commands = Vec::new();
    let serialized_context = serde_json::to_string(&msg.context).unwrap_or_default();
    let text = format!("{}\n{}", msg.content, serialized_context);
    for candidate in [
        "cargo test",
        "cargo check",
        "npm test -- --run",
        "npm run build",
        "npm.cmd test -- --run",
        "npm.cmd run build",
    ] {
        if text.contains(candidate)
            && !commands.iter().any(|item: &serde_json::Value| {
                item.get("command").and_then(serde_json::Value::as_str) == Some(candidate)
            })
        {
            commands.push(serde_json::json!({
                "command": candidate,
                "requiresApproval": true,
                "source": "instruction-or-context",
            }));
        }
    }
    commands
}

fn value_contains_failure_signal(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(map) => {
            if value.get("success").and_then(serde_json::Value::as_bool) == Some(false)
                || value.get("passed").and_then(serde_json::Value::as_bool) == Some(false)
            {
                return true;
            }
            if value
                .get("status")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|status| matches!(status, "failed" | "blocked"))
            {
                return true;
            }
            if value.get("error").is_some_and(is_meaningful_error) {
                return true;
            }
            map.values().any(value_contains_failure_signal)
        }
        serde_json::Value::Array(items) => items.iter().any(value_contains_failure_signal),
        _ => false,
    }
}

fn is_meaningful_error(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::String(text) => !text.trim().is_empty(),
        serde_json::Value::Array(items) => !items.is_empty(),
        serde_json::Value::Object(map) => !map.is_empty(),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tester_agent_returns_test_report() {
        let mut agent = TesterAgent::new();
        let msg = AgentMessage::new("Planner", "Tester", "运行 cargo test 和 npm test -- --run");

        let replies = agent.handle_message(msg).await.unwrap();

        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].msg_type, "test_report");
        assert_eq!(replies[0].context["kind"], serde_json::json!("TestReport"));
        assert_eq!(replies[0].context["status"], serde_json::json!("ready"));
        assert_eq!(
            replies[0].context["recommendedCommands"][0]["command"],
            serde_json::json!("cargo test")
        );
    }

    #[tokio::test]
    async fn tester_agent_blocks_on_failed_dependency() {
        let mut agent = TesterAgent::new();
        let msg =
            AgentMessage::new("Coder", "Tester", "整理验证").with_context(serde_json::json!({
                "dependencyResults": [
                    { "agentId": "Coder", "result": { "success": false, "error": "draft failed" } }
                ]
            }));

        let replies = agent.handle_message(msg).await.unwrap();

        assert_eq!(replies[0].context["status"], serde_json::json!("blocked"));
        assert_eq!(
            replies[0].context["dependencyFailureCount"],
            serde_json::json!(1)
        );
    }
}
