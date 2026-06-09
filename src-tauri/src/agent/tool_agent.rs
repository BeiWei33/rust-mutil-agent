//! 工具 Agent（ToolAgent）
//!
//! 负责注册、管理和调用外部工具/函数。
//! 提供插件化的工具扩展机制，每个工具实现特定的功能。
//!
//! # 内置工具
//! - `calculator`：简单数学计算
//! - `datetime`：获取当前时间
//! - `web_search`：网络搜索（占位，需接入真实 API）
//! - `file_read` / `file_write`：文件操作
//!
//! # 扩展机制
//! 通过 `ToolRegistry` 注册自定义工具函数，
//! 每个工具接收 JSON 参数，返回 JSON 结果。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use super::traits::{Agent, AgentMessage, Capability};
use crate::error::AgentError;
use crate::runtime::tool_invocation::summarize_tool_args;
use crate::runtime::{ToolInvocationInput, ToolInvocationRecord, ToolInvocationStore};
use std::time::Instant;

// ============================================================
// 工具注册表
// ============================================================

/// 工具函数签名
///
/// 接收 JSON Value 参数，返回 JSON Value 结果。
/// 使用 Arc 包装以支持多线程共享。
pub type ToolFn = Arc<dyn Fn(serde_json::Value) -> Result<serde_json::Value, String> + Send + Sync>;

/// 工具描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescription {
    /// 工具名称（唯一标识）
    pub name: String,

    /// 工具描述
    pub description: String,

    /// 参数 schema（JSON Schema 格式，用于 LLM function calling）
    pub parameters: serde_json::Value,
}

/// 工具注册表
///
/// 维护所有已注册的工具，支持按名称调用。
#[derive(Clone)]
pub struct ToolRegistry {
    /// 工具名称 -> 工具函数映射
    tools: HashMap<String, ToolFn>,

    /// 工具名称 -> 工具描述映射
    descriptions: HashMap<String, ToolDescription>,
}

impl ToolRegistry {
    /// 创建空注册表
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            descriptions: HashMap::new(),
        }
    }

    /// 注册一个新工具
    ///
    /// # 参数
    /// - `description`：工具描述（名称、说明、参数 schema）
    /// - `f`：工具实现函数
    pub fn register(
        &mut self,
        description: ToolDescription,
        f: impl Fn(serde_json::Value) -> Result<serde_json::Value, String> + Send + Sync + 'static,
    ) {
        let name = description.name.clone();
        self.tools.insert(name.clone(), Arc::new(f));
        self.descriptions.insert(name, description);
    }

    /// 调用工具
    ///
    /// # 参数
    /// - `name`：工具名称
    /// - `args`：JSON 格式参数
    pub fn call(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, AgentError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| AgentError::ToolNotFound(format!("工具 '{name}' 未注册")))?;

        tool(args).map_err(|e| AgentError::Tool(format!("工具 '{name}' 调用失败: {e}")))
    }

    /// 列出所有已注册工具的描述
    pub fn list_tools(&self) -> Vec<ToolDescription> {
        self.descriptions.values().cloned().collect()
    }

    /// 检查工具是否已注册
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// 内置工具实现
// ============================================================

/// 注册所有内置工具到注册表
pub fn register_builtin_tools(registry: &mut ToolRegistry) {
    // --- 1. 计算器 ---
    registry.register(
        ToolDescription {
            name: "calculator".to_string(),
            description: "执行数学表达式计算".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "数学表达式，如 '2 + 3 * 4'"
                    }
                },
                "required": ["expression"]
            }),
        },
        |args| {
            let expr = args["expression"].as_str().ok_or("缺少 expression 参数")?;
            // 简单计算器：仅支持基本四则运算
            let result = eval_expression(expr)?;
            Ok(serde_json::json!({ "result": result, "expression": expr }))
        },
    );

    // --- 2. 日期时间 ---
    registry.register(
        ToolDescription {
            name: "datetime".to_string(),
            description: "获取当前日期和时间".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "format": {
                        "type": "string",
                        "description": "日期格式：'iso' | 'readable' | 'timestamp'"
                    }
                }
            }),
        },
        |args| {
            let format = args["format"].as_str().unwrap_or("iso");
            let now = chrono::Utc::now();
            let result = match format {
                "readable" => now.format("%Y年%m月%d日 %H:%M:%S").to_string(),
                "timestamp" => now.timestamp().to_string(),
                _ => now.to_rfc3339(),
            };
            Ok(serde_json::json!({ "datetime": result, "timezone": "UTC" }))
        },
    );

    // --- 3. Workspace 沙箱文件读取 ---
    registry.register(
        ToolDescription {
            name: "file_read".to_string(),
            description: "读取 workspace 内的普通文本文件内容".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "workspace 内相对文件路径" }
                },
                "required": ["path"]
            }),
        },
        |args| {
            let path = args["path"].as_str().ok_or("缺少 path 参数")?;
            let file = crate::workspace::read_file(path).map_err(|e| format!("{}", e))?;
            Ok(serde_json::json!({
                "content": file.content,
                "path": file.path,
                "size": file.size_bytes,
            }))
        },
    );

    // --- 4. 网络搜索（占位） ---
    registry.register(
        ToolDescription {
            name: "web_search".to_string(),
            description: "搜索互联网信息（需接入搜索引擎 API）".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "搜索关键词" },
                    "max_results": { "type": "integer", "description": "最大结果数", "default": 5 }
                },
                "required": ["query"]
            }),
        },
        |args| {
            let query = args["query"].as_str().ok_or("缺少 query 参数")?;
            let max = args["max_results"].as_u64().unwrap_or(5);

            // 占位：返回模拟搜索结果
            Ok(serde_json::json!({
                "query": query,
                "results": [format!("[模拟] 关于 '{query}' 的搜索结果（共 {max} 条）")],
                "note": "请接入真实搜索引擎 API 以启用此功能"
            }))
        },
    );

    tracing::info!("已注册 {} 个内置工具", registry.list_tools().len());
}

/// 计算数学表达式（简化版，仅支持四则运算）
fn eval_expression(expr: &str) -> Result<f64, String> {
    // 移除空白符
    let expr: String = expr.chars().filter(|c| !c.is_whitespace()).collect();

    // 使用简单的解析方式：tokenize → shunting-yard → evaluate
    let tokens = tokenize(&expr)?;
    let postfix = shunting_yard(&tokens)?;
    evaluate_postfix(&postfix)
}

/// Token 类型
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Plus,
    Minus,
    Multiply,
    Divide,
    LParen,
    RParen,
}

/// 分词
fn tokenize(expr: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = expr.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            '0'..='9' | '.' => {
                let mut num_str = String::new();
                while let Some(&nc) = chars.peek() {
                    if nc.is_ascii_digit() || nc == '.' {
                        num_str.push(nc);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let num: f64 = num_str
                    .parse()
                    .map_err(|e| format!("无效数字 '{}': {e}", num_str))?;
                tokens.push(Token::Number(num));
            }
            '+' => {
                tokens.push(Token::Plus);
                chars.next();
            }
            '-' => {
                tokens.push(Token::Minus);
                chars.next();
            }
            '*' => {
                tokens.push(Token::Multiply);
                chars.next();
            }
            '/' => {
                tokens.push(Token::Divide);
                chars.next();
            }
            '(' => {
                tokens.push(Token::LParen);
                chars.next();
            }
            ')' => {
                tokens.push(Token::RParen);
                chars.next();
            }
            _ => {
                return Err(format!("无效字符: '{c}'"));
            }
        }
    }

    Ok(tokens)
}

/// 调度场算法 → 后缀表达式
fn shunting_yard(tokens: &[Token]) -> Result<Vec<Token>, String> {
    let mut output: Vec<Token> = Vec::new();
    let mut stack: Vec<Token> = Vec::new();

    fn precedence(t: &Token) -> u8 {
        match t {
            Token::Plus | Token::Minus => 1,
            Token::Multiply | Token::Divide => 2,
            _ => 0,
        }
    }

    for token in tokens {
        match token {
            Token::Number(_) => output.push(token.clone()),
            Token::LParen => stack.push(token.clone()),
            Token::RParen => {
                while let Some(top) = stack.last() {
                    if *top == Token::LParen {
                        break;
                    }
                    output.push(stack.pop().unwrap());
                }
                if stack.pop() != Some(Token::LParen) {
                    return Err("括号不匹配".to_string());
                }
            }
            op @ (Token::Plus | Token::Minus | Token::Multiply | Token::Divide) => {
                while let Some(top) = stack.last() {
                    if *top == Token::LParen || precedence(top) < precedence(op) {
                        break;
                    }
                    output.push(stack.pop().unwrap());
                }
                stack.push(op.clone());
            }
        }
    }

    while let Some(token) = stack.pop() {
        if token == Token::LParen {
            return Err("括号不匹配".to_string());
        }
        output.push(token);
    }

    Ok(output)
}

/// 计算后缀表达式
fn evaluate_postfix(tokens: &[Token]) -> Result<f64, String> {
    let mut stack: Vec<f64> = Vec::new();

    for token in tokens {
        match token {
            Token::Number(n) => stack.push(*n),
            Token::Plus => {
                let b = stack.pop().ok_or("操作数不足")?;
                let a = stack.pop().ok_or("操作数不足")?;
                stack.push(a + b);
            }
            Token::Minus => {
                let b = stack.pop().ok_or("操作数不足")?;
                let a = stack.pop().ok_or("操作数不足")?;
                stack.push(a - b);
            }
            Token::Multiply => {
                let b = stack.pop().ok_or("操作数不足")?;
                let a = stack.pop().ok_or("操作数不足")?;
                stack.push(a * b);
            }
            Token::Divide => {
                let b = stack.pop().ok_or("操作数不足")?;
                if b == 0.0 {
                    return Err("除数不能为零".to_string());
                }
                let a = stack.pop().ok_or("操作数不足")?;
                stack.push(a / b);
            }
            _ => return Err("无效的 token".to_string()),
        }
    }

    if stack.len() != 1 {
        return Err("表达式计算错误".to_string());
    }

    Ok(stack[0])
}

// ============================================================
// ToolAgent 实现
// ============================================================

/// 工具 Agent
///
/// 接收工具调用指令，通过 ToolRegistry 执行对应工具，返回结果。
pub struct ToolAgent {
    /// 工具注册表
    registry: ToolRegistry,

    /// 调用计数
    count: u64,

    /// 可选工具调用审计存储。
    invocation_store: Option<Arc<ToolInvocationStore>>,
}

impl ToolAgent {
    pub fn new() -> Self {
        Self {
            registry: ToolRegistry::new(),
            count: 0,
            invocation_store: None,
        }
    }

    /// 使用预配置的注册表创建
    pub fn with_registry(registry: ToolRegistry) -> Self {
        Self {
            registry,
            count: 0,
            invocation_store: None,
        }
    }

    /// 启用工具调用审计。
    pub fn with_invocation_store(mut self, store: Arc<ToolInvocationStore>) -> Self {
        self.invocation_store = Some(store);
        self
    }

    fn record_invocation(
        &self,
        msg: &AgentMessage,
        tool_name: &str,
        args: serde_json::Value,
        success: bool,
        error: Option<String>,
        duration_ms: u64,
    ) -> Option<String> {
        let store = self.invocation_store.as_ref()?;
        let record = ToolInvocationRecord::from_input(ToolInvocationInput {
            task_id: msg.task_id.clone(),
            step_id: json_string_field(&msg.context, "stepId"),
            approval_id: json_string_field(&msg.context, "toolApprovalId"),
            tool_name: tool_name.to_string(),
            args,
            success,
            error,
            duration_ms,
        });
        let id = record.id.clone();
        if let Err(err) = store.append_invocation(&record) {
            tracing::warn!("工具调用审计写入失败: {err}");
            return None;
        }
        Some(id)
    }
}

impl Default for ToolAgent {
    fn default() -> Self {
        let mut agent = Self::new();
        register_builtin_tools(&mut agent.registry);
        agent
    }
}

#[async_trait]
impl Agent for ToolAgent {
    fn name(&self) -> &str {
        "Tool"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![Capability::tool_use()]
    }

    async fn handle_message(&mut self, msg: AgentMessage) -> Result<Vec<AgentMessage>, AgentError> {
        self.count += 1;

        // 从消息中解析工具名称和参数
        let tool_name = msg.context["tool"].as_str().unwrap_or_else(|| {
            // 如果没有显式指定，从内容中提取
            let content = &msg.content;
            if content.contains("计算") || content.contains("calc") {
                "calculator"
            } else if content.contains("时间") || content.contains("日期") {
                "datetime"
            } else if content.contains("搜索") || content.contains("search") {
                "web_search"
            } else if content.contains("文件") || content.contains("file") {
                "file_read"
            } else {
                "calculator" // 默认尝试计算器
            }
        });

        let args = msg
            .context
            .get("args")
            .cloned()
            .unwrap_or_else(|| default_tool_args(tool_name, &msg.content));

        tracing::info!(
            "ToolAgent 调用工具: {tool_name}, 参数摘要: {}",
            summarize_tool_args(&args)
        );

        // 调用工具
        let start = Instant::now();
        let args_for_audit = args.clone();
        match self.registry.call(tool_name, args) {
            Ok(result) => {
                let audit_id = self.record_invocation(
                    &msg,
                    tool_name,
                    args_for_audit,
                    true,
                    None,
                    elapsed_ms(start),
                );
                let reply = msg
                    .reply_to(&format!("工具 [{tool_name}] 执行结果: {result}",))
                    .with_type("tool_result")
                    .with_context(serde_json::json!({
                        "tool": tool_name,
                        "result": result,
                        "toolInvocationId": audit_id,
                    }));

                Ok(vec![reply])
            }
            Err(e) => {
                let error = format!("{e}");
                let audit_id = self.record_invocation(
                    &msg,
                    tool_name,
                    args_for_audit,
                    false,
                    Some(error.clone()),
                    elapsed_ms(start),
                );
                tracing::error!("工具调用失败: {error}");

                let reply = msg
                    .reply_to(&format!("工具调用失败: {error}"))
                    .with_type("tool_error")
                    .with_context(serde_json::json!({
                        "tool": tool_name,
                        "error": error,
                        "toolInvocationId": audit_id,
                    }));

                Ok(vec![reply])
            }
        }
    }
}

fn elapsed_ms(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn json_string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn default_tool_args(tool_name: &str, content: &str) -> serde_json::Value {
    match tool_name {
        "file_read" => {
            infer_file_read_args_from_content(content).unwrap_or_else(|| serde_json::json!({}))
        }
        _ => serde_json::json!({
            "expression": content,
            "query": content,
        }),
    }
}

pub(crate) fn infer_file_read_args_from_content(content: &str) -> Option<serde_json::Value> {
    infer_file_path_from_content(content).map(|path| serde_json::json!({ "path": path }))
}

fn infer_file_path_from_content(content: &str) -> Option<String> {
    let normalized = content
        .chars()
        .map(|ch| {
            if matches!(
                ch,
                '，' | '。'
                    | '；'
                    | '：'
                    | ':'
                    | '、'
                    | '“'
                    | '”'
                    | '"'
                    | '\''
                    | '`'
                    | '('
                    | ')'
                    | '（'
                    | '）'
                    | '['
                    | ']'
                    | '{'
                    | '}'
            ) {
                ' '
            } else {
                ch
            }
        })
        .collect::<String>();
    let tokens = normalized.split_whitespace().collect::<Vec<_>>();

    for window in tokens.windows(2) {
        let label = window[0].to_ascii_lowercase();
        if matches!(label.as_str(), "path" | "file" | "文件" | "路径") {
            if let Some(path) = clean_path_token(window[1]) {
                return Some(path);
            }
        }
    }

    tokens
        .iter()
        .rev()
        .find_map(|token| clean_path_token(token))
}

fn clean_path_token(token: &str) -> Option<String> {
    let mut token = token
        .trim_matches(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    ',' | ';' | '，' | '。' | '；' | '、' | '"' | '\'' | '`' | '“' | '”'
                )
        })
        .trim();
    if token.ends_with('.') && token.trim_end_matches('.').contains('.') {
        token = token.trim_end_matches('.');
    }
    if token.is_empty() {
        return None;
    }

    let lower = token.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "path" | "file" | "read" | "读取" | "文件" | "路径"
    ) {
        return None;
    }

    let looks_like_path = token.contains('/')
        || token.contains('\\')
        || token.contains('.')
        || token.starts_with('.');
    looks_like_path.then(|| token.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculator_tool() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);

        let result = registry
            .call("calculator", serde_json::json!({"expression": "2 + 3 * 4"}))
            .unwrap();

        // 2 + 3 * 4 = 2 + 12 = 14
        assert_eq!(result["result"].as_f64().unwrap(), 14.0);
    }

    #[test]
    fn test_datetime_tool() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);

        let result = registry
            .call("datetime", serde_json::json!({"format": "iso"}))
            .unwrap();

        assert!(result["datetime"].as_str().is_some());
    }

    #[test]
    fn test_file_read_uses_workspace_sandbox() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);

        let result = registry
            .call("file_read", serde_json::json!({ "path": "README.md" }))
            .unwrap();

        assert_eq!(result["path"], serde_json::json!("README.md"));
        assert!(result["content"]
            .as_str()
            .unwrap()
            .contains("多 Agent 协同智能体"));
        assert!(result["size"].as_u64().unwrap() > 0);
    }

    #[test]
    fn test_file_read_rejects_workspace_escape() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);

        let parent_result =
            registry.call("file_read", serde_json::json!({ "path": "../Cargo.toml" }));
        assert!(parent_result.is_err());
        assert!(format!("{}", parent_result.unwrap_err()).contains("workspace 外部"));

        let absolute_path = crate::workspace::workspace_root().join("README.md");
        let absolute_result = registry.call(
            "file_read",
            serde_json::json!({ "path": absolute_path.to_string_lossy() }),
        );
        assert!(absolute_result.is_err());
        assert!(format!("{}", absolute_result.unwrap_err()).contains("workspace 外部"));
    }

    #[tokio::test]
    async fn test_tool_agent_calculate() {
        let mut agent = ToolAgent::default();

        let msg = AgentMessage::new("User", "Tool", "2 + 3 * 4")
            .with_context(serde_json::json!({"tool": "calculator"}));

        let replies = agent.handle_message(msg).await.unwrap();
        assert_eq!(replies[0].msg_type, "tool_result");
        assert!(replies[0].content.contains("14"));
    }

    #[tokio::test]
    async fn test_tool_agent_records_invocation_audit() {
        let store = Arc::new(ToolInvocationStore::open(":memory:").unwrap());
        let mut agent = ToolAgent::default().with_invocation_store(store.clone());

        let msg = AgentMessage::new("Orchestrator", "Tool", "2 + 2")
            .with_task_id("task-1")
            .with_context(serde_json::json!({
                "tool": "calculator",
                "args": { "expression": "2 + 2", "apiKey": "sk-secret" },
                "stepId": "task-1-1",
                "toolApprovalId": "approval-1",
            }));

        let replies = agent.handle_message(msg).await.unwrap();
        assert_eq!(replies[0].msg_type, "tool_result");

        let invocations = store.list_invocations(Some(10)).unwrap();
        assert_eq!(invocations.len(), 1);
        assert_eq!(invocations[0].task_id.as_deref(), Some("task-1"));
        assert_eq!(invocations[0].step_id.as_deref(), Some("task-1-1"));
        assert_eq!(invocations[0].approval_id.as_deref(), Some("approval-1"));
        assert_eq!(invocations[0].tool_name, "calculator");
        assert!(invocations[0].success);
        assert_eq!(
            invocations[0].args_summary["apiKey"],
            serde_json::json!("[redacted]")
        );
        assert_eq!(
            replies[0].context["toolInvocationId"],
            serde_json::json!(invocations[0].id)
        );
    }

    #[test]
    fn test_expression_parser() {
        assert!((eval_expression("2+3*4").unwrap() - 14.0).abs() < 0.001);
        assert!((eval_expression("(2+3)*4").unwrap() - 20.0).abs() < 0.001);
        assert!((eval_expression("10/2+3").unwrap() - 8.0).abs() < 0.001);
    }

    /// 测试 — 调用未注册工具应返回错误
    /// 验证：调用不存在的工具名称返回 ToolNotFound 错误
    #[test]
    fn test_call_unregistered_tool_errors() {
        let registry = ToolRegistry::new();

        let result = registry.call("non_existent", serde_json::json!({}));
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("未注册"));
    }

    /// 测试 — has_tool 正确判断工具是否存在
    /// 验证：已注册的工具返回 true，未注册的返回 false
    #[test]
    fn test_has_tool_check() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);

        assert!(registry.has_tool("calculator"));
        assert!(registry.has_tool("datetime"));
        assert!(!registry.has_tool("nonexistent"));
    }

    /// 测试 — list_tools 返回所有已注册工具的描述
    /// 验证：注册后的工具列表包含所有内置工具
    #[test]
    fn test_list_all_tools() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);

        let tools = registry.list_tools();
        assert_eq!(tools.len(), 4); // calculator, datetime, file_read, web_search

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"calculator"));
        assert!(names.contains(&"datetime"));
        assert!(names.contains(&"file_read"));
        assert!(names.contains(&"web_search"));
    }

    /// 测试 — 计算器边界值测试
    /// 验证：处理除零错误和括号不匹配
    #[test]
    fn test_calculator_edge_cases() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);

        // 除零应返回错误
        let result = registry.call("calculator", serde_json::json!({"expression": "1/0"}));
        assert!(result.is_err());

        // 括号不匹配应返回错误
        let result = registry.call("calculator", serde_json::json!({"expression": "(1+2"}));
        assert!(result.is_err());
    }

    /// 测试 — Tool agent 自动选择工具：搜索关键词
    /// 验证：内容包含"搜索"时自动调用 web_search 工具
    #[tokio::test]
    async fn test_tool_agent_auto_select_search() {
        let mut agent = ToolAgent::default();

        // 内容包含"搜索"，应自动使用 web_search
        let msg = AgentMessage::new("User", "Tool", "搜索 Rust 编程教程");
        let replies = agent.handle_message(msg).await.unwrap();

        assert_eq!(replies[0].msg_type, "tool_result");
        // web_search 返回的结果应包含搜索结果标记
        assert!(replies[0].content.contains("web_search"));
    }

    /// 测试 — Tool agent 自动选择工具：时间关键词
    /// 验证：内容包含"时间"时自动调用 datetime 工具
    #[tokio::test]
    async fn test_tool_agent_auto_select_datetime() {
        let mut agent = ToolAgent::default();

        let msg = AgentMessage::new("User", "Tool", "现在是什么时间");
        let replies = agent.handle_message(msg).await.unwrap();

        assert_eq!(replies[0].msg_type, "tool_result");
        assert!(replies[0].content.contains("datetime"));
    }

    /// 测试 — 通过 context 显式指定工具名称
    /// 验证：context 中提供 "tool" 字段可以覆盖自动选择
    #[tokio::test]
    async fn test_tool_agent_explicit_tool_selection() {
        let mut agent = ToolAgent::default();

        let msg =
            AgentMessage::new("User", "Tool", "计算一些东西").with_context(serde_json::json!({
                "tool": "datetime",
                "args": { "format": "readable" }
            }));

        let replies = agent.handle_message(msg).await.unwrap();

        assert_eq!(replies[0].msg_type, "tool_result");
        // 应包含日期（"年月日"格式）
        assert!(replies[0].content.contains("年"));
    }

    #[test]
    fn test_infer_file_read_args_from_content() {
        let args = infer_file_read_args_from_content("读取文件 README.md").unwrap();
        assert_eq!(args["path"], serde_json::json!("README.md"));

        let args = infer_file_read_args_from_content("path: src-tauri/src/main.rs").unwrap();
        assert_eq!(args["path"], serde_json::json!("src-tauri/src/main.rs"));

        let args = infer_file_read_args_from_content("读取文件 .env").unwrap();
        assert_eq!(args["path"], serde_json::json!(".env"));

        assert!(infer_file_read_args_from_content("读取文件").is_none());
    }

    /// 测试 — ToolDescription 结构体字段
    /// 验证：ToolDescription 所有字段正确设置
    #[test]
    fn test_tool_description_fields() {
        let desc = ToolDescription {
            name: "test_tool".to_string(),
            description: "测试工具".to_string(),
            parameters: serde_json::json!({ "type": "object" }),
        };

        assert_eq!(desc.name, "test_tool");
        assert_eq!(desc.description, "测试工具");
        assert_eq!(desc.parameters["type"], "object");
    }

    /// 测试 — datetime 工具不同格式输出
    /// 验证：支持的三种格式（iso、readable、timestamp）都正常
    #[test]
    fn test_datetime_formats() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);

        // iso 格式
        let r1 = registry
            .call("datetime", serde_json::json!({"format": "iso"}))
            .unwrap();
        assert!(r1["datetime"].as_str().unwrap().contains("T"));

        // readable 格式
        let r2 = registry
            .call("datetime", serde_json::json!({"format": "readable"}))
            .unwrap();
        assert!(r2["datetime"].as_str().unwrap().contains("年"));

        // timestamp 格式
        let r3 = registry
            .call("datetime", serde_json::json!({"format": "timestamp"}))
            .unwrap();
        assert!(r3["datetime"].as_str().unwrap().parse::<i64>().is_ok());
    }

    /// 测试 — 自定义工具注册与调用
    /// 验证：通过 ToolRegistry::register 注册的自定义工具可以正常调用
    #[test]
    fn test_custom_tool_registration() {
        let mut registry = ToolRegistry::new();

        // 注册一个自定义工具：将输入转为大写
        registry.register(
            ToolDescription {
                name: "uppercase".to_string(),
                description: "将文本转为大写".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string" }
                    },
                    "required": ["text"]
                }),
            },
            |args| {
                let text = args["text"].as_str().ok_or("缺少 text 参数")?;
                Ok(serde_json::json!({ "result": text.to_uppercase() }))
            },
        );

        // 调用自定义工具
        let result = registry
            .call("uppercase", serde_json::json!({ "text": "hello world" }))
            .unwrap();

        assert_eq!(result["result"], "HELLO WORLD");
    }

    /// 测试 — 空注册表初始状态
    /// 验证：ToolRegistry::new() 创建的注册表为空且可扩展
    #[test]
    fn test_empty_registry() {
        let registry = ToolRegistry::new();
        assert!(registry.list_tools().is_empty());
        assert!(!registry.has_tool("anything"));
    }

    /// 测试 — ToolAgent 计数器
    /// 验证：每次调用后 counter 递增
    #[tokio::test]
    async fn test_tool_agent_counter_increment() {
        let mut agent = ToolAgent::default();

        let msg1 = AgentMessage::new("User", "Tool", "2+2")
            .with_context(serde_json::json!({"tool": "calculator"}));
        agent.handle_message(msg1).await.unwrap();
        assert_eq!(agent.count, 1);

        let msg2 = AgentMessage::new("User", "Tool", "3+3")
            .with_context(serde_json::json!({"tool": "calculator"}));
        agent.handle_message(msg2).await.unwrap();
        assert_eq!(agent.count, 2);
    }
}
