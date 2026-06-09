//! 演进 Agent（EvolutionAgent）
//!
//! 负责把任务过程总结成 EvolutionNote。它只提出建议，不自动改写核心规则、
//! prompt 或工具权限。

use async_trait::async_trait;

use super::traits::{Agent, AgentMessage, Capability};
use crate::error::AgentError;

/// 演进建议 Agent。
pub struct EvolutionAgent {
    count: u64,
}

impl EvolutionAgent {
    /// 创建新的 EvolutionAgent。
    pub fn new() -> Self {
        Self { count: 0 }
    }
}

impl Default for EvolutionAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for EvolutionAgent {
    fn name(&self) -> &str {
        "Evolution"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![Capability::evolution(), Capability::memory()]
    }

    async fn handle_message(&mut self, msg: AgentMessage) -> Result<Vec<AgentMessage>, AgentError> {
        self.count += 1;

        let observations = evolution_observations(&msg);
        let recommendations = evolution_recommendations(&msg, observations.is_empty());
        let summary = if observations.is_empty() {
            "已生成演进记录：当前上下文不足，建议补充前置步骤结果。"
        } else {
            "已生成演进记录：任务经验和后续建议已整理。"
        };

        let context = serde_json::json!({
            "kind": "EvolutionNote",
            "summary": summary,
            "observations": observations,
            "recommendations": recommendations,
            "tags": ["EvolutionNote", "task-learning"],
            "generatedAt": chrono::Utc::now(),
            "taskId": msg.task_id,
            "stepId": msg.context.get("stepId").cloned().unwrap_or(serde_json::Value::Null),
            "sourceStepCount": msg.context
                .get("dependencyResults")
                .and_then(serde_json::Value::as_array)
                .map(|items| items.len())
                .unwrap_or(0),
            "requiresUserAcceptance": true,
        });

        let reply = msg
            .reply_to(summary)
            .with_type("evolution_note")
            .with_context(context);

        Ok(vec![reply])
    }
}

fn evolution_observations(msg: &AgentMessage) -> Vec<String> {
    let Some(items) = msg
        .context
        .get("dependencyResults")
        .and_then(serde_json::Value::as_array)
    else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(|item| {
            let agent = item
                .get("agentId")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let instruction = item
                .get("instruction")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("未记录指令");
            let result = item.get("result").unwrap_or(&serde_json::Value::Null);
            let signal = result_summary_signal(result)?;
            Some(format!("{agent}: {instruction} -> {signal}"))
        })
        .take(8)
        .collect()
}

fn evolution_recommendations(msg: &AgentMessage, no_observations: bool) -> Vec<String> {
    let mut recommendations = Vec::new();

    if no_observations {
        recommendations.push(
            "让 Evolution 步骤依赖 Review、Executor 或 Tool，以便沉淀真实任务经验。".to_string(),
        );
    }

    let text = serde_json::to_string(&msg.context).unwrap_or_default();
    if text.contains("\"passed\":false") || text.contains("验证失败") || text.contains("failed")
    {
        recommendations
            .push("把失败命令、错误摘要和返工建议保留到后续 Planner/Review 上下文。".to_string());
    } else {
        recommendations
            .push("将可复用结论保存为 ProjectFact，并在相似任务规划时检索。".to_string());
    }

    recommendations.push("所有策略改动都需要用户显式接受后再影响后续任务。".to_string());
    recommendations
}

fn result_summary_signal(result: &serde_json::Value) -> Option<String> {
    if result.is_null() {
        return None;
    }

    if result.get("passed").and_then(serde_json::Value::as_bool) == Some(false) {
        return Some("Review 未通过".to_string());
    }
    if result.get("passed").and_then(serde_json::Value::as_bool) == Some(true) {
        return Some("Review 通过".to_string());
    }
    if let Some(summary) = result.get("summary").and_then(serde_json::Value::as_str) {
        return Some(summary.to_string());
    }
    if let Some(content) = result.get("content").and_then(serde_json::Value::as_str) {
        return Some(first_line(content));
    }
    if let Some(output) = result.get("output").and_then(serde_json::Value::as_str) {
        return Some(first_line(output));
    }
    if result.get("success").and_then(serde_json::Value::as_bool) == Some(true) {
        return Some("执行成功".to_string());
    }
    if result.get("success").and_then(serde_json::Value::as_bool) == Some(false) {
        return Some("执行失败".to_string());
    }

    Some("已产生结构化结果".to_string())
}

fn first_line(value: &str) -> String {
    value
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .unwrap_or("无摘要")
        .chars()
        .take(160)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn evolution_agent_returns_note() {
        let mut agent = EvolutionAgent::new();
        let msg = AgentMessage::new("Planner", "Evolution", "总结任务经验").with_context(
            serde_json::json!({
                "dependencyResults": [
                    {
                        "agentId": "Review",
                        "instruction": "审查执行结果",
                        "result": {
                            "passed": true,
                            "summary": "审查通过"
                        }
                    }
                ]
            }),
        );

        let replies = agent.handle_message(msg).await.unwrap();

        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].msg_type, "evolution_note");
        assert_eq!(
            replies[0].context["kind"],
            serde_json::json!("EvolutionNote")
        );
        assert_eq!(replies[0].context["sourceStepCount"], serde_json::json!(1));
    }

    #[tokio::test]
    async fn evolution_agent_marks_note_as_user_acceptance_required() {
        let mut agent = EvolutionAgent::new();
        let msg = AgentMessage::new("Planner", "Evolution", "总结任务经验");

        let replies = agent.handle_message(msg).await.unwrap();

        assert_eq!(
            replies[0].context["requiresUserAcceptance"],
            serde_json::json!(true)
        );
        assert!(replies[0].context["recommendations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_str().unwrap_or_default().contains("显式接受")));
    }

    #[test]
    fn evolution_agent_declares_evolution_capability() {
        let agent = EvolutionAgent::new();
        let caps = agent.capabilities();

        assert_eq!(agent.name(), "Evolution");
        assert!(caps.iter().any(|cap| cap.name == "evolution"));
    }
}
