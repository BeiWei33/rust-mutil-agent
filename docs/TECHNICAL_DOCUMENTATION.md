# 多 Agent 协同智能体技术文档

## 1. 项目概览

本项目是一个基于 Rust 与 Tauri v2 的跨平台桌面应用，用于构建“多 Agent 协同智能体”运行时。系统由 React 前端提供聊天工作台、Agent 状态面板和设置面板，由 Rust 后端负责 Agent 注册、任务分发、消息通信、工具调用、记忆管理和 Tauri IPC 命令。

当前代码处于可运行原型阶段：Agent 框架、前后端通信、状态展示、工具注册表、短期记忆、LLM 客户端、项目理解和任务执行闭环 v1 已具备；真实命令执行、聊天历史持久化、完整步骤调度和写入型工具权限仍待完善。

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
│       ├── bus/
│       ├── llm/
│       ├── memory/
│       ├── orchestrator/
│       └── tool/
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
  ├─ get_project_snapshot / list_project_files / read_project_file / search_project_text
  ├─ health_check
  ├─ get_history
  └─ clear_history
        │
        ▼
Orchestrator
  ├─ 注册内置 Agent
  ├─ 维护 Agent mpsc 通道
  ├─ 创建 TaskResult 缓存
  └─ 将用户任务发送给目标 Agent
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
4. 创建 `Orchestrator`，并调用 `register_builtin_agents()` 注册内置 Agent。
5. 将 `AppState { bus, orchestrator }` 注入 Tauri 状态。
6. 注册 Tauri IPC 命令。
7. 注册 Tauri 插件：shell、fs、notification、dialog、clipboard-manager、process。
8. 启动 Tauri 应用。

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
- 将消息直接发送给指定 Agent。
- 提供 Agent 列表和任务结果查询。
- 监听 `MessageBus` 中的 Agent 回复，应用 Planner 计划并推进任务执行闭环 v1。

当前内置 Agent：

| Runtime 名称 | 前端展示名 | 直接选择 | 职责 |
| --- | --- | --- | --- |
| `Planner` | 协调员/总控 | 是 | 理解需求并生成计划 |
| `Executor` | 执行工程师 | 是 | 执行明确任务、HTTP 请求、模拟搜索 |
| `Memory` | 记忆管理员 | 否 | 短期记忆、SQLite 长期记忆能力 |
| `Tool` | 工具操作员 | 否 | 工具调用 |
| `Echo` | 回声测试员 | 是 | 通信链路测试 |

注意：当前 `submit_task_to_agent()` 会把消息发到 Agent 的 `mpsc` 通道，Agent 回复会发送到 `MessageBus` 的 broadcast 通道；Orchestrator 已订阅 Planner/Echo 等回复并更新任务状态。Planner 生成计划后，任务执行闭环 v1 会对只读 Tool 步骤执行项目搜索/文件预览，其余步骤仍以模拟结果完成。

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
- 后续为 `plan_step`，分别发往计划中的 Agent。

### 6.5 ExecutorAgent

位置：`src-tauri/src/agent/executor_agent.rs`

职责：

- 执行通用任务，返回执行确认。
- 对 `msg_type == "http_request"` 的消息发起 HTTP 请求。
- 对包含“搜索/search”的消息执行模拟搜索。

当前搜索是占位实现，会等待约 200ms 并返回模拟结果。真实命令执行和沙箱能力尚未接入。

### 6.6 MemoryAgent

位置：`src-tauri/src/agent/memory_agent.rs`

职责：

- 维护最多 50 条短期记忆。
- 支持 `store` / `remember` 写入记忆。
- 支持 `query` / `recall` / `retrieve` 通过关键词检索短期记忆。
- 支持通过 `with_database(db_path)` 初始化 SQLite 长期记忆表。

默认注册时使用 `MemoryAgent::new()`，不连接 SQLite 数据库；长期记忆需要显式使用 `with_database()`。

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
| `file_read` | 已实现 | 读取文本文件，当前未做路径沙箱限制 |
| `web_search` | 占位 | 返回模拟搜索结果 |

`ToolAgent` 可通过 `context.tool` 显式指定工具；未指定时会根据消息内容关键词推断工具。

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

`ChatCompletionRequest` 支持 `response_format`，其中 `JsonObject` 会映射为 OpenAI-compatible JSON mode。Planner 侧已提供 JSON plan schema 和解析校验入口；通过 `PLANNER_USE_LLM=true` 可启用 LLM JSON 规划，默认仍使用规则规划作为稳定降级路径。

Planner LLM 相关环境变量：

| 变量 | 说明 |
| --- | --- |
| `PLANNER_USE_LLM` | 设为 `true` / `1` / `yes` / `on` 时启用 LLM 规划 |
| `PLANNER_LLM_PROVIDER` | `openai` 或 `deepseek`，未设置时优先 OpenAI Key，否则 DeepSeek |
| `PLANNER_LLM_RETRIES` | LLM 调用重试次数，范围 1-3 |

## 7. Tauri IPC 命令契约

位置：`src-tauri/src/commands.rs`，前端封装在 `src-web/src/lib/tauri.ts`。

| 命令 | 入参 | 返回 | 当前状态 |
| --- | --- | --- | --- |
| `send_message` | `{ request: { content, agentId?, routeMode?, sessionId?, llmSettings? } }` | `SendMessageResponse` | 已实现任务提交 |
| `create_task` | `{ request: { content, agentId?, llmSettings? } }` | `CreateTaskResponse` | 已实现软件工程任务创建 |
| `get_task` | `{ taskId }` | `Task` 或 `null` | 已实现 |
| `list_tasks` | 无 | `{ tasks }` | 已实现 |
| `get_task_events` | `{ taskId }` | `TaskEvent[]` | 已实现 |
| `get_project_snapshot` | 无 | `ProjectSnapshot` | 已实现 |
| `list_project_files` | `{ maxFiles? }` | `{ files }` | 已实现 |
| `read_project_file` | `{ path }` | `FileReadResponse` | 已实现只读沙箱 |
| `search_project_text` | `{ request: { query, maxResults? } }` | `SearchResponse` | 已实现只读搜索 |
| `get_agent_status` | `{ agentId }` | `AgentStatusResponse` | 已实现 |
| `list_agents` | 无 | `{ agents }` | 已实现 |
| `get_task_result` | `{ taskId }` | 兼容旧接口的任务结果或 `null` | 已聚合当前任务步骤和输出 |
| `health_check` | 无 | `{ healthy, version, agentCount }` | 已实现 |
| `get_history` | `{ sessionId }` | 消息数组 | 当前返回空数组 |
| `clear_history` | `{ sessionId }` | `void` | 当前为 no-op |

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

## 8. 前端架构

入口：

- `src-web/src/main.tsx`
- `src-web/src/App.tsx`

页面：

| 页面 | 组件 | 说明 |
| --- | --- | --- |
| 对话 | `ChatWindow` | 消息展示、Markdown 渲染、Agent 选择、发送/重试 |
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

当前模板包含：

- `OPENAI_API_KEY`
- `OPENAI_BASE_URL`
- `DEFAULT_MODEL`
- `ANTHROPIC_API_KEY`
- `LOCAL_MODEL_PATH`
- `SERPAPI_KEY`
- `GOOGLE_API_KEY`
- `GOOGLE_CSE_ID`
- `RUST_LOG`
- `DATABASE_PATH`

代码中的 DeepSeek 客户端另外读取：

- `DEEPSEEK_API_KEY`
- `DEEPSEEK_BASE_URL`
- `DEEPSEEK_MODEL`

建议后续统一 `.env.example` 与实际读取的环境变量。

### 9.2 Tauri 配置

位置：`src-tauri/tauri.conf.json`

关键配置：

- `productName`: 多 Agent 协同智能体
- `identifier`: `com.rust-mutil-agent.app`
- 开发 URL：`http://localhost:1420`
- 前端构建目录：`../src-web/dist`
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

注意：当前 `tauri.conf.json` 中 `beforeDevCommand` 为空，通常需要先单独启动 Vite，或后续把它改为自动运行前端开发命令。

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
- Memory 短期记忆、SQLite 初始化、检索
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
2. Planner 已有 JSON plan schema、解析校验和失败降级策略；前端模型/API 配置已随请求传给后端，但 API Key 当前只做脱敏占位，尚未用于请求级 LLMClient。
3. 任务执行闭环 v1 会自动完成计划步骤，其中只读 Tool 步骤能检索项目；Executor/Memory 等步骤仍是模拟结果。
4. Planner 生成的 `plan_step` 回复发布到 broadcast 后，尚未作为真正的依赖调度队列逐步投递到对应 Agent 的 `mpsc` 通道。
5. `get_history` 返回空数组，`clear_history` 是 no-op。
6. MemoryAgent 默认不使用 SQLite；长期记忆未接入应用启动流程。
7. `web_search` 是模拟结果。
8. `file_read` 当前直接读取路径，后续需要加路径权限、沙箱和审计。
9. 前端设置中的 API Key 和模型配置保存在 localStorage；当前已随请求传递脱敏摘要，但未接入安全存储或请求级 LLMClient。
10. 运行时 API Key 仍主要依赖环境变量或 localStorage，尚未接入系统安全凭据存储。
11. README 中部分描述仍偏旧，例如前端并非 Next.js 预留，而是 Vite + React 已实现。

## 13. 建议后续路线

1. 实现计划步骤调度器：将 Planner 产出的 `plan_step` 真正按依赖关系投递给对应 Agent。
2. 接入请求级真实 LLM：在安全存储方案落地后，让 Planner 可按前端配置构造临时 LLMClient，再让 Executor/Tool 使用工具调用。
3. 扩展工具执行层：在只读项目检索之后，引入可审批的命令运行、补丁生成和差异审查能力。
4. 引入持久化会话：实现 `get_history` / `clear_history`，并统一 MemoryAgent 与 KnowledgeBase。
5. 强化工具权限：对 `file_read`、未来命令执行和网络请求增加白名单、确认流和审计日志。
6. 同步配置体系：将前端设置、安全存储和后端环境变量统一。
7. 更新 README：修正前端技术栈、运行方式和当前实现状态。
