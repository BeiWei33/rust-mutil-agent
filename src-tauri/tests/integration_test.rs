//! 多 Agent 协同系统集成测试
//!
//! 模拟完整的协同流程：Planner → Executor → Memory
//! 验证各 Agent 之间通过 MessageBus 通信的端到端流程。
//!
//! 注意：此测试使用 MemoryAgent 的短期记忆，不依赖外部数据库。

/// 导入被测试模块所需的类型
use rust_mutil_agent::agent::{
    Agent, AgentMessage, EchoAgent, ExecutorAgent, MemoryAgent, PlannerAgent, ToolAgent,
};
use rust_mutil_agent::bus::MessageBus;
use std::sync::Arc;

// ============================================================
// 集成测试 1：完整协同流程 — 搜索任务
// ============================================================

/// 测试 — 模拟搜索任务的完整协同流程
///
/// 流程：
/// 1. 用户发送搜索请求给 PlannerAgent
/// 2. PlannerAgent 生成计划（Tool → Executor → Memory）
/// 3. ToolAgent 执行模拟搜索
/// 4. ExecutorAgent 整理结果
/// 5. MemoryAgent 存储记忆
#[tokio::test]
async fn test_full_coop_flow_search_task() {
    // ---------- 初始化基础设施 ----------
    // 创建消息总线
    let bus = Arc::new(MessageBus::new());
    // 订阅总线以接收所有广播消息（模拟系统监听）
    let _rx = bus.subscribe();
    let tx = bus.sender();

    // ---------- 创建所有 Agent ----------
    let mut planner = PlannerAgent::new();
    let mut tool_agent = ToolAgent::default();
    let mut executor = ExecutorAgent::new();
    let mut memory = MemoryAgent::new();

    // ---------- 第 1 步：用户向 Planner 发送搜索任务 ----------
    let user_msg = AgentMessage::new("User", "Planner", "帮我搜索 Rust 编程教程");
    let plan_responses = planner.handle_message(user_msg).await.unwrap();

    // 验证：Planner 生成了计划（1 回复 + 3 分派 = 4 条消息）
    assert_eq!(plan_responses.len(), 4, "Planner 应为搜索任务生成 3 步骤计划");
    assert_eq!(plan_responses[0].msg_type, "plan_created");

    // 提取 task_id 用于后续追踪
    let task_id = plan_responses[0]
        .task_id
        .clone()
        .expect("计划应包含 task_id");

    // 通过总线发布所有回复消息
    for reply in plan_responses {
        let _ = tx.send(reply);
    }

    // ---------- 第 2 步：ToolAgent 接收并执行搜索 ----------
    let tool_msg = AgentMessage::new("Planner", "Tool", "搜索相关信息: 帮我搜索 Rust 编程教程")
        .with_type("plan_step")
        .with_task_id(&task_id);

    let tool_responses = tool_agent.handle_message(tool_msg).await.unwrap();
    assert!(!tool_responses.is_empty(), "ToolAgent 应返回搜索结果");
    assert_eq!(tool_responses[0].msg_type, "tool_result");

    // 验证搜索结果包含搜索关键词
    assert!(
        tool_responses[0].content.contains("web_search")
            || tool_responses[0].content.contains("search"),
        "搜索结果应包含工具名称"
    );

    // 发布结果到总线
    for reply in tool_responses {
        let _ = tx.send(reply);
    }

    // ---------- 第 3 步：ExecutorAgent 整理分析结果 ----------
    let exec_msg = AgentMessage::new("Planner", "Executor", "整理和分析搜索结果: 帮我搜索 Rust 编程教程")
        .with_type("plan_step")
        .with_task_id(&task_id);

    let exec_responses = executor.handle_message(exec_msg).await.unwrap();
    assert!(!exec_responses.is_empty(), "ExecutorAgent 应返回执行结果");
    assert_eq!(exec_responses[0].msg_type, "execution_result");

    // 发布结果
    for reply in exec_responses {
        let _ = tx.send(reply);
    }

    // ---------- 第 4 步：MemoryAgent 存储结果 ----------
    let memory_msg = AgentMessage::new("Planner", "Memory", "存储结果到知识库: 帮我搜索 Rust 编程教程")
        .with_type("plan_step")
        .with_task_id(&task_id);

    let memory_responses = memory.handle_message(memory_msg).await.unwrap();
    assert!(!memory_responses.is_empty(), "MemoryAgent 应返回存储确认");
    assert_eq!(memory_responses[0].msg_type, "memory_ack");

    // ---------- 验证：MemoryAgent 短期记忆中有存储的内容 ----------
    assert!(!memory.recent_turns(1).is_empty(), "MemoryAgent 短期记忆应有记录");
}

// ============================================================
// 集成测试 2：完整协同流程 — 编码任务
// ============================================================

/// 测试 — 模拟编码任务的完整协同流程
///
/// 流程：
/// 1. 用户发送编码请求给 PlannerAgent
/// 2. PlannerAgent 生成计划（Planner → Executor）
/// 3. ExecutorAgent 执行编码
#[tokio::test]
async fn test_full_coop_flow_coding_task() {
    // ---------- 初始化 ----------
    let bus = Arc::new(MessageBus::new());
    let _rx = bus.subscribe();
    let tx = bus.sender();

    let mut planner = PlannerAgent::new();
    let mut executor = ExecutorAgent::new();

    // ---------- 第 1 步：Planner 生成编码计划 ----------
    let user_msg = AgentMessage::new("User", "Planner", "写一个 Python 排序函数");
    let plan_responses = planner.handle_message(user_msg).await.unwrap();

    // 编码任务应有 1 + 2 = 3 条消息
    assert_eq!(plan_responses.len(), 3, "编码任务应生成 2 步骤计划");
    assert_eq!(plan_responses[0].msg_type, "plan_created");

    let task_id = plan_responses[0].task_id.clone().unwrap();

    for reply in plan_responses {
        let _ = tx.send(reply);
    }

    // ---------- 第 2 步：Executor 执行编码 ----------
    let exec_msg = AgentMessage::new("Planner", "Executor", "编写代码: 写一个 Python 排序函数")
        .with_type("plan_step")
        .with_task_id(&task_id);

    let exec_responses = executor.handle_message(exec_msg).await.unwrap();

    assert!(!exec_responses.is_empty());
    assert_eq!(exec_responses[0].msg_type, "execution_result");
    assert!(exec_responses[0].content.contains("已执行"));

    for reply in exec_responses {
        let _ = tx.send(reply);
    }
}

// ============================================================
// 集成测试 3：Echo Agent 集成
// ============================================================

/// 测试 — EchoAgent 作为消息通信验证器
///
/// 验证：
/// 1. EchoAgent 可以接收 Planner 发送的消息
/// 2. EchoAgent 正确回显并计数
#[tokio::test]
async fn test_echo_agent_integration() {
    let mut echo = EchoAgent::new();

    // 模拟 Planner 发送给 Echo 的消息
    for i in 1..=3 {
        let msg = AgentMessage::new("Planner", "Echo", &format!("处理请求 {i}"));
        let replies = echo.handle_message(msg).await.unwrap();

        assert_eq!(replies.len(), 1);
        assert!(replies[0].content.contains(&format!("#{i}")));
        assert_eq!(replies[0].msg_type, "echo_reply");
    }
}

// ============================================================
// 集成测试 4：MemoryAgent 长期记忆集成
// ============================================================

/// 测试 — MemoryAgent 的存储与检索集成
///
/// 验证：
/// 1. 不同 Agent 可以通过 MemoryAgent 共享记忆
/// 2. 关键词检索可以找到之前存储的信息
#[tokio::test]
async fn test_memory_agent_shared_context() {
    let mut memory = MemoryAgent::new();

    // Executor 存储执行结果
    let exec_store = AgentMessage::new("Executor", "Memory", "任务 A 完成：生成了 500 行代码")
        .with_type("store")
        .with_task_id("task-a");
    memory.handle_message(exec_store).await.unwrap();

    // Tool 存储工具调用结果
    let tool_store = AgentMessage::new("Tool", "Memory", "搜索任务 B：找到 10 条相关结果")
        .with_type("store")
        .with_task_id("task-b");
    memory.handle_message(tool_store).await.unwrap();

    // Planner 检索 Executor 的执行记录
    let query = AgentMessage::new("Planner", "Memory", "代码")
        .with_type("recall");
    let replies = memory.handle_message(query).await.unwrap();

    assert_eq!(replies[0].msg_type, "memory_retrieved");
    assert!(
        replies[0].content.contains("代码"),
        "检索结果应包含 Executor 存储的信息"
    );

    // 查看上下文
    let ctx = &replies[0].context;
    assert!(ctx["count"].as_u64().unwrap() >= 1);
}

// ============================================================
// 集成测试 5：多 Agent 通过总线并发通信
// ============================================================

/// 测试 — 多个 Agent 通过 MessageBus 进行发布/订阅通信
///
/// 验证：
/// 1. 多个 Agent 各自独立处理消息
/// 2. 消息发布后所有订阅者都能收到
#[tokio::test]
async fn test_multi_agent_bus_communication() {
    // 初始化总线
    let bus = Arc::new(MessageBus::new());
    let mut rx1 = bus.subscribe();
    let mut rx2 = bus.subscribe();

    let mut echo = EchoAgent::new();

    // EchoAgent 处理一条消息
    let msg = AgentMessage::new("Orchestrator", "Echo", "ping");
    let echo_replies = echo.handle_message(msg).await.unwrap();

    assert_eq!(echo_replies.len(), 1);
    assert!(echo_replies[0].content.contains("Echo #1"));

    // 通过总线发布 Echo 的回复
    bus.publish(echo_replies[0].clone()).unwrap();

    // 两个订阅者都应收到
    let r1 = rx1.recv().await.unwrap();
    let r2 = rx2.recv().await.unwrap();

    assert_eq!(r1.content, r2.content);
    assert!(r1.content.contains("Echo #1"));
}

// ============================================================
// 集成测试 6：任务 Plan → Execute → Store 完整链路
// ============================================================

/// 测试 — 模拟完整的"搜索→执行→存储"任务链路
///
/// 端到端验证多 Agent 协同处理的完整生命周期。
#[tokio::test]
async fn test_end_to_end_task_lifecycle() {
    // ---------- 初始化 ----------
    let mut planner = PlannerAgent::new();
    let mut tool = ToolAgent::default();
    let mut executor = ExecutorAgent::new();
    let mut memory = MemoryAgent::new();

    // ---------- 1. 用户提交搜索任务 ----------
    let user_input = "搜索今天的 AI 新闻";
    let plan_result = planner
        .handle_message(AgentMessage::new("User", "Planner", user_input))
        .await
        .unwrap();

    // 提取 task_id
    let task_id = plan_result[0].task_id.clone().unwrap();

    // 提取分派给 Tool 的步骤
    let tool_step = plan_result
        .iter()
        .find(|m| m.to == "Tool")
        .expect("计划应包含 Tool 步骤");
    let tool_reply = tool
        .handle_message(tool_step.clone())
        .await
        .unwrap();
    assert_eq!(tool_reply[0].msg_type, "tool_result");

    // 提取分派给 Executor 的步骤
    let exec_step = plan_result
        .iter()
        .find(|m| m.to == "Executor")
        .expect("计划应包含 Executor 步骤");
    let exec_reply = executor
        .handle_message(exec_step.clone())
        .await
        .unwrap();
    assert_eq!(exec_reply[0].msg_type, "execution_result");

    // 提取分派给 Memory 的步骤
    let memory_step = plan_result
        .iter()
        .find(|m| m.to == "Memory")
        .expect("计划应包含 Memory 步骤");
    let memory_reply = memory
        .handle_message(memory_step.clone())
        .await
        .unwrap();
    assert_eq!(memory_reply[0].msg_type, "memory_ack");

    // ---------- 验证：Memory 中有记录 ----------
    let recall = memory
        .handle_message(
            AgentMessage::new("User", "Memory", &task_id)
                .with_type("recall"),
        )
        .await
        .unwrap();
    // 至少应有一条记录（与 task_id 相关的）
    assert!(!memory.recent_turns(10).is_empty());
}

// ============================================================
// 集成测试 7：AgentMessage 的构造和 reply_to 链
// ============================================================

/// 测试 — AgentMessage 的构造方法和 reply_to 链
///
/// 验证：
/// 1. with_type / with_context / with_task_id 的链式调用
/// 2. reply_to 方法正确翻转 from/to
#[test]
fn test_agent_message_chain_construction() {
    let msg = AgentMessage::new("Alice", "Bob", "你好")
        .with_type("greeting")
        .with_task_id("task-1")
        .with_context(serde_json::json!({ "lang": "zh" }));

    assert_eq!(msg.from, "Alice");
    assert_eq!(msg.to, "Bob");
    assert_eq!(msg.msg_type, "greeting");
    assert_eq!(msg.task_id, Some("task-1".to_string()));
    assert_eq!(msg.context["lang"], "zh");

    // reply_to 翻转方向
    let reply = msg.reply_to("你好，Alice！");
    assert_eq!(reply.from, "Bob");
    assert_eq!(reply.to, "Alice");
    assert_eq!(reply.msg_type, "reply");
    assert_eq!(reply.task_id, Some("task-1".to_string()));
    assert_eq!(reply.content, "你好，Alice！");
}
