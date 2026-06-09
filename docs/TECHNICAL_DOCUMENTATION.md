# 多 Agent 协同智能体技术文档

## 1. 项目概览

本项目是一个基于 Rust 与 Tauri v2 的跨平台桌面应用，用于构建“多 Agent 协同智能体”运行时。系统由 React 前端提供聊天工作台、Agent 状态面板和设置面板，由 Rust 后端负责 Agent 注册、任务分发、消息通信、工具调用、记忆管理和 Tauri IPC 命令。

当前代码处于可运行原型阶段：Agent 框架、前后端通信、状态展示、聊天多会话界面、工具注册表、短期记忆、MemoryAgent SQLite 长期记忆、结构化 KnowledgeBase 存储/检索、Planner 长期经验注入、ReviewAgent 结构化审查、EvolutionAgent 经验建议、任务级 `evolutionNote` artifact、LLM 客户端、项目理解、聊天历史持久化、任务/事件持久化、依赖调度式任务闭环、步骤超时、受控验证命令执行、命令运行审计、ToolAgent 调用审计、审批请求基础、非 allowlist 命令审批入口、已审批命令执行、命令审批等待/恢复、已审批命令结果回写、补丁提案持久化、diff 审批预览、已审批补丁手动应用、应用后自动验证、已应用补丁安全回滚、可配置默认的验证失败自动回滚、补丁审批等待/恢复、通用 `tool.*` 工具审批等待/恢复、legacy `file_read` / `web_search` 自动审批拦截和 workspace 路径沙箱、任务 command/patch/verification/revert/tool/review/evolution artifact、验证失败任务/步骤状态回写、补丁验证失败 `FailureCase`、验证通过/跳过补丁 `ProjectFact` 和验证失败 `reworkSuggestion` 经验/返工上下文已具备；更多工具权限、自动返工执行器和写入型工具权限仍待完善。

## 2. 技术栈

| 层级 | 技术 |
| --- | --- |
| 桌面容器 | Tauri v2 |
| 后端语言 | Rust 2021 |
| 异步运行时 | Tokio |
| 前端框架 | React 18 + Vite |
| 前端状态管理 | Zustand |
| 样式系统 | Tailwind CSS |
| IPC | Tauri `invoke` 命令 |
| 序列化 | serde / serde_json / rmp-serde |
| 数据库 | SQLite via rusqlite |
| HTTP 客户端 | reqwest |
| 日志 | tracing / tracing-subscriber |
| 测试 | Rust unit/integration tests, Vitest + Testing Library |

## 3. 目录结构

```text
rust-mutil-agent/
├── README.md
├── .env.example
├── docs/
│   └── TECHNICAL_DOCUMENTATION.md
├── plugins/
├── scripts/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── tests/
│   │   └── integration_test.rs
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── commands.rs
│       ├── error.rs
│       ├── agent/
│       ├── approval.rs
│       ├── bus/
│       ├── llm/
│       ├── memory/
│       ├── orchestrator/
│       ├── runtime/
│       ├── tool/
│       └── workspace/
└── src-web/
    ├── package.json
    ├── vite.config.ts
    ├── tailwind.config.js
    └── src/
        ├── App.tsx
        ├── main.tsx
        ├── components/
        ├── lib/
        ├── store/
        ├── types/
        └── __tests__/
```

## 4. 总体架构

```text
React UI
  ├─ ChatWindow: 聊天、Agent 选择、发送/重试
  ├─ MemoryPanel: 长期知识写入与检索
  ├─ AgentPanel: Agent 列表、状态、能力展示
  └─ SettingsPanel: 模型/API Key/参数设置
        │
        │ Tauri invoke
        ▼
Rust Tauri Commands
  ├─ send_message
  ├─ list_agents
  ├─ get_agent_status
  ├─ create_task / get_task / list_tasks / get_task_events
  ├─ get_task_result
  ├─ cancel_task / retry_task
  ├─ get_project_snapshot / list_project_files / read_project_file / search_project_text
  ├─ run_project_command / request_project_command_approval / request_tool_action_approval
  ├─ run_approved_project_command
  ├─ list_project_command_runs
  ├─ store_knowledge / search_knowledge
  ├─ create_patch_proposal / list_patch_proposals / get_patch_proposal
  ├─ apply_approved_patch / revert_applied_patch
  ├─ list_approval_requests / approve_action
  ├─ health_check
  ├─ get_history
  └─ clear_history
        │
        ▼
Orchestrator
  ├─ 注册内置 Agent
  ├─ 维护 Agent mpsc 通道
  ├─ 创建可追踪 Task / TaskEvent
  └─ 按步骤依赖分派计划任务，支持取消后重试
        │
        ▼
Agents
  ├─ Planner
  ├─ Executor
  ├─ Memory
  ├─ Tool
  └─ Echo
        │
        ▼
MessageBus
  └─ Tokio broadcast，用于 Agent 回复和广播消息
```

## 5. 后端启动流程

入口文件：`src-tauri/src/main.rs`

1. 初始化 `tracing_subscriber`，默认日志级别为 `info`。
2. 通过 `dotenvy::dotenv()` 尝试加载 `.env`。
3. 创建全局 `MessageBus`。
4. 初始化聊天历史、结构化知识库、任务/事件、命令审计、工具调用审计、审批请求和补丁提案 SQLite store。
5. 创建 `Orchestrator`，注入审批 store、工具调用审计 store 和 `MEMORY_DB_PATH`，再调用 `register_builtin_agents()` 注册内置 Agent。
6. `MemoryAgent` 注册时会尝试连接长期记忆 SQLite；初始化失败会记录 warning 并回退到短期记忆。
7. 将 `AppState` 注入 Tauri 状态并注册 Tauri IPC 命令。
8. 注册 Tauri 插件：shell、fs、notification、dialog、clipboard-manager、process。
9. 启动 Tauri 应用。

## 6. 核心后端模块

### 6.1 Agent 抽象

位置：`src-tauri/src/agent/traits.rs`

核心 trait：

```rust
#[async_trait]
pub trait Agent: Send + Sync {
    fn name(&self) -> &str;
    fn capabilities(&self) -> Vec<Capability>;
    async fn handle_message(&mut self, msg: AgentMessage)
        -> Result<Vec<AgentMessage>, AgentError>;
}
```

`AgentMessage` 是 Agent 间的统一消息格式，包含：

| 字段 | 说明 |
| --- | --- |
| `id` | 消息 ID |
| `from` | 发送方 Agent |
| `to` | 接收方 Agent |
| `content` | 消息正文 |
| `context` | JSON 结构化上下文 |
| `task_id` | 任务 ID |
| `reply_id` | 回复关联 ID，当前预留 |
| `msg_type` | 消息类型，如 `plan_request`、`plan_step`、`tool_result` |

### 6.2 Orchestrator 调度器

位置：`src-tauri/src/orchestrator/mod.rs`

职责：

- 注册并启动内置 Agent。
- 维护 Agent 名称到 `mpsc::UnboundedSender<AgentMessage>` 的映射。
- 接收用户任务，生成任务 ID，创建可追踪 `Task`，并维护任务事件。
- 通过 SQLite 保存 `Task` 和 `TaskEvent` JSON 快照，应用重启后恢复任务列表和事件时间线。
- 将消息直接发送给指定 Agent。
- 提供 Agent 列表和任务结果查询。
- 监听 `MessageBus` 中的 Agent 回复，应用 Planner 计划，并按步骤依赖推进任务执行。

当前内置 Agent：

| Runtime 名称 | 前端展示名 | 直接选择 | 职责 |
| --- | --- | --- | --- |
| `Planner` | 协调员/总控 | 是 | 理解需求并生成计划 |
| `Executor` | 执行工程师 | 是 | 执行明确任务、HTTP 请求、模拟搜索 |
| `Memory` | 记忆管理员 | 否 | 短期记忆、SQLite 长期记忆能力 |
| `Tool` | 工具操作员 | 否 | 工具调用 |
| `Echo` | 回声测试员 | 是 | 通信链路测试 |

注意：当前 `submit_task_to_agent()` 会把消息发到 Agent 的 `mpsc` 通道，Agent 回复会发送到 `MessageBus` 的 broadcast 通道；Orchestrator 已订阅回复并更新任务状态。Planner 生成计划后，Orchestrator 会按 `depends_on` 启动就绪步骤：Planner 分析步骤和只读项目检索在运行时内部完成，Executor/Memory/Echo/通用 Tool 步骤会投递给对应 Agent 的 `mpsc` 通道。

步骤分派上下文会携带 `stepId`、`stepAttempt`、任务目标和依赖步骤结果；Agent 回复会保留并合并该上下文，Orchestrator 优先按 `stepId + stepAttempt` 精确完成或失败对应步骤，缺失时才回退到旧的 Agent 名称匹配。

任务持久化由 `src-tauri/src/task/store.rs` 提供，默认数据库路径是当前工作目录下的 `rust-mutil-agent-tasks.sqlite3`；可通过 `TASK_DB_PATH` 覆盖。持久化初始化失败时，应用会记录 warning 并退回内存任务运行时。

### 6.3 MessageBus 消息总线

位置：`src-tauri/src/bus/message_bus.rs`

实现基于 `tokio::sync::broadcast`，默认缓冲区大小为 1024。

能力：

- `publish(msg)`：向所有订阅者广播消息；没有订阅者时返回错误，避免静默丢消息。
- `subscribe()`：创建广播订阅者。
- `sender()`：返回 broadcast sender，供 Agent 回复。
- `subscriber_count()`：返回当前订阅者数量。

### 6.4 PlannerAgent

位置：`src-tauri/src/agent/planner_agent.rs`

职责：把用户目标拆解为 `TaskPlan` 和 `PlanStep`。

当前默认实现是关键词规则；当 `PLANNER_USE_LLM=true` 时可优先调用 LLM 生成 JSON 计划，解析或调用失败会自动降级到规则规划：

- 包含“搜索/查找/新闻”：生成 `Tool -> Executor -> Memory` 三步计划。
- 包含“代码/编程/写”：生成 `Planner -> Executor` 两步计划。
- 其他请求：生成 `Echo` 单步计划。

输出消息：

- 第一条为 `plan_created`，`context` 内包含完整 `TaskPlan`。
- 后续仍会广播 `plan_step` 作为兼容消息；实际执行分派由 Orchestrator 根据 `TaskPlan.depends_on` 统一调度，避免绕过任务状态机。

### 6.5 ExecutorAgent

位置：`src-tauri/src/agent/executor_agent.rs`

职责：

- 执行通用任务，返回执行确认。
- 对 `msg_type == "http_request"` 的消息发起 HTTP 请求。
- 对包含“搜索/search”的消息执行模拟搜索。

当前搜索是占位实现，会等待约 200ms 并返回模拟结果。ExecutorAgent 仍不直接执行系统命令；真实项目验证命令通过独立的 `runtime::command` 受控模块和 `run_project_command` IPC 暴露。

### 6.6 MemoryAgent

位置：`src-tauri/src/agent/memory_agent.rs`

职责：

- 维护最多 50 条短期记忆。
- 支持 `store` / `remember` 写入记忆。
- 支持 `query` / `recall` / `retrieve` 通过关键词检索短期记忆，并在短期结果不足时查询 SQLite 长期记忆补足结果。
- 支持通过 `with_database(db_path)` 初始化 SQLite 长期记忆表，表结构包括 `conversations` 和 `knowledge`。
- 自动存储模式会同时写入短期记忆和长期记忆；显式 `store` / `remember` 也会在数据库可用时落盘。

应用启动时读取 `MEMORY_DB_PATH`，默认使用 `rust-mutil-agent-memory.sqlite3`。Orchestrator 注册内置 Agent 时会优先使用 `MemoryAgent::with_database()`；数据库初始化失败时记录 warning，并回退到 `MemoryAgent::new()` 的短期记忆模式。

同一数据库路径也会初始化 `KnowledgeBase`，并通过 `store_knowledge` / `search_knowledge` IPC 暴露结构化知识条目。条目包含标题、内容、来源、标签和创建时间，可作为 ProjectFact、FailureCase、SuccessPattern 等经验模型的存储基础；补丁自动验证失败时，后端会把失败命令、验证错误、回滚状态和后续处理建议写入 `FailureCase` 条目，验证通过或跳过时会把变更摘要、文件列表和验证命令结果写入补丁级 `ProjectFact`；创建 Planner 任务时，命令层会检索用户目标、`ProjectFact` 和 `FailureCase`，去重后以 `knowledgeContext` 注入规划上下文，规则 Planner 会把经验摘要写入计划步骤，LLM Planner 会在安全上下文中看到这些条目；数据库初始化失败时会回退到内存知识库。

### 6.7 ToolAgent 与 ToolRegistry

位置：`src-tauri/src/agent/tool_agent.rs`

`ToolRegistry` 支持注册工具描述和工具函数：

```rust
pub type ToolFn = Arc<
    dyn Fn(serde_json::Value) -> Result<serde_json::Value, String> + Send + Sync,
>;
```

内置工具：

| 工具 | 状态 | 说明 |
| --- | --- | --- |
| `calculator` | 已实现 | 支持基本四则运算和括号 |
| `datetime` | 已实现 | 支持 `iso`、`readable`、`timestamp` |
| `file_read` | 已实现 | 读取 workspace 内普通文本文件，复用只读文件 API 的路径逃逸、敏感文件和大小限制 |
| `web_search` | 占位 | 返回模拟搜索结果 |

`ToolAgent` 可通过 `context.tool` 显式指定工具；未指定时会根据消息内容关键词推断工具。

通用工具审批状态流已通过 `request_tool_action_approval` 和 `approve_action` 接入调度器：`tool.*` 审批可让关联任务/步骤进入 `waitingApproval`，通过后恢复运行，拒绝后标记失败。调度器已能在 Tool 步骤即将调用 legacy `file_read` 或 `web_search` 时自动创建 `tool.fileRead` / `tool.webSearch` 审批，并在审批通过后恢复原 Tool 步骤投递；`file_read` 本身复用 workspace 只读文件 API 的路径沙箱。ToolAgent 调用会写入 `tool_invocations` SQLite 审计表，参数摘要会脱敏和截断，并可通过 `list_tool_invocations` 查询。后续还需要把同类机制扩展到更多工具。

### 6.8 LLMClient

位置：`src-tauri/src/llm/mod.rs`

提供统一 LLM 类型和请求/响应结构，支持枚举：

- `OpenAI`
- `DeepSeek`
- `LlamaCpp`
- `Custom(String)`
- `Mock`

当前 `Mock`、OpenAI、DeepSeek 和 Custom OpenAI-compatible 端点已实现 `chat_completion()`；OpenAI/DeepSeek 缺少 API Key 时会本地返回可解释错误，不发起网络请求。`LlamaCpp` 仍是预留 provider，尚未接入本地服务调用。

代码中已有 OpenAI 和 DeepSeek 默认端点与模型配置：

- OpenAI 默认端点：`https://api.openai.com/v1/chat/completions`
- OpenAI 默认模型：`gpt-4o`

- 默认端点：`https://api.deepseek.com/v1/chat/completions`
- 默认模型：`deepseek-v4-pro`

`ChatCompletionRequest` 支持 `response_format`，其中 `JsonObject` 会映射为 OpenAI-compatible JSON mode。Planner 侧已提供 JSON plan schema 和解析校验入口；通过 `PLANNER_USE_LLM=true` 可启用环境变量 LLM JSON 规划，默认仍使用规则规划作为稳定降级路径。前端传入 API Key 时，后端会通过 `AgentMessage.transient_context` 为本次 Planner 请求构造临时 OpenAI-compatible LLMClient；该临时上下文不参与序列化、持久化或 Planner prompt 的项目上下文拼接。

Planner LLM 相关环境变量：

| 变量 | 说明 |
| --- | --- |
| `PLANNER_USE_LLM` | 设为 `true` / `1` / `yes` / `on` 时启用 LLM 规划 |
| `PLANNER_LLM_PROVIDER` | `openai` 或 `deepseek`，未设置时优先 OpenAI Key，否则 DeepSeek |
| `PLANNER_LLM_RETRIES` | LLM 调用重试次数，范围 1-3 |
| `TASK_DB_PATH` | 任务/事件 SQLite 数据库路径，默认 `rust-mutil-agent-tasks.sqlite3` |
| `CHAT_DB_PATH` | 聊天历史 SQLite 数据库路径，默认 `rust-mutil-agent-chat.sqlite3` |
| `MEMORY_DB_PATH` | MemoryAgent 长期记忆 SQLite 数据库路径，默认 `rust-mutil-agent-memory.sqlite3` |
| `COMMAND_DB_PATH` | 命令运行审计 SQLite 数据库路径，默认 `rust-mutil-agent-commands.sqlite3` |
| `TOOL_INVOCATION_DB_PATH` | ToolAgent 工具调用审计 SQLite 数据库路径，默认 `rust-mutil-agent-tool-invocations.sqlite3` |
| `APPROVAL_DB_PATH` | 审批请求 SQLite 数据库路径，默认 `rust-mutil-agent-approvals.sqlite3` |
| `PATCH_DB_PATH` | 补丁提案 SQLite 数据库路径，默认 `rust-mutil-agent-patches.sqlite3` |
| `PATCH_AUTO_ROLLBACK_ON_VERIFICATION_FAILURE` | 设为 `true` / `1` / `yes` / `on` 时，新补丁审批默认开启验证失败自动回滚；单次 `apply_approved_patch` 请求可覆盖 |

## 7. Tauri IPC 命令契约

位置：`src-tauri/src/commands.rs`，前端封装在 `src-web/src/lib/tauri.ts`。

| 命令 | 入参 | 返回 | 当前状态 |
| --- | --- | --- | --- |
| `send_message` | `{ request: { content, agentId?, routeMode?, sessionId?, llmSettings? } }` | `SendMessageResponse` | 已实现任务提交 |
| `create_task` | `{ request: { content, agentId?, llmSettings? } }` | `CreateTaskResponse` | 已实现软件工程任务创建 |
| `get_task` | `{ taskId }` | `Task` 或 `null` | 已实现 |
| `list_tasks` | 无 | `{ tasks }` | 已实现 |
| `get_task_events` | `{ taskId }` | `TaskEvent[]` | 已实现 |
| `cancel_task` | `{ request: { taskId, reason? } }` | `CancelTaskResponse` | 已实现任务取消 |
| `retry_task` | `{ request: { taskId, reason? } }` | `RetryTaskResponse` | 已实现失败/取消任务重试 |
| `skip_task_step` | `{ request: { taskId, stepId, reason? } }` | `SkipTaskStepResponse` | 已实现单步骤跳过、依赖继续推进和 `stepSkipped` 事件 |
| `get_project_snapshot` | 无 | `ProjectSnapshot` | 已实现 |
| `list_project_files` | `{ maxFiles? }` | `{ files }` | 已实现 |
| `read_project_file` | `{ path }` | `FileReadResponse` | 已实现只读沙箱 |
| `search_project_text` | `{ request: { query, maxResults? } }` | `SearchResponse` | 已实现只读搜索 |
| `run_project_command` | `{ request: { command, workingDir } }` | `ProjectCommandRunResponse` | 已实现 allowlist 受控运行 |
| `request_project_command_approval` | `{ request: { command, workingDir, taskId?, stepId? } }` | `ApprovalRequest` | 已实现非 allowlist 命令审批创建和关联任务/步骤等待审批 |
| `request_tool_action_approval` | `{ request: { taskId?, stepId?, title, reason, actionType, actionPayload?, risk?, requestedBy? } }` | `ApprovalRequest` | 已实现通用 `tool.*` 审批创建和关联任务/步骤等待审批 |
| `run_approved_project_command` | `{ request: { approvalId } }` | `ProjectCommandRunResponse` | 已实现已审批命令执行、审计关联和关联任务结果回写 |
| `list_project_command_runs` | `{ limit? }` | `{ runs }` | 已实现最近命令审计读取 |
| `list_tool_invocations` | `{ limit? }` | `{ invocations }` | 已实现最近 ToolAgent 调用审计读取，参数摘要会脱敏和截断 |
| `store_knowledge` | `{ request: { title, content, source?, tags? } }` | `{ id }` | 已实现结构化长期知识写入 |
| `search_knowledge` | `{ request: { query, limit? } }` | `{ query, items }` | 已实现标题/内容关键词检索，返回来源、标签和创建时间 |
| `create_patch_proposal` | `{ request: { summary, files, taskId?, stepId?, requestedBy? } }` | `{ proposal, approval }` | 已实现补丁提案持久化、diff 审批创建和关联任务/步骤等待审批 |
| `list_patch_proposals` | `{ limit? }` | `{ proposals }` | 已实现最近补丁提案读取 |
| `get_patch_proposal` | `{ patchId }` | `PatchProposal` 或 `null` | 已实现单个补丁提案读取 |
| `apply_approved_patch` | `{ request: { approvalId, autoRollbackOnVerificationFailure? } }` | `PatchApplyResult` | 已实现已审批补丁应用、proposal 状态写回、推荐验证命令自动运行、默认/可覆盖的验证失败自动回滚、关联任务 artifact/event 写回，以及验证失败任务/步骤 failed 标记 |
| `revert_applied_patch` | `{ request: { patchId } }` | `PatchRevertResult` | 已实现已应用补丁安全回滚、proposal `reverted` 状态写回，以及关联任务 `patchReverted` artifact/event 写回 |
| `list_approval_requests` | `{ status?, limit? }` | `{ approvals }` | 已实现审批请求读取 |
| `approve_action` | `{ request: { approvalId, approved, note?, decidedBy? } }` | `ApprovalRequest` 或 `null` | 已实现审批/拒绝决策；命令/补丁/通用 `tool.*` 审批会恢复或失败关联任务/步骤，补丁审批会同步 proposal 状态 |
| `get_agent_status` | `{ agentId }` | `AgentStatusResponse` | 已实现 |
| `list_agents` | 无 | `{ agents }` | 已实现 |
| `get_task_result` | `{ taskId }` | 兼容旧接口的任务结果或 `null` | 已聚合当前任务步骤和输出 |
| `health_check` | 无 | `{ healthy, version, agentCount }` | 已实现 |
| `get_history` | `{ sessionId }` | 消息数组 | 已实现聊天历史读取 |
| `clear_history` | `{ sessionId }` | `void` | 已实现会话历史清理 |

`run_project_command` 只接受归一化后精确匹配的低风险验证命令：

- `src-tauri`: `cargo check`, `cargo test`
- `src-web`: `npm test -- --run`, `npm run build`

该命令不会经过 shell；工作目录会解析到 workspace 内部，拒绝父目录穿越和 shell 控制字符；执行超时为 120 秒，stdout/stderr 会截断到前 96 KB 并返回截断标记。

每次成功进入 allowlist 的命令运行都会写入 `command_runs` 审计表，保存命令、工作目录、退出码、是否成功、stdout/stderr、耗时、超时标记、截断标记、创建时间和可选 `approval_id`。前端项目面板会通过 `list_project_command_runs` 展示最近记录。ToolAgent 每次真实工具调用会写入 `tool_invocations` 审计表，保存任务/步骤、approvalId、工具名、脱敏参数摘要、成功状态、错误、耗时和创建时间；前端项目面板通过 `list_tool_invocations` 展示最近记录。

非 allowlist 命令不会直接执行。前端项目面板可调用 `request_project_command_approval` 创建高风险审批请求；后端会复用命令解析逻辑，仍然拒绝空命令、shell 控制字符、父目录穿越和 workspace 外目录。审批 payload 记录归一化命令、工作目录、默认 allowlist 判定和可选 `taskId` / `stepId`。

审批请求由 `approval_requests` 表持久化，包含任务/步骤关联、风险等级、动作类型、动作 payload、请求方、状态和决策信息。当前已支持 `pending` / `approved` / `rejected` / `cancelled` 状态、列表筛选、重复决策保护、非 allowlist 命令手动审批、补丁提案审批和通用 `tool.*` 工具审批。命令审批创建时会把关联任务/步骤置为 `waitingApproval`，写入 `commandApproval` artifact 并生成 `approvalRequested` 事件；命令审批通过会把关联任务/步骤恢复到运行态，拒绝会标记为 `failed`，并写入 `commandApprovalResolved` artifact 与 `approvalResolved` 事件。审批通过后，前端审批面板可调用 `run_approved_project_command` 按 approvalId 执行原 payload 中的项目命令，结果会写入命令审计和关联任务的 `commandRun` artifact；执行通过会完成关联步骤，失败会把关联任务/步骤标记为 `failed`，便于 `retry_task` 重新调度。补丁提案创建审批时也会把关联任务/步骤置为 `waitingApproval`，写入 `patchApproval` artifact 并生成 `approvalRequested` 事件；补丁审批通过会把关联任务/步骤恢复到运行态，拒绝会标记为 `failed`，并写入 `patchApprovalResolved` artifact 与 `approvalResolved` 事件。审批通过后，也可调用 `apply_approved_patch` 应用原审批 payload 关联的补丁提案，并在已应用后调用 `revert_applied_patch` 回滚补丁。通用 `tool.*` 审批创建时会写入 `toolApproval` artifact 并生成 `approvalRequested` 事件；审批通过会恢复等待中的关联任务/步骤，拒绝会标记为 `failed`，并写入 `toolApprovalResolved` artifact 与 `approvalResolved` 事件。自动拦截的 legacy `file_read` 和 `web_search` 审批 payload 会保存原 Tool dispatch，上述审批通过后会恢复投递原步骤；实际文件读取会复用 workspace 只读 API，拒绝父目录穿越、绝对路径、敏感文件和超大文件；工具调用结果会写入 `tool_invocations` 审计表并关联 approvalId。后端不会接受前端重新传入命令文本或补丁内容。补丁应用结果会在关联任务上生成 `patchApplied` artifact 和 `artifactCreated` 事件，首次应用成功后还会自动运行推荐验证命令，写入命令审计和 `patchVerification` artifact；验证通过或跳过会完成关联步骤，验证失败会把关联任务/步骤标记为 `failed` 并生成失败事件，便于 `retry_task` 重新调度。`PATCH_AUTO_ROLLBACK_ON_VERIFICATION_FAILURE` 控制新补丁审批的失败回滚默认值，调用方也可显式传入 `autoRollbackOnVerificationFailure` 覆盖；触发时会执行安全回滚，并把 `autoRollback` 写入 `patchVerification` artifact，同时生成 `patchReverted` artifact 和事件。通用工具审批状态流、`file_read` 路径沙箱和工具调用审计已具备，更多 ToolAgent 工具拦截仍待扩展。

补丁提案由 `src-tauri/src/workspace/patch.rs` 提供，持久化到 `patch_proposals` 表。`create_patch_proposal` 当前支持修改 workspace 内已有文本文件：后端会拒绝父目录穿越、workspace 外路径、受保护目录、密钥文件、空变更和基线内容不一致的请求；成功后生成统一 diff，保存 proposal，并创建 `workspace.applyPatch` 审批，审批 payload 会记录当时的 `defaultAutoRollbackOnVerificationFailure` 供前端初始化“失败回滚”选项。若 proposal 带 `taskId` / `stepId`，任务运行时会进入等待审批。审批面板会展开 diff 预览；`approve_action` 对补丁审批做出通过或拒绝时，会把 proposal 状态同步为 `approved` 或 `rejected`，并把关联任务/步骤恢复为运行态或标记失败。`apply_approved_patch` 会重新校验审批已通过、proposal 与 approval 匹配、目标文件仍等于提案基线，再把 `new_content` 写入工作区并将 proposal 更新为 `applied`；首次应用成功后会运行 `scan_project` 返回的推荐 allowlist 验证命令，把每次运行写入 `command_runs`，并在关联任务上按 patchId 幂等写入 `patchVerification` artifact。验证通过或跳过会完成关联步骤；验证失败会把关联任务与 proposal 绑定的步骤标记为 `failed`，已有 `retry_task` 流程可重新调度失败步骤。若默认策略或本次请求开启 `autoRollbackOnVerificationFailure` 且验证失败，后端会调用同一套安全回滚逻辑恢复 `old_content`，成功后 proposal 进入 `reverted`，`PatchApplyResult.autoRollback` 返回回滚结果。重复应用已应用 proposal 会返回幂等结果，不重复执行自动验证。`revert_applied_patch` 只允许回滚 `applied` proposal；回滚前会确认当前文件内容仍等于 `new_content`，再恢复 `old_content` 并把 proposal 更新为 `reverted`。重复回滚会返回幂等结果；若文件内容已偏离补丁应用结果，会拒绝回滚以保护用户后续修改。

### send_message 路由规则

- `agentId` 为空：默认路由到 `Planner`，前端展示为“自动分配”。
- `agentId` 为 `coordinator` / `planner`：路由到 `Planner`。
- `agentId` 为 `executor`：路由到 `Executor`。
- `agentId` 为 `echo`：路由到 `Echo`。
- `Memory` 和 `Tool` 标记为内部 Agent，不允许前端直接选择。

结构化错误 `ApiError` 包含：

| 字段 | 说明 |
| --- | --- |
| `code` | 错误码 |
| `message` | 给用户看的中文错误 |
| `detail` | 技术详情 |
| `retryable` | 是否建议重试 |
| `requestId` | 请求 ID |

主要错误码：

- `INVALID_ARGUMENT`
- `AGENT_NOT_FOUND`
- `AGENT_NOT_SELECTABLE`
- `ROUTE_FAILED`
- `COMMAND_ERROR`
- `APPROVAL_ERROR`
- `PATCH_ERROR`

## 8. 前端架构

入口：

- `src-web/src/main.tsx`
- `src-web/src/App.tsx`

页面：

| 页面 | 组件 | 说明 |
| --- | --- | --- |
| 对话 | `ChatWindow` | 消息展示、Markdown 渲染、Agent 选择、发送/重试 |
| 项目 | `ProjectPanel` | 项目快照、文件检索、只读预览、推荐验证命令运行、非 allowlist 命令审批和补丁提案创建 |
| 记忆 | `MemoryPanel` | 长期知识写入与检索，支持来源和标签 |
| 审批 | `ApprovalPanel` | 高风险动作审批请求列表、通过/拒绝、执行已审批命令、补丁 diff 预览 |
| Agent | `AgentPanel` | Agent 状态列表、能力展示、5 秒轮询 |
| 设置 | `SettingsPanel` | 模型、API Key、Base URL、max tokens、temperature |

状态管理：

位置：`src-web/src/store/useAgentStore.ts`

核心状态：

- `currentPage`
- `agents` / `agentsLoading` / `agentsError`
- `messages` / `sending` / `sendError`
- `selectedAgentId`
- `healthy` / `healthVersion`
- `settings`
- `projectSnapshot` / `projectFiles` / `latestCommandRun` / `lastCommandApproval`
- `patchProposals` / `lastPatchProposal` / `patchProposalLoading` / `patchProposalError` / `patchApplyLoadingId` / `lastPatchApplyResult` / `patchRevertLoadingId` / `lastPatchRevertResult`
- `approvals` / `approvalsLoading` / `approvalDecisionLoadingId` / `approvalExecutionLoadingId`

设置项保存在浏览器 `localStorage` 的 `app-settings` 键中。

发送消息和创建任务时，前端会把当前模型、Base URL、max tokens、temperature 以及是否配置 API Key 的信息传给后端。后端写入 Planner 消息上下文时会脱敏，仅保留 `frontendLlmSettings.hasApiKey`，不会把 API Key 写入任务事件或计划上下文；Planner 是否实际使用 LLM 仍由后端环境变量 `PLANNER_USE_LLM` 控制。

前端 IPC 适配：

位置：`src-web/src/lib/tauri.ts`

- Tauri 环境使用 `@tauri-apps/api/core` 的 `invoke()`。
- 非 Tauri 浏览器环境自动使用 mock API，方便单独运行 Vite 前端。

错误归一化：

位置：`src-web/src/lib/errors.ts`

把 Tauri/Rust/JS 错误统一转成中文提示，并提取技术详情用于错误消息。

## 9. 配置

### 9.1 环境变量

模板文件：`.env.example`

当前模板包含当前运行时读取的变量：

- `OPENAI_API_KEY`
- `OPENAI_BASE_URL`
- `DEFAULT_MODEL`
- `OPENAI_MODEL`
- `DEEPSEEK_API_KEY`
- `DEEPSEEK_BASE_URL`
- `DEEPSEEK_MODEL`
- `PLANNER_USE_LLM`
- `PLANNER_LLM_PROVIDER`
- `PLANNER_LLM_RETRIES`
- `RUST_LOG`
- `TASK_DB_PATH`
- `CHAT_DB_PATH`
- `MEMORY_DB_PATH`
- `COMMAND_DB_PATH`
- `TOOL_INVOCATION_DB_PATH`
- `APPROVAL_DB_PATH`
- `PATCH_DB_PATH`
- `PATCH_AUTO_ROLLBACK_ON_VERIFICATION_FAILURE`

同时保留了 `ANTHROPIC_API_KEY`、`LOCAL_MODEL_PATH`、`SERPAPI_KEY`、`GOOGLE_API_KEY` 和 `GOOGLE_CSE_ID` 等后续扩展占位项；当前主流程尚未读取这些变量。

### 9.2 Tauri 配置

位置：`src-tauri/tauri.conf.json`

关键配置：

- `productName`: 多 Agent 协同智能体
- `identifier`: `com.rust-mutil-agent.app`
- 开发 URL：`http://localhost:1420`
- 前端构建目录：`../src-web/dist`
- `beforeDevCommand`: `npm --prefix ../src-web run dev -- --host 127.0.0.1 --port 1420`
- `beforeBuildCommand`: `npm --prefix ../src-web run build`
- 窗口默认尺寸：1200 x 800，最小 900 x 600
- CSP 允许连接 OpenAI 和 Anthropic API

### 9.3 Vite 配置

位置：`src-web/vite.config.ts`

关键配置：

- 开发端口固定为 `1420`
- 别名 `@ -> src-web/src`
- 构建输出 `dist`
- 测试环境 `jsdom`
- 测试 setup 文件：`src/test/setup.ts`

## 10. 开发运行

### 10.1 安装依赖

```bash
cd src-web
npm install
```

Rust 依赖由 Cargo 根据 `src-tauri/Cargo.toml` 和 `Cargo.lock` 管理。

### 10.2 前端单独运行

```bash
cd src-web
npm run dev
```

前端会运行在：

```text
http://localhost:1420
```

非 Tauri 环境下会自动使用 mock API。

### 10.3 Tauri 开发运行

```bash
cd src-tauri
cargo tauri dev
```

`tauri.conf.json` 已配置 `beforeDevCommand`，上述命令会自动启动 Vite 开发服务。

### 10.4 生产构建

```bash
cd src-web
npm run build

cd ../src-tauri
cargo tauri build
```

产物位于：

```text
src-tauri/target/release/bundle/
```

## 11. 测试

### 11.1 Rust 测试

```bash
cd src-tauri
cargo test
```

覆盖范围：

- AgentMessage 构造与回复链
- MessageBus 发布/订阅
- Planner 计划生成
- Executor 通用执行、HTTP 错误路径、模拟搜索
- Memory 短期记忆、SQLite 初始化、自动存储落盘和短期/长期组合检索
- ToolRegistry 和内置工具
- LLM mock 客户端
- Orchestrator 注册与任务提交
- 多 Agent 集成流程

### 11.2 前端测试

```bash
cd src-web
npm test
```

覆盖范围：

- Zustand store 行为
- ChatWindow 交互
- AgentPanel 展示与轮询
- SettingsPanel 设置保存

## 12. 当前限制与风险

1. Planner 默认仍是关键词规则；真实 LLM 规划需要通过 `PLANNER_USE_LLM` 显式开启。
2. Planner 已有 JSON plan schema、解析校验和失败降级策略；前端模型/API 配置可作为请求级临时 LLMClient 使用，API Key 不写入普通上下文或持久化数据。
3. 任务调度器已按步骤依赖推进，并把依赖步骤结果写入后续步骤上下文；当前已支持任务取消、单步骤跳过、步骤超时、失败/取消后的任务重试、任务/事件持久化，以及命令、补丁和通用 `tool.*` 审批对任务步骤的等待、恢复和失败回写；legacy `file_read` 和 `web_search` 已能自动进入审批，其他高风险工具仍待接入。
4. Planner 分析步骤仍由运行时内部模拟完成，避免把计划内 Planner 子步骤再次送入 Planner 触发嵌套规划。
5. 聊天历史已按 `sessionId` 持久化；当前前端提供本地会话列表、会话切换和新建会话，发送、读取与清理都会使用当前会话 ID。
6. MemoryAgent 已接入启动流程并默认使用 SQLite 长期记忆，KnowledgeBase 已提供结构化知识条目写入/检索 IPC；补丁验证失败会自动写入 `FailureCase`，验证通过或跳过会自动写入补丁级 `ProjectFact`，任务完成会生成任务级 `evolutionNote` artifact，Planner 请求前会检索并注入相关长期经验；后续仍需统一完整任务级 ProjectFact 写入 KnowledgeBase 和更精细的经验排序/引用策略。
7. `web_search` 是模拟结果。
8. 前端只读项目文件 API 已限制在 workspace 内；ToolRegistry 中的 legacy `file_read` 已复用同一套 workspace 沙箱并接入自动审批拦截，ToolAgent 调用审计已接入项目面板。
9. 前端设置中的 API Key 和模型配置保存在 localStorage；后端请求期可使用该配置，但尚未接入系统安全凭据存储。
10. 运行时环境变量 LLM 配置和前端请求级 LLM 配置已经并存，尚未提供统一的凭据管理界面。
11. 命令审批已经支持任务等待/恢复和已审批运行结果回写；补丁提案已经支持 diff 预览、审批状态同步、补丁审批等待/恢复、已审批手动应用、应用后自动验证、已应用补丁安全回滚、默认/可覆盖的验证失败自动回滚、任务 patch/verification/revert artifact 记录、验证失败任务/步骤 failed 标记、失败经验写入 `FailureCase` 和验证失败 `reworkSuggestion`；通用 `tool.*` 审批已经支持任务等待/恢复、拒绝失败回写、legacy `file_read` / `web_search` 自动拦截和 ToolAgent 调用审计；还没有自动生成修复补丁的验证失败返工执行器和更细粒度的回滚策略。

## 13. 建议后续路线

1. 扩展工具执行层：将 `file_read` / `web_search` 已接入的 `tool.*` 自动审批拦截推广到更多 ToolAgent 高风险动作。
2. 扩展请求级真实 LLM：在 Planner 临时 LLMClient 基础上，继续让 Executor/Tool 使用受控工具调用，并接入安全存储。
3. 扩展工程闭环：在受控验证命令运行、审计、审批请求、补丁提案、ReviewReport 和 `reworkSuggestion` 基础上，引入自动生成修复补丁的验证失败返工执行器和高风险动作自动暂停。
4. 引入更完整的记忆检索语义：统一 MemoryAgent 与 KnowledgeBase，在已自动写入补丁验证失败 `FailureCase`、补丁验证成功 `ProjectFact`、任务级 `evolutionNote` artifact 和规划前经验注入的基础上补齐完整任务级 ProjectFact 生成、经验排序和引用反馈。
5. 强化工具权限：继续对文件写入、命令执行和网络请求增加白名单、确认流和审计日志。
6. 同步配置体系：将前端设置、安全存储和后端环境变量统一。
7. 在已写回命令、补丁、通用工具审批结果、legacy `file_read` / `web_search` 自动拦截、路径沙箱、工具调用审计、补丁应用、验证结果、手动回滚、失败回滚默认策略、Review/Evolution 报告和验证失败 `reworkSuggestion` 的基础上，将更多工具拦截与自动返工继续纳入任务状态机。
