//! Structured Agent actions.
//!
//! The current runtime still exchanges `AgentMessage` values. These types define
//! the next compatibility layer: Agents can gradually move from free-form text
//! replies toward schema-checked actions without changing every implementation
//! at once.

use serde::{Deserialize, Serialize};

/// Risk level for an action that may require approval.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// A structured action proposed by an Agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum AgentAction {
    Think {
        summary: String,
    },
    ToolCall {
        name: String,
        args: serde_json::Value,
    },
    PatchProposal {
        patch_id: String,
        summary: String,
    },
    Handoff {
        target_agent: String,
        reason: String,
    },
    AskApproval {
        reason: String,
        risk: RiskLevel,
    },
    Finish {
        summary: String,
    },
    Fail {
        reason: String,
        retryable: bool,
    },
}

/// The structured output of one Agent turn.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentOutcome {
    pub actions: Vec<AgentAction>,
    pub observations: Vec<String>,
    pub confidence: f32,
}

impl AgentOutcome {
    pub fn finish(summary: impl Into<String>) -> Self {
        Self {
            actions: vec![AgentAction::Finish {
                summary: summary.into(),
            }],
            observations: Vec::new(),
            confidence: 1.0,
        }
    }
}
