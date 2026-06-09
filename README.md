# 多 Agent 协同智能体

基于 Rust、Tauri v2 和 React/Vite 构建的本地优先多 Agent 软件工程桌面应用。项目目标是把用户需求拆成可追踪的软件工程任务，由 Planner、Executor、Tool、Memory 等 Agent 通过消息总线协作推进，并在前端展示对话、任务、项目结构、命令运行、审批请求和 Agent 状态。

当前代码处于可运行原型阶段：多 Agent 运行时、Tauri IPC、任务状态机、步骤超时、项目只读检索、聊天/任务/事件持久化、MemoryAgent SQLite 长期记忆、请求级 Planner LLM 配置、受控验证命令、命令审计、ToolAgent 调用审计、审批请求基础、非 allowlist 命令审批入口、已审批命令执行、命令审批等待/恢复、已审批命令结果回写、补丁提案持久化、diff 审批预览、已审批补丁手动应用、应用后自动验证、已应用补丁安全回滚、可配置默认的验证失败自动回滚、补丁审批等待/恢复、通用 `tool.*` 工具审批等待/恢复、legacy `file_read` / `web_search` 自动审批拦截和 workspace 路径沙箱、任务产物记录和验证失败任务/步骤标记已经落地；更多工具权限、验证失败自动返工和完整自进化闭环仍在路线图中。

## 当前进度

| 模块 | 状态 | 说明 |
| --- | --- | --- |
| Tauri 桌面壳 | 已实现 | Rust 后端启动 Agent 运行时，注册 IPC 命令和 Tauri 插件。 |
| React/Vite 前端 | 已实现 | 包含对话、任务看板、项目面板、审批面板、Agent 面板和设置页。 |
| Agent 运行时 | 已实现原型 | 内置 `Planner`、`Executor`、`Memory`、`Tool`、`Echo`，通过 mpsc + broadcast 通信。 |
| 任务闭环 | 已实现 | 支持 `Task`、`TaskStep`、`TaskEvent`、依赖推进、步骤超时、取消、重试和 SQLite 恢复。 |
| 聊天历史 | 已实现 | 按 `sessionId` 写入 SQLite，前端默认使用 `default` 会话。 |
| 长期记忆 | 已实现原型 | `MemoryAgent` 启动时默认连接 `MEMORY_DB_PATH`，自动存储写入 SQLite，检索会先查短期记忆再补充长期记忆；KnowledgeBase 已提供结构化知识存储/搜索 IPC；初始化失败时回退到短期记忆或内存知识库。 |
| 项目理解 | 已实现 | 扫描 Rust/Tauri/React/Vite 项目，生成技术栈、Manifest、关键文件和推荐命令。 |
| Workspace 只读能力 | 已实现 | 支持项目内文件列表、文本读取和源码搜索，并限制路径逃逸和敏感文件读取。 |
| Planner 规划 | 部分实现 | 默认使用规则/项目上下文规划；可通过环境变量或前端请求级配置启用 LLM JSON 规划并自动降级。 |
| LLM Client | 部分实现 | 支持 Mock、OpenAI、DeepSeek 和自定义 OpenAI-compatible 端点；`LlamaCpp` 仍为预留。 |
| 受控命令运行 | 已实现 | 只允许低风险验证命令，执行不经过 shell，结果写入命令审计 SQLite。 |
| 审批请求基础 | 已实现原型 | 支持审批请求持久化、列表筛选、通过/拒绝、重复决策保护、非 allowlist 命令手动审批、通用 `tool.*` 工具审批、任务关联审批等待/恢复和已审批命令执行结果回写。 |
| 工程修改闭环 | 部分实现 | 已支持现有文本文件的 patch proposal、统一 diff 预览、审批请求生成、关联任务/步骤等待审批、审批决策恢复或失败、审批后应用、应用后自动验证、手动安全回滚、可配置默认的验证失败自动回滚、legacy `file_read` / `web_search` 自动审批拦截、workspace 路径沙箱、ToolAgent 调用审计、任务 artifact 写回和验证失败任务/步骤 failed 标记；更多 ToolAgent 工具权限、验证失败自动返工和 ReviewAgent 尚未接入。 |

## 功能概览

- 多 Agent 协作：Planner 负责任务拆解，Executor 负责执行确认，Tool 负责工具调用，Memory 负责记忆，Echo 用于链路调试。
- 可追踪任务：每次消息或任务创建都会生成任务 ID，可查看步骤状态、事件时间线、输出、步骤超时、取消和重试。
- 持久化：任务、任务事件、聊天历史、MemoryAgent 长期记忆、结构化知识条目、命令运行记录、ToolAgent 调用审计、审批请求和补丁提案都使用 SQLite 本地保存。
- 项目面板：展示技术栈、Manifest、关键文件、文件列表、只读预览、文本搜索、推荐命令、最近运行记录和补丁提案草稿。
- 受控验证命令：支持 `cargo check`、`cargo test`、`npm test -- --run`、`npm run build`，带工作目录限制、超时和输出截断。
- 审批面板：展示待审批/全部审批请求，可通过或拒绝高风险动作请求；项目面板可为非 allowlist 命令和补丁提案创建审批请求，通用工具可创建 `tool.*` 审批请求；命令审批通过后可从审批面板执行并写入命令审计，若审批关联任务/步骤会同步等待、恢复、完成或失败状态；补丁审批会展示 diff，同步提案状态，在应用后自动运行推荐验证命令，可对已应用补丁执行安全回滚；“失败回滚”会按后端默认策略初始化，也可在应用前手动覆盖，以便验证失败时自动恢复补丁；关联任务的验证失败会标记任务/步骤 failed，并可通过重试入口重新调度。
- 请求级 LLM 设置：前端可传模型、Base URL、max tokens、temperature 和 API Key；API Key 只进入请求期临时上下文，不写入持久化数据。
- 浏览器降级：前端单独运行 Vite 时自动使用 mock API，方便开发 UI。
- 结构化错误：后端返回 `ApiError`，前端统一转换为中文错误提示和技术详情。

## 技术栈

| 层级 | 技术 |
| --- | --- |
| 桌面容器 | Tauri v2 |
| 后端 | Rust 2021, Tokio |
| IPC | Tauri `invoke` |
| Agent 通信 | Tokio `mpsc` + `broadcast` |
| 序列化 | serde, serde_json, rmp-serde |
| 本地数据 | SQLite via rusqlite |
| HTTP/LLM | reqwest, OpenAI-compatible Chat Completions |
| 前端 | React 18, Vite, TypeScript |
| 状态管理 | Zustand |
| 样式 | Tailwind CSS |
| 测试 | Rust tests, Vitest, Testing Library |

## 项目结构

```text
rust-mutil-agent/
├── README.md
├── .env.example
├── docs/
│   ├── TECHNICAL_DOCUMENTATION.md
│   └── SELF_EVOLVING_AGENT_ROADMAP.md
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── tests/
│   └── src/
│       ├── main.rs              # Tauri 入口和 AppState 初始化
│       ├── lib.rs               # 库模块导出，供测试使用
│       ├── commands.rs          # Tauri IPC 命令
│       ├── approval.rs          # 审批请求模型和持久化
│       ├── chat.rs              # 聊天历史持久化
│       ├── agent/               # Planner/Executor/Memory/Tool/Echo
│       ├── bus/                 # MessageBus
│       ├── llm/                 # LLMClient
│       ├── orchestrator/        # 任务调度器
│       ├── project/             # 项目扫描和规划上下文
│       ├── runtime/             # 受控命令运行和审计
│       ├── task/                # Task/Step/Event/Store
│       ├── tool/                # 工具注册表
│       └── workspace/           # 项目文件列表、读取、搜索和补丁提案
└── src-web/
    ├── package.json
    ├── vite.config.ts
    └── src/
        ├── App.tsx
        ├── components/
        │   ├── ChatWindow.tsx
        │   ├── TaskBoard.tsx
        │   ├── ProjectPanel.tsx
        │   ├── ApprovalPanel.tsx
        │   ├── AgentPanel.tsx
        │   └── SettingsPanel.tsx
        ├── lib/
        ├── store/
        ├── types/
        └── __tests__/
```

## 架构概览

```text
React/Vite UI
  ├─ ChatWindow
  ├─ TaskBoard
  ├─ ProjectPanel
  ├─ ApprovalPanel
  ├─ AgentPanel
  └─ SettingsPanel
        │
        │ Tauri invoke
        ▼
Rust Commands
  ├─ send_message / create_task
  ├─ get_task / list_tasks / get_task_events
  ├─ cancel_task / retry_task
  ├─ get_project_snapshot / list_project_files / read_project_file / search_project_text
  ├─ run_project_command / request_project_command_approval / request_tool_action_approval
  ├─ run_approved_project_command
  ├─ list_project_command_runs
  ├─ create_patch_proposal / list_patch_proposals / get_patch_proposal
  ├─ apply_approved_patch / revert_applied_patch
  ├─ list_approval_requests / approve_action
  ├─ get_history / clear_history
  └─ list_agents / health_check
        │
        ▼
AppState
  ├─ Orchestrator + TaskStore
  ├─ ChatStore
  ├─ KnowledgeBase
  ├─ CommandRunStore
  ├─ ToolInvocationStore
  ├─ ApprovalStore
  └─ PatchProposalStore
        │
        ▼
Agents + MessageBus
  ├─ Planner
  ├─ Executor
  ├─ Tool
  ├─ Memory
  └─ Echo
```

## 快速开始

### 前置要求

- Rust stable 1.80+
- Node.js 18+
- Tauri v2 本地依赖，参考 [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
- Tauri CLI：`cargo install tauri-cli`

### 安装依赖

```bash
cd src-web
npm install
```

Rust 依赖由 Cargo 根据 `src-tauri/Cargo.toml` 和 `src-tauri/Cargo.lock` 管理。

### 前端单独运行

```bash
cd src-web
npm run dev
```

默认地址为：

```text
http://localhost:1420
```

非 Tauri 浏览器环境会自动使用 mock API，可用于开发和查看前端界面。

### Tauri 开发运行

`src-tauri/tauri.conf.json` 已配置 `beforeDevCommand`，运行 Tauri 开发命令时会自动启动 Vite：

```bash
cd src-tauri
cargo tauri dev
```

### 生产构建

```bash
cd src-web
npm run build

cd ../src-tauri
cargo tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`。

## 测试

```bash
# Rust 后端测试
cd src-tauri
cargo test

# 前端测试
cd src-web
npm test -- --run
```

当前测试覆盖 Agent 消息、MessageBus、Planner 规划、LLM mock/OpenAI-compatible 请求结构、任务运行时、持久化 store、审批 store、补丁提案 store、补丁应用/回滚、项目扫描、workspace 只读能力、受控命令 runner、前端 store 和主要组件交互。

## 配置

复制 `.env.example` 为 `.env` 后按需填写：

```bash
cp .env.example .env
```

常用环境变量：

| 变量 | 说明 |
| --- | --- |
| `RUST_LOG` | Rust 日志级别，例如 `info`、`debug`。 |
| `OPENAI_API_KEY` | OpenAI API Key。 |
| `OPENAI_BASE_URL` | OpenAI-compatible base URL，可填 base URL 或完整 `/chat/completions` 端点。 |
| `OPENAI_MODEL` / `DEFAULT_MODEL` | OpenAI 默认模型。 |
| `DEEPSEEK_API_KEY` | DeepSeek API Key。 |
| `DEEPSEEK_BASE_URL` | DeepSeek OpenAI-compatible base URL。 |
| `DEEPSEEK_MODEL` | DeepSeek 默认模型。 |
| `PLANNER_USE_LLM` | 设为 `true`、`1`、`yes` 或 `on` 时启用 Planner LLM JSON 规划。 |
| `PLANNER_LLM_PROVIDER` | `openai` 或 `deepseek`。 |
| `PLANNER_LLM_RETRIES` | Planner LLM 重试次数，范围 1-3。 |
| `TASK_DB_PATH` | 任务/事件 SQLite 路径，默认 `rust-mutil-agent-tasks.sqlite3`。 |
| `CHAT_DB_PATH` | 聊天历史 SQLite 路径，默认 `rust-mutil-agent-chat.sqlite3`。 |
| `MEMORY_DB_PATH` | MemoryAgent 长期记忆 SQLite 路径，默认 `rust-mutil-agent-memory.sqlite3`。 |
| `COMMAND_DB_PATH` | 命令运行审计 SQLite 路径，默认 `rust-mutil-agent-commands.sqlite3`。 |
| `TOOL_INVOCATION_DB_PATH` | ToolAgent 工具调用审计 SQLite 路径，默认 `rust-mutil-agent-tool-invocations.sqlite3`。 |
| `APPROVAL_DB_PATH` | 审批请求 SQLite 路径，默认 `rust-mutil-agent-approvals.sqlite3`。 |
| `PATCH_DB_PATH` | 补丁提案 SQLite 路径，默认 `rust-mutil-agent-patches.sqlite3`。 |
| `PATCH_AUTO_ROLLBACK_ON_VERIFICATION_FAILURE` | 设为 `true`、`1`、`yes` 或 `on` 时，新补丁审批默认勾选验证失败自动回滚；单次应用请求可覆盖。 |

默认情况下 Planner 不会访问网络，而是使用规则和项目上下文生成计划。启用 LLM 后，Planner 会要求模型输出 JSON，并在解析或调用失败时降级为规则规划。前端设置中的 API Key 只在单次请求中通过 `transient_context` 传给 Planner，不会写入任务事件、聊天历史、命令审计或审批记录。

本地生成的 `*.sqlite` / `*.sqlite3` 已在 `.gitignore` 中忽略。

## 内置 Agent

| Runtime 名称 | 前端名称 | 可直接选择 | 职责 |
| --- | --- | --- | --- |
| `Planner` | 协调员/总控 | 是 | 理解需求、生成计划、安排协作。 |
| `Executor` | 执行工程师 | 是 | 执行明确任务；当前真实代码修改仍未接入。 |
| `Memory` | 记忆管理员 | 否 | 短期记忆、SQLite 长期记忆、检索和结果记录。 |
| `Tool` | 工具操作员 | 否 | 工具调用和内部只读检索步骤。 |
| `Echo` | 回声测试员 | 是 | 测试消息链路。 |

## IPC 能力

| 命令 | 状态 | 说明 |
| --- | --- | --- |
| `send_message` | 已实现 | 聊天入口，返回任务 ID、路由信息并写入聊天历史。 |
| `create_task` | 已实现 | 创建软件工程任务。 |
| `get_task` / `list_tasks` | 已实现 | 查询任务详情和任务列表。 |
| `get_task_events` | 已实现 | 查询任务事件时间线。 |
| `cancel_task` / `retry_task` | 已实现 | 取消未结束任务，或重试失败/取消任务。 |
| `get_project_snapshot` | 已实现 | 返回项目技术栈、Manifest、关键文件和推荐命令。 |
| `list_project_files` | 已实现 | 列出 workspace 内文件。 |
| `read_project_file` | 已实现 | 只读读取 workspace 内文本文件。 |
| `search_project_text` | 已实现 | 搜索 workspace 内文本内容。 |
| `run_project_command` | 已实现 | 运行 allowlist 内低风险验证命令并写审计。 |
| `request_project_command_approval` | 已实现 | 为非 allowlist 项目命令创建待审批请求。 |
| `request_tool_action_approval` | 已实现 | 为通用 `tool.*` 工具动作创建待审批请求，并让关联任务/步骤进入待审批状态。 |
| `run_approved_project_command` | 已实现 | 通过 approvalId 执行已通过审批的项目命令，并关联写入命令审计。 |
| `list_project_command_runs` | 已实现 | 查询最近命令运行记录。 |
| `list_tool_invocations` | 已实现 | 查询最近 ToolAgent 工具调用审计记录，包含任务/步骤、approvalId、脱敏参数摘要、成功状态、错误和耗时。 |
| `store_knowledge` / `search_knowledge` | 已实现 | 写入和检索长期知识条目，支持标题、内容、来源和标签，用于后续 ProjectFact / FailureCase 经验沉淀。 |
| `create_patch_proposal` | 已实现 | 为现有文本文件生成补丁提案、持久化 diff，创建 `workspace.applyPatch` 审批请求，并让关联任务/步骤进入待审批状态。 |
| `list_patch_proposals` / `get_patch_proposal` | 已实现 | 查询最近补丁提案或指定补丁提案。 |
| `apply_approved_patch` | 已实现 | 通过 approvalId 应用已通过审批的补丁，把提案状态更新为 `applied`，运行推荐验证命令，并在关联任务中写入 patch/verification artifact 与事件；验证失败会标记关联任务/步骤 failed，可按后端默认策略或单次请求开启验证失败自动回滚。 |
| `revert_applied_patch` | 已实现 | 通过 patchId 回滚已应用补丁；回滚前校验当前文件内容仍等于补丁应用结果，成功后写入 `patchReverted` artifact 与事件。 |
| `list_approval_requests` | 已实现 | 查询审批请求，可按状态过滤。 |
| `approve_action` | 已实现 | 对审批请求执行通过或拒绝；命令、`workspace.applyPatch` 和通用 `tool.*` 审批会恢复或失败关联任务/步骤，补丁审批还会同步补丁提案状态。 |
| `get_history` / `clear_history` | 已实现 | 读取或清理指定会话聊天历史。 |
| `list_agents` / `get_agent_status` | 已实现 | 查询 Agent 列表和状态。 |
| `health_check` | 已实现 | 返回后端健康状态、版本和 Agent 数量。 |

## 受控命令 allowlist

`run_project_command` 不接受任意 shell 命令，只匹配下列命令和工作目录：

| 工作目录 | 命令 |
| --- | --- |
| `src-tauri` | `cargo check` |
| `src-tauri` | `cargo test` |
| `src-web` | `npm test -- --run` |
| `src-web` | `npm run build` |

命令执行使用 `tokio::process::Command`，不经过 shell；工作目录必须解析在 workspace 内；包含 shell 控制字符或父目录穿越会被拒绝。执行超时为 120 秒，stdout/stderr 最多返回前 96 KB，并带截断标记。

非 allowlist 命令不会直接执行；项目面板可调用 `request_project_command_approval` 创建高风险审批请求，审批 payload 会记录归一化命令、工作目录、默认 allowlist 判定和可选 `taskId` / `stepId`。若审批关联任务/步骤，创建审批会写入 `commandApproval` artifact、发出 `approvalRequested` 事件并把关联任务/步骤置为 `waitingApproval`；审批通过会写入 `commandApprovalResolved` artifact 并恢复运行态，拒绝会把关联任务/步骤标记为 `failed`。审批通过后，审批面板可调用 `run_approved_project_command` 按 approvalId 执行原审批 payload 中的命令，不经过 shell，继续限制在 workspace 内，并在 `command_runs.approval_id` 中写入审计关联；执行结果会按 approvalId 幂等写入 `commandRun` artifact 和事件，成功时完成关联步骤，失败时标记任务/步骤 failed，便于通过 `retry_task` 重新调度。

## 补丁提案与 diff 审批

项目面板在读取文本文件后可编辑草稿并调用 `create_patch_proposal`。后端会校验路径仍在 workspace 内、目标是普通文本文件、基线内容与当前文件一致，并拒绝 `.git`、密钥文件和构建产物目录。创建成功后会写入 `patch_proposals` SQLite 表，并同步生成 `workspace.applyPatch` 审批请求；审批 payload 包含 `patchId`、文件列表和统一 diff。若补丁提案关联任务/步骤，任务运行时会写入 `patchApproval` artifact，发出 `approvalRequested` 事件，并把关联任务/步骤置为 `waitingApproval`。

审批面板会对 `workspace.applyPatch` 展开文件列表和 diff。用户通过或拒绝审批时，后端会把对应补丁提案状态更新为 `approved` 或 `rejected`，写入 `patchApprovalResolved` artifact，并发出 `approvalResolved` 事件；通过会让关联任务/步骤恢复运行，拒绝会把关联任务/步骤标记为 failed，便于通过 `retry_task` 重新调度。通过后可继续调用 `apply_approved_patch` 手动写入工作区。应用时后端会重新校验目标文件仍与提案基线一致，拒绝过期补丁，并把提案状态更新为 `applied`。首次应用成功后，后端会运行项目扫描得到的推荐验证命令，把结果写入命令审计；若补丁提案关联了任务，系统会同步写入 `patchApplied` / `patchVerification` artifact 和 `artifactCreated` 事件，验证通过或跳过时会完成关联步骤，验证失败时还会把关联任务和步骤标记为 `failed`。`PATCH_AUTO_ROLLBACK_ON_VERIFICATION_FAILURE` 可配置新补丁审批的失败回滚默认值，`apply_approved_patch` 也可携带 `autoRollbackOnVerificationFailure` 对单次应用显式覆盖；触发时后端会复用安全回滚校验，成功后把 proposal 更新为 `reverted`，在 `patchVerification` artifact 记录 `autoRollback` 元信息，并额外写入 `patchReverted` artifact。已应用补丁也可通过 `revert_applied_patch` 手动回滚：后端会确认当前文件内容仍等于补丁应用后的 `newContent`，再恢复 `oldContent`，并写入 `patchReverted` artifact 和事件；若文件已被用户继续修改，会拒绝回滚以避免覆盖新改动。

## 通用工具动作审批

`request_tool_action_approval` 可为 `tool.*` 动作创建审批请求，调用方需提供标题、原因、动作类型、payload、风险等级和可选 `taskId` / `stepId`。后端会拒绝非 `tool.` 前缀的动作类型；若请求关联任务/步骤，创建审批会写入 `toolApproval` artifact、发出 `approvalRequested` 事件，并把关联任务/步骤置为 `waitingApproval`。

用户通过或拒绝审批时，`approve_action` 会为 `tool.*` 请求写入 `toolApprovalResolved` artifact 和 `approvalResolved` 事件；通过会把等待中的任务/步骤恢复为 `running`，拒绝会把关联任务/步骤标记为 `failed`，便于后续通过 `retry_task` 重新调度。调度器会在 Tool 步骤即将调用 legacy `file_read` 或 `web_search` 时自动创建 `tool.fileRead` / `tool.webSearch` 审批；通过后会按审批 payload 恢复原 Tool 步骤投递，拒绝则保留失败可重试状态。legacy `file_read` 会复用 workspace 只读文件能力，限制父目录穿越、绝对路径、敏感文件和超大文件。ToolAgent 每次真实工具调用都会写入 `tool_invocations` 审计表，并在项目面板展示最近记录；审计包含任务/步骤、approvalId、工具名、脱敏参数摘要、成功/失败、错误、耗时和创建时间。

## 已知限制

1. `Executor` 还没有接入真实代码修改或沙箱写入，补丁应用仍需要用户在审批面板手动触发。
2. 审批请求已经可持久化、决策，并接入非 allowlist 命令手动审批、通用 `tool.*` 工具审批、legacy `file_read` / `web_search` 自动审批拦截、已审批命令执行结果回写、ToolAgent 调用审计和补丁提案 diff 审批/应用；命令、补丁和通用工具审批已能挂起/恢复关联任务步骤，但尚未覆盖所有 ToolAgent 工具或通用文件写入。
3. `web_search` 仍是模拟工具，不会访问真实互联网。
4. `MemoryAgent` 已接入 SQLite 长期记忆，KnowledgeBase 也已提供结构化知识条目 IPC；后续还需要沉淀 ProjectFact / FailureCase 等经验模型，并在新任务规划前自动注入相关经验。
5. 项目内 `workspace` IPC 已限制路径和敏感文件；通用 ToolRegistry 中的 legacy `file_read` 已复用同一套 workspace 沙箱并接入自动审批拦截和工具调用审计。
6. ReviewAgent、EvolutionAgent、验证失败自动返工和更细粒度的回滚策略仍待实现。
7. 前端默认只有 `default` 聊天会话，尚未提供多会话管理界面。

## 路线图

详细路线见 [docs/SELF_EVOLVING_AGENT_ROADMAP.md](docs/SELF_EVOLVING_AGENT_ROADMAP.md)。近期优先级：

1. 把已具备的命令/patch 审批等待恢复、手动回滚、失败回滚默认策略和验证失败可重试状态继续推进到验证失败自动返工。
2. 继续把 `tool.*` 自动审批拦截从 `file_read` / `web_search` 扩展到更多 ToolAgent 高风险动作。
3. 将 Executor 拆分/演进为 Coder、Tester、Reviewer 等更清晰的工程角色。
4. 统一前端设置、安全存储和后端 LLMClient 配置。

## 文档

- [技术文档](docs/TECHNICAL_DOCUMENTATION.md)
- [自进化 Agent 路线图](docs/SELF_EVOLVING_AGENT_ROADMAP.md)

## 许可证

MIT License
