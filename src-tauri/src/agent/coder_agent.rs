//! 编码 Agent（CoderAgent）
//!
//! 负责把需求、只读检索结果和返工建议整理成受控补丁草案。
//! 它不直接写文件，也不绕过 patch proposal / approval / verification 闭环。

use async_trait::async_trait;

use super::traits::{Agent, AgentMessage, Capability};
use crate::error::AgentError;

/// 编码 Agent。
pub struct CoderAgent {
    count: u64,
}

impl CoderAgent {
    /// 创建新的 CoderAgent。
    pub fn new() -> Self {
        Self { count: 0 }
    }
}

impl Default for CoderAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for CoderAgent {
    fn name(&self) -> &str {
        "Coder"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![Capability::coding()]
    }

    async fn handle_message(&mut self, msg: AgentMessage) -> Result<Vec<AgentMessage>, AgentError> {
        self.count += 1;

        let rework_suggestions = msg
            .context
            .get("reworkSuggestions")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let dependency_results = msg
            .context
            .get("dependencyResults")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let target_files = collect_target_files(&dependency_results, &rework_suggestions);
        let summary = if rework_suggestions.is_empty() {
            "已生成补丁草案：等待用户把草案转换为 patch proposal。"
        } else {
            "已根据验证失败上下文生成返工补丁草案。"
        };

        let context = serde_json::json!({
            "kind": "PatchProposalDraft",
            "summary": summarize_text(&msg.content, 160),
            "targetFiles": target_files,
            "suggestedChanges": suggested_changes(&target_files, &rework_suggestions, &msg.content),
            "reworkSuggestions": rework_suggestions,
            "dependencyCount": dependency_results.len(),
            "requiresPatchProposal": true,
            "directFileWritesAllowed": false,
            "nextAction": "将草案转换为 create_patch_proposal 请求，并继续走审批、应用和验证闭环。",
            "draftedAt": chrono::Utc::now(),
            "taskId": msg.task_id,
            "stepId": msg.context.get("stepId").cloned().unwrap_or(serde_json::Value::Null),
        });

        let reply = msg
            .reply_to(summary)
            .with_type("patch_proposal_draft")
            .with_context(context);

        Ok(vec![reply])
    }
}

fn collect_target_files(
    dependency_results: &[serde_json::Value],
    rework_suggestions: &[serde_json::Value],
) -> Vec<String> {
    let mut files = Vec::new();
    for value in dependency_results.iter().chain(rework_suggestions.iter()) {
        collect_string_fields(value, &mut files);
    }
    files.truncate(12);
    files
}

fn collect_string_fields(value: &serde_json::Value, files: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                if matches!(key.as_str(), "path" | "file" | "filePath") {
                    if let Some(path) = value.as_str().map(str::trim).filter(|path| {
                        !path.is_empty() && !files.iter().any(|existing| existing == path)
                    }) {
                        files.push(path.to_string());
                    }
                }
                collect_string_fields(value, files);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_string_fields(item, files);
            }
        }
        _ => {}
    }
}

fn suggested_changes(
    target_files: &[String],
    rework_suggestions: &[serde_json::Value],
    instruction: &str,
) -> Vec<serde_json::Value> {
    let reason = if rework_suggestions.is_empty() {
        "根据当前实现目标补充最小代码改动。"
    } else {
        "根据验证失败命令、stderr 摘要和回滚状态修复问题。"
    };

    if target_files.is_empty() {
        return vec![serde_json::json!({
            "path": serde_json::Value::Null,
            "reason": reason,
            "instruction": summarize_text(instruction, 240),
            "requiresOldContent": true,
            "requiresNewContent": true,
        })];
    }

    target_files
        .iter()
        .take(6)
        .map(|path| {
            serde_json::json!({
                "path": path,
                "reason": reason,
                "instruction": summarize_text(instruction, 240),
                "requiresOldContent": true,
                "requiresNewContent": true,
            })
        })
        .collect()
}

fn summarize_text(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = normalized.chars();
    let mut preview = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        preview.push_str("...");
    }
    preview
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn coder_agent_returns_patch_proposal_draft() {
        let mut agent = CoderAgent::new();
        let msg = AgentMessage::new("Planner", "Coder", "修改 TaskBoard").with_context(
            serde_json::json!({
                "dependencyResults": [
                    {
                        "agentId": "Tool",
                        "result": {
                            "readFiles": [{ "path": "src-web/src/components/TaskBoard.tsx" }]
                        }
                    }
                ]
            }),
        );

        let replies = agent.handle_message(msg).await.unwrap();

        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].msg_type, "patch_proposal_draft");
        assert_eq!(
            replies[0].context["kind"],
            serde_json::json!("PatchProposalDraft")
        );
        assert_eq!(
            replies[0].context["targetFiles"][0],
            serde_json::json!("src-web/src/components/TaskBoard.tsx")
        );
        assert_eq!(
            replies[0].context["directFileWritesAllowed"],
            serde_json::json!(false)
        );
    }

    #[tokio::test]
    async fn coder_agent_keeps_rework_suggestions() {
        let mut agent = CoderAgent::new();
        let msg =
            AgentMessage::new("Review", "Coder", "修复验证失败").with_context(serde_json::json!({
                "reworkSuggestions": [
                    {
                        "patchId": "patch-1",
                        "failedCommands": [{ "command": "cargo test" }]
                    }
                ]
            }));

        let replies = agent.handle_message(msg).await.unwrap();

        assert_eq!(
            replies[0].context["reworkSuggestions"][0]["patchId"],
            serde_json::json!("patch-1")
        );
        assert!(replies[0].content.contains("返工"));
    }
}
