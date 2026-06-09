//! 任务规划 Agent（PlannerAgent）
//!
//! 负责将用户的自然语言目标分解为结构化的执行计划。
//! 计划由多个步骤（Step）组成，每个步骤指定了负责的 Agent 和具体指令。
//!
//! # 工作流程
//! 1. 接收用户目标（通过 AgentMessage）
//! 2. 分析目标复杂度，生成执行计划
//! 3. 将计划步骤封装为消息，发布到总线供其他 Agent 执行
//!
//! # 计划结构
//! ```text
//! TaskPlan {
//!     task_id: "uuid",
//!     goal: "整理今日热点新闻",
//!     steps: [
//!         Step { agent: "Tool", instruction: "搜索今日新闻" },
//!         Step { agent: "Executor", instruction: "总结每条新闻" },
//!         Step { agent: "Echo", instruction: "生成最终报告" },
//!     ]
//! }
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use super::traits::{Agent, AgentMessage, Capability};
use crate::error::AgentError;
use crate::llm::{ChatCompletionRequest, ChatMessage, LLMClient, ResponseFormat, Role};

const PLANNER_ALLOWED_AGENTS: &[&str] = &["Planner", "Tool", "Executor", "Memory", "Echo"];
const DEFAULT_PLANNER_LLM_RETRIES: usize = 1;

// ============================================================
// 计划数据结构
// ============================================================

/// 执行计划的单个步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    /// 步骤 ID（在计划内唯一）
    pub step_id: String,

    /// 步骤序号（从 1 开始）
    pub order: u32,

    /// 负责执行的 Agent 名称
    pub agent: String,

    /// 该步骤的具体指令
    pub instruction: String,

    /// 依赖的前置步骤 ID 列表（用于 DAG 图结构，预留）
    pub depends_on: Vec<String>,

    /// 步骤状态
    pub status: StepStatus,
}

/// 步骤执行状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StepStatus {
    /// 等待执行
    Pending,

    /// 执行中
    Running,

    /// 执行完成
    Completed,

    /// 执行失败
    Failed(String),
}

/// 完整的任务执行计划
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    /// 关联的任务 ID
    pub task_id: String,

    /// 用户原始目标描述
    pub goal: String,

    /// 执行步骤列表
    pub steps: Vec<PlanStep>,

    /// 计划创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl TaskPlan {
    /// 根据目标生成简单线性计划（演示用）
    ///
    /// 实际生产环境中，这里应调用 LLM 来智能分解任务。
    pub fn from_goal(goal: &str) -> Self {
        Self::from_goal_with_id(goal, Uuid::new_v4().to_string())
    }

    /// 根据目标和外部任务 ID 生成简单线性计划。
    pub fn from_goal_with_id(goal: &str, task_id: impl Into<String>) -> Self {
        let task_id = task_id.into();

        // 演示：根据关键词简单推断所需步骤
        let steps = if goal.contains("搜索") || goal.contains("查找") || goal.contains("新闻")
        {
            vec![
                PlanStep {
                    step_id: format!("{task_id}-1"),
                    order: 1,
                    agent: "Tool".to_string(),
                    instruction: format!("搜索相关信息: {goal}"),
                    depends_on: vec![],
                    status: StepStatus::Pending,
                },
                PlanStep {
                    step_id: format!("{task_id}-2"),
                    order: 2,
                    agent: "Executor".to_string(),
                    instruction: format!("整理和分析搜索结果: {goal}"),
                    depends_on: vec![format!("{task_id}-1")],
                    status: StepStatus::Pending,
                },
                PlanStep {
                    step_id: format!("{task_id}-3"),
                    order: 3,
                    agent: "Memory".to_string(),
                    instruction: format!("存储结果到知识库: {goal}"),
                    depends_on: vec![format!("{task_id}-2")],
                    status: StepStatus::Pending,
                },
            ]
        } else if goal.contains("代码") || goal.contains("编程") || goal.contains("写") {
            vec![
                PlanStep {
                    step_id: format!("{task_id}-1"),
                    order: 1,
                    agent: "Planner".to_string(),
                    instruction: format!("分析编码需求: {goal}"),
                    depends_on: vec![],
                    status: StepStatus::Pending,
                },
                PlanStep {
                    step_id: format!("{task_id}-2"),
                    order: 2,
                    agent: "Executor".to_string(),
                    instruction: format!("编写代码: {goal}"),
                    depends_on: vec![format!("{task_id}-1")],
                    status: StepStatus::Pending,
                },
            ]
        } else {
            // 默认单步计划
            vec![PlanStep {
                step_id: format!("{task_id}-1"),
                order: 1,
                agent: "Echo".to_string(),
                instruction: format!("处理用户请求: {goal}"),
                depends_on: vec![],
                status: StepStatus::Pending,
            }]
        };

        Self {
            task_id,
            goal: goal.to_string(),
            steps,
            created_at: chrono::Utc::now(),
        }
    }

    /// 从 LLM JSON 输出解析结构化计划。
    ///
    /// 该入口只做解析和校验，不直接调用 LLM；调用方可在真实 LLM 接入后
    /// 先用 JSON mode 取得文本，再通过这里转换为内部 `TaskPlan`。
    pub fn from_llm_json(
        goal: &str,
        task_id: impl Into<String>,
        raw_json: &str,
    ) -> Result<Self, AgentError> {
        let task_id = task_id.into();
        let json = extract_json_object(raw_json)?;
        let envelope: LlmTaskPlanEnvelope = serde_json::from_str(&json)
            .map_err(|err| AgentError::MessageFormat(format!("Planner JSON 解析失败: {err}")))?;

        if envelope.steps.is_empty() {
            return Err(AgentError::MessageFormat(
                "Planner JSON 至少需要包含一个步骤".to_string(),
            ));
        }
        if envelope.steps.len() > 12 {
            return Err(AgentError::MessageFormat(
                "Planner JSON 步骤数量不能超过 12".to_string(),
            ));
        }

        let mut original_to_internal = HashMap::new();
        let mut seen_original_ids = HashSet::new();
        for (index, step) in envelope.steps.iter().enumerate() {
            let original_id = step.original_id(index + 1);
            if !seen_original_ids.insert(original_id.clone()) {
                return Err(AgentError::MessageFormat(format!(
                    "Planner JSON 步骤 ID 重复: {original_id}"
                )));
            }
            original_to_internal.insert(original_id, format!("{task_id}-{}", index + 1));
        }

        let mut steps = Vec::new();
        let mut known_internal_ids = HashSet::new();
        for (index, step) in envelope.steps.into_iter().enumerate() {
            let order = (index + 1) as u32;
            let original_id = step.original_id(index + 1);
            let step_id = format!("{task_id}-{}", index + 1);
            let agent = normalize_agent_id(&step.agent).ok_or_else(|| {
                AgentError::MessageFormat(format!(
                    "Planner JSON 包含不允许的 Agent: {}",
                    step.agent
                ))
            })?;
            let instruction = step.instruction.trim().to_string();
            if instruction.is_empty() {
                return Err(AgentError::MessageFormat(format!(
                    "Planner JSON 步骤 {original_id} 的 instruction 不能为空"
                )));
            }

            let depends_on = resolve_depends_on(
                &step.depends_on,
                &original_to_internal,
                &known_internal_ids,
                &original_id,
            )?;

            known_internal_ids.insert(step_id.clone());
            steps.push(PlanStep {
                step_id,
                order,
                agent,
                instruction,
                depends_on,
                status: StepStatus::Pending,
            });
        }

        let plan_goal = envelope
            .goal
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| goal.to_string());

        Ok(Self {
            task_id,
            goal: plan_goal,
            steps,
            created_at: chrono::Utc::now(),
        })
    }

    /// 根据目标、任务 ID 和项目上下文生成计划。
    pub fn from_goal_with_context(
        goal: &str,
        task_id: impl Into<String>,
        context: &serde_json::Value,
    ) -> Self {
        let task_id = task_id.into();
        if is_software_goal(goal) {
            if let Some(project_context) = context.get("projectPlanningContext") {
                return Self::from_software_goal(goal, task_id, project_context);
            }
        }

        Self::from_goal_with_id(goal, task_id)
    }

    fn from_software_goal(
        goal: &str,
        task_id: String,
        project_context: &serde_json::Value,
    ) -> Self {
        let tech_stack = project_context
            .pointer("/snapshot/techStack")
            .and_then(|value| value.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str())
                    .take(8)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "未知技术栈".to_string());

        let search_query = project_context
            .get("searchQuery")
            .and_then(|value| value.as_str())
            .unwrap_or("未生成搜索词");

        let related_files = project_context
            .get("searchMatches")
            .and_then(|value| value.as_array())
            .map(|matches| {
                let mut paths = Vec::new();
                for item in matches {
                    if let Some(path) = item.get("path").and_then(|value| value.as_str()) {
                        if !paths.iter().any(|existing: &&str| *existing == path) {
                            paths.push(path);
                        }
                    }
                    if paths.len() >= 5 {
                        break;
                    }
                }
                paths.join(", ")
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "暂无直接命中文件，优先查看项目关键文件".to_string());

        let recommended_commands = project_context
            .pointer("/snapshot/recommendedCommands")
            .and_then(|value| value.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.get("command").and_then(|command| command.as_str()))
                    .take(4)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "cargo check, cargo test".to_string());

        let steps = vec![
            PlanStep {
                step_id: format!("{task_id}-1"),
                order: 1,
                agent: "Planner".to_string(),
                instruction: format!(
                    "基于项目快照理解软件任务: {goal}。当前技术栈: {tech_stack}。"
                ),
                depends_on: vec![],
                status: StepStatus::Pending,
            },
            PlanStep {
                step_id: format!("{task_id}-2"),
                order: 2,
                agent: "Tool".to_string(),
                instruction: format!(
                    "只读检索相关源码。搜索词: {search_query}。优先关注: {related_files}。"
                ),
                depends_on: vec![format!("{task_id}-1")],
                status: StepStatus::Pending,
            },
            PlanStep {
                step_id: format!("{task_id}-3"),
                order: 3,
                agent: "Executor".to_string(),
                instruction: format!(
                    "结合项目结构形成实现方案和模拟执行结果，不直接修改文件: {goal}。建议验证命令: {recommended_commands}。"
                ),
                depends_on: vec![format!("{task_id}-2")],
                status: StepStatus::Pending,
            },
            PlanStep {
                step_id: format!("{task_id}-4"),
                order: 4,
                agent: "Memory".to_string(),
                instruction: format!(
                    "记录本轮项目事实、相关文件和后续实现建议: {goal}。"
                ),
                depends_on: vec![format!("{task_id}-3")],
                status: StepStatus::Pending,
            },
        ];

        Self {
            task_id,
            goal: goal.to_string(),
            steps,
            created_at: chrono::Utc::now(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct LlmTaskPlanEnvelope {
    #[serde(default)]
    goal: Option<String>,
    steps: Vec<LlmPlanStep>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LlmPlanStep {
    #[serde(default, alias = "step_id", alias = "stepId")]
    id: Option<String>,
    agent: String,
    instruction: String,
    #[serde(default, alias = "depends_on", alias = "dependsOn")]
    depends_on: Vec<String>,
}

impl LlmPlanStep {
    fn original_id(&self, fallback_index: usize) -> String {
        self.id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("step-{fallback_index}"))
    }
}

/// Planner LLM 输出 JSON Schema。
pub fn planner_plan_json_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["steps"],
        "properties": {
            "goal": {
                "type": "string",
                "description": "用户目标的简短复述"
            },
            "steps": {
                "type": "array",
                "minItems": 1,
                "maxItems": 12,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id", "agent", "instruction"],
                    "properties": {
                        "id": {
                            "type": "string",
                            "description": "计划内唯一步骤 ID，例如 understand, inspect, implement"
                        },
                        "agent": {
                            "type": "string",
                            "enum": PLANNER_ALLOWED_AGENTS
                        },
                        "instruction": {
                            "type": "string",
                            "minLength": 1
                        },
                        "dependsOn": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "只允许引用前置步骤 ID"
                        }
                    }
                }
            }
        }
    })
}

/// Planner LLM system prompt；要求模型只输出可校验 JSON。
pub fn planner_json_system_prompt() -> String {
    format!(
        "你是软件工程多 Agent 调度器 Planner。只输出一个 JSON 对象，不要 Markdown，不要解释。\
JSON 必须符合此 schema: {}",
        planner_plan_json_schema()
    )
}

fn normalize_agent_id(agent: &str) -> Option<String> {
    match agent.trim().to_ascii_lowercase().as_str() {
        "planner" | "coordinator" => Some("Planner".to_string()),
        "tool" | "toolagent" => Some("Tool".to_string()),
        "executor" | "coder" | "coderagent" => Some("Executor".to_string()),
        "memory" | "memoryagent" => Some("Memory".to_string()),
        "echo" => Some("Echo".to_string()),
        _ => None,
    }
}

fn resolve_depends_on(
    raw_depends_on: &[String],
    original_to_internal: &HashMap<String, String>,
    known_internal_ids: &HashSet<String>,
    current_original_id: &str,
) -> Result<Vec<String>, AgentError> {
    let mut depends_on = Vec::new();
    for raw_dep in raw_depends_on {
        let dep = raw_dep.trim();
        if dep.is_empty() {
            continue;
        }
        let Some(internal_id) = original_to_internal.get(dep).cloned() else {
            return Err(AgentError::MessageFormat(format!(
                "Planner JSON 步骤 {current_original_id} 引用了不存在的依赖: {dep}"
            )));
        };
        if !known_internal_ids.contains(&internal_id) {
            return Err(AgentError::MessageFormat(format!(
                "Planner JSON 步骤 {current_original_id} 的依赖必须指向前置步骤: {dep}"
            )));
        }
        if !depends_on.iter().any(|existing| existing == &internal_id) {
            depends_on.push(internal_id);
        }
    }

    Ok(depends_on)
}

fn extract_json_object(raw: &str) -> Result<String, AgentError> {
    let trimmed = raw.trim();
    let candidate = if trimmed.starts_with("```") {
        let without_opening = trimmed
            .strip_prefix("```json")
            .or_else(|| trimmed.strip_prefix("```JSON"))
            .or_else(|| trimmed.strip_prefix("```"))
            .unwrap_or(trimmed)
            .trim();
        without_opening
            .strip_suffix("```")
            .unwrap_or(without_opening)
            .trim()
    } else {
        trimmed
    };

    let start = candidate
        .find('{')
        .ok_or_else(|| AgentError::MessageFormat("Planner 输出缺少 JSON 对象".to_string()))?;
    let end = candidate
        .rfind('}')
        .ok_or_else(|| AgentError::MessageFormat("Planner 输出缺少 JSON 对象".to_string()))?;
    if end < start {
        return Err(AgentError::MessageFormat(
            "Planner 输出 JSON 对象边界无效".to_string(),
        ));
    }

    Ok(candidate[start..=end].to_string())
}

fn is_software_goal(goal: &str) -> bool {
    let lower = goal.to_lowercase();
    [
        "项目", "代码", "编程", "实现", "修复", "bug", "测试", "构建", "前端", "后端", "页面",
        "组件", "文件", "ipc", "agent", "rust", "react", "tauri", "store",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword))
}

// ============================================================
// PlannerAgent 实现
// ============================================================

/// 任务规划 Agent
///
/// 接收用户目标，生成执行计划，并将计划分发给其他 Agent。
pub struct PlannerAgent {
    /// 已创建的计划数量
    plans_created: u64,
    /// 可选 LLM 客户端；默认关闭，避免测试和本地开发环境意外联网。
    llm_client: Option<LLMClient>,
    #[cfg(test)]
    llm_response_override: Option<Result<String, String>>,
}

impl PlannerAgent {
    pub fn new() -> Self {
        Self {
            plans_created: 0,
            llm_client: None,
            #[cfg(test)]
            llm_response_override: None,
        }
    }

    /// 从环境变量创建 Planner。
    ///
    /// 只有 `PLANNER_USE_LLM=true|1|yes|on` 时才启用 LLM 规划；
    /// 否则保持规则规划，保证默认运行稳定且不访问网络。
    pub fn from_env() -> Self {
        if !planner_llm_enabled() {
            return Self::new();
        }

        let provider = std::env::var("PLANNER_LLM_PROVIDER")
            .ok()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(default_planner_llm_provider);

        let llm_client = match provider.as_str() {
            "openai" => LLMClient::openai_from_env(),
            "deepseek" => LLMClient::deepseek_from_env(),
            _ => {
                tracing::warn!(
                    "未知 PLANNER_LLM_PROVIDER [{}]，Planner LLM 已禁用",
                    provider
                );
                return Self::new();
            }
        };

        Self::with_llm_client(llm_client)
    }

    pub fn with_llm_client(llm_client: LLMClient) -> Self {
        Self {
            plans_created: 0,
            llm_client: Some(llm_client),
            #[cfg(test)]
            llm_response_override: None,
        }
    }

    #[cfg(test)]
    fn with_llm_response(response: Result<String, String>) -> Self {
        Self {
            plans_created: 0,
            llm_client: None,
            llm_response_override: Some(response),
        }
    }

    async fn build_plan(&self, msg: &AgentMessage) -> TaskPlan {
        let task_id = msg
            .task_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        match self.try_build_llm_plan(msg, &task_id).await {
            Ok(Some(plan)) => plan,
            Ok(None) => TaskPlan::from_goal_with_context(&msg.content, task_id, &msg.context),
            Err(err) => {
                tracing::warn!("Planner LLM 规划失败，降级为规则规划: {err}");
                TaskPlan::from_goal_with_context(&msg.content, task_id, &msg.context)
            }
        }
    }

    async fn try_build_llm_plan(
        &self,
        msg: &AgentMessage,
        task_id: &str,
    ) -> Result<Option<TaskPlan>, AgentError> {
        #[cfg(test)]
        if let Some(response) = &self.llm_response_override {
            return match response {
                Ok(content) => TaskPlan::from_llm_json(&msg.content, task_id, content).map(Some),
                Err(err) => Err(AgentError::LlmError(err.clone())),
            };
        }

        let Some(llm_client) = &self.llm_client else {
            if let Some(config) = request_planner_llm_config(&msg.transient_context) {
                return self
                    .call_llm_plan(
                        msg,
                        task_id,
                        &config.client,
                        config.max_tokens,
                        config.temperature,
                    )
                    .await
                    .map(Some);
            }
            return Ok(None);
        };

        let request_config = request_planner_llm_config(&msg.transient_context);
        let (llm_client, max_tokens, temperature) = request_config
            .as_ref()
            .map(|config| (&config.client, config.max_tokens, config.temperature))
            .unwrap_or((llm_client, None, None));

        self.call_llm_plan(msg, task_id, llm_client, max_tokens, temperature)
            .await
            .map(Some)
    }

    async fn call_llm_plan(
        &self,
        msg: &AgentMessage,
        task_id: &str,
        llm_client: &LLMClient,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
    ) -> Result<TaskPlan, AgentError> {
        let request = ChatCompletionRequest {
            system_prompt: Some(planner_json_system_prompt()),
            messages: vec![ChatMessage {
                role: Role::User,
                content: build_planner_llm_user_prompt(msg),
            }],
            max_tokens: Some(max_tokens.unwrap_or(1_500)),
            temperature: Some(temperature.unwrap_or(0.2)),
            response_format: Some(ResponseFormat::JsonObject),
        };

        let attempts = planner_llm_retry_count();
        let mut last_error = None;
        for _ in 0..attempts {
            match llm_client.chat_completion(request.clone()).await {
                Ok(response) => {
                    return TaskPlan::from_llm_json(&msg.content, task_id, &response.content)
                        .map_err(Into::into);
                }
                Err(err) => last_error = Some(err),
            }
        }

        Err(last_error
            .unwrap_or_else(|| AgentError::LlmError("Planner LLM 调用未执行".to_string())))
    }
}

struct RequestPlannerLlmConfig {
    client: LLMClient,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
}

fn request_planner_llm_config(context: &serde_json::Value) -> Option<RequestPlannerLlmConfig> {
    let settings = context.get("plannerLlmSettings")?;
    let api_key = settings
        .get("apiKey")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let model = settings
        .get("model")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let api_base_url = settings
        .get("apiBaseUrl")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let max_tokens = settings
        .get("maxTokens")
        .and_then(|value| value.as_u64())
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0);
    let temperature = settings
        .get("temperature")
        .and_then(|value| value.as_f64())
        .map(|value| value.clamp(0.0, 2.0) as f32);

    Some(RequestPlannerLlmConfig {
        client: LLMClient::openai_compatible(api_key, model, api_base_url),
        max_tokens,
        temperature,
    })
}

impl Default for PlannerAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for PlannerAgent {
    fn name(&self) -> &str {
        "Planner"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![Capability::planning()]
    }

    async fn handle_message(&mut self, msg: AgentMessage) -> Result<Vec<AgentMessage>, AgentError> {
        self.plans_created += 1;

        let plan = self.build_plan(&msg).await;

        tracing::info!(
            "PlannerAgent 为任务 {} 创建了 {} 个步骤的计划",
            plan.task_id,
            plan.steps.len()
        );

        // 序列化计划为 JSON，放入消息 context
        let context = serde_json::to_value(&plan)
            .map_err(|e| AgentError::Internal(format!("计划序列化失败: {e}")))?;

        // 构建包含计划的回复消息
        let reply = AgentMessage::new(
            self.name(),
            &msg.from,
            &format!("已为任务创建计划，共 {} 个步骤", plan.steps.len()),
        )
        .with_type("plan_created")
        .with_context(context)
        .with_task_id(&plan.task_id);

        // 为每个步骤生成分派消息（发送给对应的 Agent）
        let mut dispatch_messages = Vec::new();
        for step in &plan.steps {
            let dispatch_msg = AgentMessage::new(self.name(), &step.agent, &step.instruction)
                .with_type("plan_step")
                .with_task_id(&plan.task_id);

            dispatch_messages.push(dispatch_msg);
        }

        // 合并回复和分派消息
        let mut all_messages = vec![reply];
        all_messages.extend(dispatch_messages);

        Ok(all_messages)
    }
}

fn planner_llm_enabled() -> bool {
    std::env::var("PLANNER_USE_LLM")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn default_planner_llm_provider() -> String {
    if std::env::var("OPENAI_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_some()
    {
        "openai".to_string()
    } else {
        "deepseek".to_string()
    }
}

fn planner_llm_retry_count() -> usize {
    std::env::var("PLANNER_LLM_RETRIES")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .map(|value| value.clamp(1, 3))
        .unwrap_or(DEFAULT_PLANNER_LLM_RETRIES)
}

fn build_planner_llm_user_prompt(msg: &AgentMessage) -> String {
    let context = if msg.context.is_null() {
        "null".to_string()
    } else {
        serde_json::to_string_pretty(&msg.context).unwrap_or_else(|_| msg.context.to_string())
    };

    format!(
        "用户目标:\n{}\n\n项目上下文 JSON:\n{}\n\n请生成适合当前多 Agent 系统执行的计划。",
        msg.content, context
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_planner_simple_goal() {
        let mut planner = PlannerAgent::new();

        let msg = AgentMessage::new("User", "Planner", "帮我整理今日新闻");
        let replies = planner.handle_message(msg).await.unwrap();

        // 应该有 1 个回复 + 3 个分派消息
        assert_eq!(replies.len(), 4);
        assert_eq!(replies[0].msg_type, "plan_created");

        // 检查分派消息的目标
        let targets: Vec<&str> = replies.iter().skip(1).map(|m| m.to.as_str()).collect();
        assert!(targets.contains(&"Tool"));
        assert!(targets.contains(&"Executor"));
        assert!(targets.contains(&"Memory"));
    }

    #[tokio::test]
    async fn test_planner_code_request() {
        let mut planner = PlannerAgent::new();

        let msg = AgentMessage::new("User", "Planner", "写一个排序函数");
        let replies = planner.handle_message(msg).await.unwrap();

        // 编码请求应有 1 + 2 = 3 条消息
        assert_eq!(replies.len(), 3);
    }

    /// 测试 — 默认目标生成单步计划
    /// 验证：不包含任何关键词的目标只生成一个步骤（Echo）
    #[tokio::test]
    async fn test_planner_default_single_step() {
        let mut planner = PlannerAgent::new();

        let msg = AgentMessage::new("User", "Planner", "你好");
        let replies = planner.handle_message(msg).await.unwrap();

        // 只有 1 回复 + 1 分派 = 2 条消息
        assert_eq!(replies.len(), 2);
        // 分派消息应发送给 Echo
        assert_eq!(replies[1].to, "Echo");
    }

    /// 测试 — 计划回复中包含正确的类型标记
    /// 验证：第一条回复的 msg_type 为 "plan_created"
    #[tokio::test]
    async fn test_planner_reply_type() {
        let mut planner = PlannerAgent::new();

        let msg = AgentMessage::new("User", "Planner", "帮我搜索信息");
        let replies = planner.handle_message(msg).await.unwrap();

        // 第一条消息是计划创建确认
        assert_eq!(replies[0].msg_type, "plan_created");
        // 后续消息是分派步骤
        assert_eq!(replies[1].msg_type, "plan_step");
    }

    /// 测试 — 计划包含 task_id
    /// 验证：创建的计划关联了 task_id，且分派消息也包含同一 task_id
    #[tokio::test]
    async fn test_planner_task_id_present() {
        let mut planner = PlannerAgent::new();

        let msg = AgentMessage::new("User", "Planner", "搜索新闻");
        let replies = planner.handle_message(msg).await.unwrap();

        // 检查计划回复包含 task_id
        assert!(replies[0].task_id.is_some());
        let tid = replies[0].task_id.as_ref().unwrap();

        // 所有分派消息应包含相同的 task_id
        for reply in replies.iter().skip(1) {
            assert_eq!(reply.task_id.as_ref().unwrap(), tid);
        }
    }

    /// 测试 — 计划上下文包含完整 TaskPlan JSON
    /// 验证：计划回复的 context 字段包含序列化的 TaskPlan
    #[tokio::test]
    async fn test_planner_context_contains_plan() {
        let mut planner = PlannerAgent::new();

        let msg = AgentMessage::new("User", "Planner", "搜索新闻");
        let replies = planner.handle_message(msg).await.unwrap();

        // 计划回复的 context 应包含 TaskPlan
        let ctx = &replies[0].context;
        assert!(ctx.get("task_id").is_some());
        assert!(ctx.get("goal").is_some());
        assert!(ctx.get("steps").is_some());

        // 验证步骤数组
        let steps = ctx["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 3);
    }

    /// 测试 — 多次规划后计划计数递增
    /// 验证：两次规划后 plans_created 递增
    #[tokio::test]
    async fn test_planner_multiple_calls() {
        let mut planner = PlannerAgent::new();

        // 第一次规划
        let msg1 = AgentMessage::new("User", "Planner", "搜索新闻");
        planner.handle_message(msg1).await.unwrap();
        assert_eq!(planner.plans_created, 1);

        // 第二次规划
        let msg2 = AgentMessage::new("User", "Planner", "写代码");
        planner.handle_message(msg2).await.unwrap();
        assert_eq!(planner.plans_created, 2);
    }

    /// 测试 — 计划步骤的依赖关系
    /// 验证：搜索类任务的步骤 2 依赖于步骤 1
    #[tokio::test]
    async fn test_planner_step_dependencies() {
        let mut planner = PlannerAgent::new();

        let msg = AgentMessage::new("User", "Planner", "今天有什么新闻？");
        let replies = planner.handle_message(msg).await.unwrap();

        // 从 context 解析 TaskPlan
        let plan: crate::agent::planner_agent::TaskPlan =
            serde_json::from_value(replies[0].context.clone()).unwrap();

        // 步骤 2 应依赖步骤 1
        let step2 = plan.steps.iter().find(|s| s.order == 2).unwrap();
        assert!(!step2.depends_on.is_empty());

        // 步骤 1 无依赖
        let step1 = plan.steps.iter().find(|s| s.order == 1).unwrap();
        assert!(step1.depends_on.is_empty());
    }

    /// 测试 — 编程类目标的计划结构
    /// 验证：包含"编程"关键词的目标生成 2 步计划（Planner + Executor）
    #[tokio::test]
    async fn test_planner_code_structure() {
        let mut planner = PlannerAgent::new();

        let msg = AgentMessage::new("User", "Planner", "用 Python 编程实现快速排序");
        let replies = planner.handle_message(msg).await.unwrap();

        // 1 回复 + 2 分派 = 3 条
        assert_eq!(replies.len(), 3);

        // 分派消息的目标应为 Planner 和 Executor
        let targets: Vec<&str> = replies.iter().skip(1).map(|m| m.to.as_str()).collect();
        assert!(targets.contains(&"Planner"));
        assert!(targets.contains(&"Executor"));
    }

    /// 测试 — 带项目上下文的软件任务生成项目感知计划
    #[tokio::test]
    async fn test_planner_project_aware_software_plan() {
        let mut planner = PlannerAgent::new();
        let context = serde_json::json!({
            "projectPlanningContext": {
                "snapshot": {
                    "techStack": ["Rust", "Tauri v2", "React"],
                    "recommendedCommands": [
                        { "command": "cargo test" },
                        { "command": "npm test -- --run" }
                    ]
                },
                "searchQuery": "Task",
                "searchMatches": [
                    { "path": "src-web/src/components/TaskBoard.tsx" },
                    { "path": "src-tauri/src/orchestrator/mod.rs" }
                ]
            }
        });

        let msg = AgentMessage::new("User", "Planner", "优化项目任务看板")
            .with_task_id("task-project")
            .with_context(context);
        let replies = planner.handle_message(msg).await.unwrap();

        assert_eq!(replies.len(), 5);
        let plan: TaskPlan = serde_json::from_value(replies[0].context.clone()).unwrap();
        assert_eq!(plan.steps.len(), 4);
        assert!(plan.steps[0].instruction.contains("Rust"));
        assert!(plan.steps[1].instruction.contains("TaskBoard.tsx"));
        assert!(plan.steps[2].instruction.contains("cargo test"));
    }

    /// 测试 — 普通搜索任务不会因项目上下文被误判为软件任务
    #[tokio::test]
    async fn test_planner_project_context_keeps_non_software_search_plan() {
        let mut planner = PlannerAgent::new();
        let context = serde_json::json!({
            "projectPlanningContext": {
                "snapshot": { "techStack": ["Rust"] },
                "searchQuery": "Agent",
                "searchMatches": []
            }
        });

        let msg = AgentMessage::new("User", "Planner", "帮我搜索今日新闻")
            .with_task_id("task-news")
            .with_context(context);
        let replies = planner.handle_message(msg).await.unwrap();

        assert_eq!(replies.len(), 4);
        let plan: TaskPlan = serde_json::from_value(replies[0].context.clone()).unwrap();
        assert_eq!(plan.steps.len(), 3);
        assert_eq!(plan.steps[0].agent, "Tool");
    }

    /// 测试 — Planner JSON schema 包含步骤约束和 Agent 枚举
    #[test]
    fn test_planner_plan_json_schema_shape() {
        let schema = planner_plan_json_schema();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["steps"]["minItems"], 1);
        assert!(
            schema["properties"]["steps"]["items"]["properties"]["agent"]["enum"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == "Tool")
        );

        let prompt = planner_json_system_prompt();
        assert!(prompt.contains("只输出一个 JSON 对象"));
        assert!(prompt.contains("steps"));
    }

    /// 测试 — 从 LLM JSON 解析 TaskPlan 并规范化内部 step_id
    #[test]
    fn test_task_plan_from_llm_json_normalizes_steps() {
        let raw = r#"{
            "goal": "优化任务看板",
            "steps": [
                {
                    "id": "understand",
                    "agent": "planner",
                    "instruction": "理解需求和项目上下文"
                },
                {
                    "id": "inspect",
                    "agent": "Tool",
                    "instruction": "只读检索 TaskBoard 相关文件",
                    "dependsOn": ["understand"]
                },
                {
                    "id": "summarize",
                    "agent": "coder",
                    "instruction": "形成实现方案，不直接修改文件",
                    "dependsOn": ["inspect"]
                }
            ]
        }"#;

        let plan = TaskPlan::from_llm_json("fallback goal", "task-llm", raw).unwrap();

        assert_eq!(plan.task_id, "task-llm");
        assert_eq!(plan.goal, "优化任务看板");
        assert_eq!(plan.steps.len(), 3);
        assert_eq!(plan.steps[0].step_id, "task-llm-1");
        assert_eq!(plan.steps[0].agent, "Planner");
        assert_eq!(plan.steps[1].depends_on, vec!["task-llm-1".to_string()]);
        assert_eq!(plan.steps[2].agent, "Executor");
        assert_eq!(plan.steps[2].depends_on, vec!["task-llm-2".to_string()]);
    }

    /// 测试 — 可从 Markdown fenced JSON 中提取计划
    #[test]
    fn test_task_plan_from_llm_json_accepts_fenced_json() {
        let raw = r#"```json
        {
            "steps": [
                {
                    "id": "answer",
                    "agent": "Echo",
                    "instruction": "回复用户"
                }
            ]
        }
        ```"#;

        let plan = TaskPlan::from_llm_json("原始目标", "task-fenced", raw).unwrap();
        assert_eq!(plan.goal, "原始目标");
        assert_eq!(plan.steps[0].step_id, "task-fenced-1");
        assert_eq!(plan.steps[0].agent, "Echo");
    }

    /// 测试 — 未知 Agent 会被拒绝
    #[test]
    fn test_task_plan_from_llm_json_rejects_unknown_agent() {
        let raw = r#"{
            "steps": [
                {
                    "id": "unsafe",
                    "agent": "Shell",
                    "instruction": "执行任意命令"
                }
            ]
        }"#;

        let err = TaskPlan::from_llm_json("目标", "task-bad", raw).unwrap_err();
        assert!(format!("{}", err).contains("不允许的 Agent"));
    }

    /// 测试 — 依赖只能引用前置步骤
    #[test]
    fn test_task_plan_from_llm_json_rejects_forward_dependency() {
        let raw = r#"{
            "steps": [
                {
                    "id": "first",
                    "agent": "Planner",
                    "instruction": "第一步",
                    "dependsOn": ["second"]
                },
                {
                    "id": "second",
                    "agent": "Tool",
                    "instruction": "第二步"
                }
            ]
        }"#;

        let err = TaskPlan::from_llm_json("目标", "task-forward", raw).unwrap_err();
        assert!(format!("{}", err).contains("依赖必须指向前置步骤"));
    }

    /// 测试 — Planner 可使用 LLM JSON 输出生成计划
    #[tokio::test]
    async fn test_planner_uses_llm_json_when_available() {
        let raw = r#"{
            "goal": "优化任务看板",
            "steps": [
                {
                    "id": "inspect",
                    "agent": "Tool",
                    "instruction": "只读检索 TaskBoard 相关源码"
                },
                {
                    "id": "plan",
                    "agent": "Executor",
                    "instruction": "整理实现方案",
                    "dependsOn": ["inspect"]
                }
            ]
        }"#;
        let mut planner = PlannerAgent::with_llm_response(Ok(raw.to_string()));
        let msg = AgentMessage::new("User", "Planner", "优化项目任务看板")
            .with_task_id("task-llm-runtime");

        let replies = planner.handle_message(msg).await.unwrap();
        let plan: TaskPlan = serde_json::from_value(replies[0].context.clone()).unwrap();

        assert_eq!(plan.goal, "优化任务看板");
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].agent, "Tool");
        assert_eq!(plan.steps[1].depends_on, vec!["task-llm-runtime-1"]);
        assert_eq!(replies.len(), 3);
    }

    /// 测试 — LLM 失败时 Planner 降级为规则规划
    #[tokio::test]
    async fn test_planner_falls_back_when_llm_fails() {
        let mut planner = PlannerAgent::with_llm_response(Err("模型暂不可用".to_string()));
        let msg =
            AgentMessage::new("User", "Planner", "帮我搜索今日新闻").with_task_id("task-fallback");

        let replies = planner.handle_message(msg).await.unwrap();
        let plan: TaskPlan = serde_json::from_value(replies[0].context.clone()).unwrap();

        assert_eq!(plan.task_id, "task-fallback");
        assert_eq!(plan.steps.len(), 3);
        assert_eq!(plan.steps[0].agent, "Tool");
    }

    /// 测试 — 默认环境不会启用 Planner LLM，避免意外联网
    #[test]
    fn test_planner_from_env_defaults_to_rule_mode() {
        std::env::remove_var("PLANNER_USE_LLM");
        let planner = PlannerAgent::from_env();
        assert!(planner.llm_client.is_none());
    }

    #[test]
    fn test_request_planner_llm_config_builds_transient_client() {
        let config = request_planner_llm_config(&serde_json::json!({
            "plannerLlmSettings": {
                "apiKey": "sk-request",
                "model": "custom-model",
                "apiBaseUrl": "https://example.com/v1",
                "maxTokens": 2048,
                "temperature": 0.3
            }
        }))
        .unwrap();

        assert_eq!(config.client.api_key(), Some("sk-request"));
        assert_eq!(config.client.model(), "custom-model");
        assert_eq!(
            config.client.endpoint(),
            "https://example.com/v1/chat/completions"
        );
        assert_eq!(config.max_tokens, Some(2048));
        assert_eq!(config.temperature, Some(0.3));
    }

    #[test]
    fn test_planner_llm_prompt_omits_transient_context() {
        let msg = AgentMessage::new("User", "Planner", "实现功能")
            .with_context(serde_json::json!({ "projectPlanningContext": { "name": "demo" } }))
            .with_transient_context(serde_json::json!({
                "plannerLlmSettings": { "apiKey": "sk-secret" }
            }));

        let prompt = build_planner_llm_user_prompt(&msg);

        assert!(prompt.contains("projectPlanningContext"));
        assert!(!prompt.contains("sk-secret"));
        assert!(!prompt.contains("plannerLlmSettings"));
    }

    /// 测试 — PlanStep 序列化反序列化
    /// 验证：PlanStep 可以正确序列化为 JSON 再反序列化
    #[test]
    fn test_plan_step_serialization() {
        let step = PlanStep {
            step_id: "s1".to_string(),
            order: 1,
            agent: "Tool".to_string(),
            instruction: "搜索".to_string(),
            depends_on: vec![],
            status: StepStatus::Pending,
        };

        let json = serde_json::to_string(&step).unwrap();
        let restored: PlanStep = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.step_id, "s1");
        assert_eq!(restored.agent, "Tool");
        assert_eq!(restored.status, StepStatus::Pending);
    }

    /// 测试 — 默认构造器
    /// 验证：PlannerAgent::default() 创建正确的 Agent
    #[tokio::test]
    async fn test_planner_default() {
        let planner = PlannerAgent::default();
        assert_eq!(planner.name(), "Planner");
        assert_eq!(planner.plans_created, 0);
    }
}
