# 自研多 Agent 软件工程智能体后续任务规划

## 1. 目标定位

本项目后续目标是打造一个专门面向软件项目开发的多 Agent 智能体系统。它不依赖外部 Agent 框架，而是在 Rust/Tauri 项目内部自研 Agent Kernel、任务调度、工具权限、代码修改、测试验证、审查返工和经验沉淀机制。

目标闭环：

```text
理解项目
-> 理解需求
-> 制定计划
-> 修改代码
-> 运行验证
-> 审查 diff
-> 返工或完成
-> 沉淀项目经验
-> 下一轮任务复用经验
```

这里的“自进化”不是让系统无约束修改自己，而是在明确边界内持续改进：

- 每次改动必须形成可审查的 diff。
- 每次改动必须有验证步骤。
- 高风险操作必须等待用户审批。
- 失败经验和成功模式可以沉淀为记忆。
- EvolutionAgent 只能提出策略建议，不能静默改写核心规则。

## 2. 总体技术原则

1. 不引入外部 Agent 框架。
2. Agent 核心调度、任务状态机、权限控制和自进化策略由项目自研。
3. LLM 只作为能力提供方，通过自研 `LLMClient` 调用。
4. 工具系统自研，但工具描述采用 JSON Schema 风格，便于未来兼容 MCP。
5. 所有文件修改都走 patch/diff 流程。
6. 所有命令执行都走受控 runtime 模块。
7. 任务、步骤、日志、diff、审批、记忆都要持久化。
8. 前端必须能看见计划、步骤、日志、diff 和审批请求。

## 3. 推荐 Agent 分工

| Agent | 主要职责 | 是否直接面向用户 |
| --- | --- | --- |
| ProductAgent | 理解用户需求，提炼软件任务、验收标准和约束 | 是 |
| ArchitectAgent | 分析架构影响、模块边界、技术方案和风险 | 否 |
| PlannerAgent | 拆解任务步骤，安排执行顺序和负责 Agent | 是 |
| CoderAgent | 读取代码、生成 patch、修改实现 | 否 |
| ToolAgent | 调用文件、搜索、命令、测试、构建等工具 | 否 |
| TesterAgent | 运行测试和构建，分析失败原因 | 否 |
| ReviewerAgent | 审查 diff，发现 bug、风险、缺失测试 | 否 |
| MemoryAgent | 保存项目知识、历史任务、经验和偏好 | 否 |
| EvolutionAgent | 总结经验，提出策略和提示词改进建议 | 否 |
| CoordinatorAgent | 总控，负责状态推进、返工、完成判断 | 是 |

当前已有 `Planner`、`Executor`、`Memory`、`Tool`、`Echo`。后续可以先不急着拆出所有 Agent，而是在核心结构稳定后逐步扩展角色。

## 4. 核心架构改造

### 4.1 新增 Agent Kernel

新增模块：

```text
src-tauri/src/agent/kernel.rs
src-tauri/src/agent/spec.rs
src-tauri/src/agent/action.rs
src-tauri/src/agent/context.rs
```

核心结构建议：

```rust
pub struct AgentSpec {
    pub id: String,
    pub display_name: String,
    pub role: AgentRole,
    pub instructions: String,
    pub capabilities: Vec<String>,
    pub allowed_tools: Vec<String>,
    pub output_schema: serde_json::Value,
}

pub struct AgentContext {
    pub task: Task,
    pub step: TaskStep,
    pub project_snapshot: ProjectSnapshot,
    pub recent_memory: Vec<MemoryItem>,
    pub policy: ExecutionPolicy,
}

pub enum AgentAction {
    Think { summary: String },
    ToolCall { name: String, args: serde_json::Value },
    PatchProposal { patch_id: String, summary: String },
    Handoff { target_agent: String, reason: String },
    AskApproval { reason: String, risk: RiskLevel },
    Finish { summary: String },
    Fail { reason: String, retryable: bool },
}

pub struct AgentOutcome {
    pub actions: Vec<AgentAction>,
    pub observations: Vec<String>,
    pub confidence: f32,
}
```

任务：

- 将当前 `Agent::handle_message()` 从简单消息响应升级为可返回结构化 action。
- 保留旧 `AgentMessage` 作为兼容层，逐步迁移。
- 为每个 Agent 定义 `AgentSpec`。
- 输出必须经过 JSON/schema 校验，不能直接相信 LLM 文本。

### 4.2 新增 Task 状态机

新增模块：

```text
src-tauri/src/task/mod.rs
src-tauri/src/task/state.rs
src-tauri/src/task/event.rs
src-tauri/src/task/artifact.rs
```

核心数据：

```rust
pub struct Task {
    pub id: String,
    pub title: String,
    pub user_goal: String,
    pub status: TaskStatus,
    pub steps: Vec<TaskStep>,
    pub artifacts: Vec<TaskArtifact>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct TaskStep {
    pub id: String,
    pub task_id: String,
    pub order: u32,
    pub agent_id: String,
    pub title: String,
    pub instruction: String,
    pub status: StepStatus,
    pub depends_on: Vec<String>,
    pub attempts: u32,
    pub result: Option<serde_json::Value>,
}

pub enum TaskStatus {
    Draft,
    Planning,
    WaitingApproval,
    Running,
    Reviewing,
    Failed,
    Completed,
    Cancelled,
}

pub enum StepStatus {
    Pending,
    WaitingApproval,
    Running,
    TimedOut,
    Failed,
    Completed,
    Skipped,
}
```

任务：

- 用 `Task` / `TaskStep` 替代当前简单 `TaskResult`。
- Orchestrator 按步骤状态推进任务。
- 支持步骤依赖、重试次数、失败原因和产物记录。
- 每个状态变化生成 `TaskEvent`。

### 4.3 改造 Orchestrator

修改文件：

```text
src-tauri/src/orchestrator/mod.rs
```

目标：

- 从“消息发送器”升级为“任务运行时”。
- 负责任务创建、计划生成、步骤派发、结果聚合、失败返工和完成判断。

任务：

1. 新增 `TaskRuntime`，维护任务运行状态。
2. 新增 `submit_software_task()`，创建软件工程任务。
3. 新增 `dispatch_next_steps()`，查找可运行步骤。
4. 新增 `handle_agent_outcome()`，处理 Agent action。
5. 新增 `handle_tool_result()`，把工具结果写回 step。
6. 新增 `run_review_cycle()`，在代码修改后触发测试和审查。
7. 新增 `emit_task_event()`，通过 Tauri event 推送给前端。
8. 新增取消任务、重试步骤、继续任务接口。

## 5. 软件工程工具链

### 5.1 项目理解模块

新增模块：

```text
src-tauri/src/project/mod.rs
src-tauri/src/project/scanner.rs
src-tauri/src/project/index.rs
src-tauri/src/project/language.rs
```

任务：

- 扫描项目目录。
- 识别语言和框架，例如 Rust、Node、Tauri、React、Vite。
- 读取 manifest，例如 `Cargo.toml`、`package.json`、`tauri.conf.json`。
- 生成 `ProjectSnapshot`。
- 维护文件索引，支持按路径、扩展名、关键词查找。
- 识别推荐验证命令，例如 `cargo test`、`cargo check`、`npm test`、`npm run build`。

验收标准：

- 前端能展示当前项目结构摘要。
- 后端能返回项目技术栈、关键命令和重要文件列表。

### 5.2 Workspace 文件操作模块

新增模块：

```text
src-tauri/src/workspace/mod.rs
src-tauri/src/workspace/file_ops.rs
src-tauri/src/workspace/search.rs
src-tauri/src/workspace/patch.rs
src-tauri/src/workspace/diff.rs
```

工具：

| 工具名 | 说明 |
| --- | --- |
| `workspace.list_files` | 列出文件 |
| `workspace.read_file` | 读取文件 |
| `workspace.search_text` | 文本搜索 |
| `workspace.propose_patch` | 生成 patch 草案 |
| `workspace.apply_patch` | 应用 patch |
| `workspace.get_diff` | 获取当前 diff |
| `workspace.revert_patch` | 回滚指定 patch |

当前进度：`src-tauri/src/workspace/patch.rs` 已落地补丁提案模型、统一 diff 生成、SQLite 持久化、`workspace.applyPatch` 审批请求创建、关联任务/步骤等待审批、补丁审批通过恢复或拒绝失败、已审批补丁手动应用、应用后推荐验证命令自动运行、已应用补丁安全回滚、默认/可覆盖的验证失败自动回滚，以及关联任务的 `patchApplied` / `patchVerification` / `patchReverted` artifact 与事件写回；命令审批也已支持关联任务/步骤等待审批、审批决策恢复或失败，以及已审批命令运行后的 `commandRun` artifact 与事件写回。调度器已支持单步骤跳过和步骤超时，跳过的依赖可继续解锁后续步骤，运行超时的步骤会进入 `timedOut` 并生成事件。通用 `tool.*` 工具审批已支持创建请求、关联任务/步骤等待审批、审批通过恢复和拒绝失败，并写入 `toolApproval` / `toolApprovalResolved` artifact；Tool 步骤即将调用 legacy `file_read` 或 `web_search` 时会自动创建 `tool.fileRead` / `tool.webSearch` 审批，通过后恢复原 Tool 步骤投递，实际读取复用 workspace 只读 API 的路径沙箱。ToolAgent 调用会写入 `tool_invocations` SQLite 审计表，记录任务/步骤、approvalId、工具名、脱敏参数摘要、成功/失败、错误、耗时和创建时间，并在项目页展示最近记录。验证通过或跳过会完成关联步骤，验证失败会标记关联任务/步骤 `failed`，可通过 `retry_task` 重新调度。前端项目页、审批页和任务看板已能展示对应状态。更多 ToolAgent 高风险动作拦截、验证失败自动返工、更细粒度的回滚策略仍未实现。

安全要求：

- 所有路径必须限制在 workspace root 内。
- 默认不允许写 `.git`、密钥文件、构建产物目录。
- 删除文件、移动大量文件、覆盖配置文件必须进入审批。
- patch 应保存为 artifact，便于审查和回滚。

### 5.3 Runtime 命令执行模块

新增模块：

```text
src-tauri/src/runtime/mod.rs
src-tauri/src/runtime/command.rs
src-tauri/src/runtime/output.rs
src-tauri/src/runtime/preset.rs
```

工具：

| 工具名 | 说明 |
| --- | --- |
| `runtime.run_command` | 运行受控命令 |
| `runtime.run_test` | 运行测试命令 |
| `runtime.run_check` | 运行检查命令 |
| `runtime.detect_commands` | 识别项目可用命令 |

安全要求：

- 命令必须经过 allowlist 或审批。
- 默认允许低风险验证命令，例如 `cargo check`、`cargo test`、`npm test`、`npm run build`。
- 不允许直接执行破坏性命令。
- stdout/stderr 必须结构化保存。
- 超时、退出码、工作目录、环境变量都要记录。

### 5.4 Review 审查模块

新增模块：

```text
src-tauri/src/review/mod.rs
src-tauri/src/review/diff_review.rs
src-tauri/src/review/test_analysis.rs
src-tauri/src/review/risk.rs
```

任务：

- 分析 diff 是否符合需求。
- 检查是否遗漏测试。
- 解析测试失败输出。
- 识别高风险改动，例如认证、权限、文件删除、配置变更。
- 给出 `approve`、`request_changes`、`blocked` 三类结果。

验收标准：

- 每次代码改动后自动产生 ReviewReport。
- Review 不通过时，任务回到 CoderAgent 返工。
- Review 通过并且验证命令通过后，任务才可完成。

## 6. LLM 与结构化输出

修改模块：

```text
src-tauri/src/llm/mod.rs
```

任务：

1. 实现 OpenAI-compatible Chat Completions 调用。
2. 支持 DeepSeek 环境变量：
   - `DEEPSEEK_API_KEY`
   - `DEEPSEEK_BASE_URL`
   - `DEEPSEEK_MODEL`
3. 支持请求超时和重试。
4. 支持 JSON 输出模式。
5. 为 Planner、Coder、Reviewer 定义不同 system prompt。
6. 所有 LLM 输出先解析为结构化对象，再进入执行层。

不做：

- 不接入外部 Agent SDK。
- 不让 LLM 直接执行命令。
- 不让 LLM 直接写文件。

## 7. 持久化与记忆

新增模块：

```text
src-tauri/src/storage/mod.rs
src-tauri/src/storage/schema.rs
src-tauri/src/storage/task_repo.rs
src-tauri/src/storage/memory_repo.rs
src-tauri/src/storage/event_repo.rs
```

建议 SQLite 表：

```text
tasks
task_steps
task_events
agent_runs
tool_invocations
command_runs
patch_sets
review_reports
memory_items
evolution_notes
project_snapshots
```

Memory 类型：

| 类型 | 内容 |
| --- | --- |
| ProjectFact | 项目技术栈、模块职责、运行方式 |
| UserPreference | 用户偏好，例如测试习惯、代码风格 |
| FailureCase | 失败命令、错误原因、修复方式 |
| SuccessPattern | 成功方案、适用场景 |
| ReviewRule | 审查规则和常见风险 |
| PromptNote | 提示词改进建议 |

任务：

- 把当前 `MemoryAgent` 从短期内存升级为 SQLite 持久化。
- 每次任务完成后由 EvolutionAgent 生成经验摘要。
- Planner 在新任务开始前检索相关经验。

## 8. 自进化模块

新增模块：

```text
src-tauri/src/evolution/mod.rs
src-tauri/src/evolution/lesson.rs
src-tauri/src/evolution/prompt_version.rs
src-tauri/src/evolution/policy_update.rs
```

职责：

- 从任务历史中总结经验。
- 发现反复失败的测试、命令、模块和策略。
- 生成“下次应该怎么做”的建议。
- 对 Agent prompt 提出版本化修改建议。
- 对工具权限和验证命令提出建议。

限制：

- 不能自动改核心系统代码。
- 不能自动放宽权限。
- 不能静默修改 prompt。
- 所有策略变化都要可审查、可回滚。

验收标准：

- 每个完成或失败的任务都生成一条 EvolutionNote。
- 新任务开始时能检索并展示相关经验。
- 用户可以接受或拒绝 EvolutionAgent 的策略建议。

## 9. 前端改造

现有页面：

```text
ChatWindow
AgentPanel
SettingsPanel
```

需要新增页面：

```text
ProjectPanel
TaskBoard
StepTimeline
ExecutionLog
DiffViewer
ApprovalPanel
MemoryPanel
```

### 9.1 TaskBoard

展示：

- 当前任务列表。
- 每个任务的状态。
- 当前执行步骤。
- 失败步骤和重试入口。

### 9.2 StepTimeline

展示：

- Planner 生成的步骤。
- 每步负责 Agent。
- 每步输入、输出、状态、耗时。
- Agent handoff 记录。

### 9.3 ExecutionLog

展示：

- 工具调用日志。
- 命令执行 stdout/stderr。
- 退出码和耗时。
- 测试失败摘要。

### 9.4 DiffViewer

展示：

- patch 摘要。
- 文件级 diff。
- ReviewReport。
- 应用、拒绝、要求返工按钮。

### 9.5 ApprovalPanel

展示：

- 高风险操作说明。
- 风险等级。
- 影响文件或命令。
- 允许一次、拒绝、总是允许同类操作。

## 10. IPC 接口扩展

修改文件：

```text
src-tauri/src/commands.rs
src-web/src/lib/tauri.ts
src-web/src/types/index.ts
```

新增命令建议：

| 命令 | 说明 |
| --- | --- |
| `create_task` | 创建软件工程任务 |
| `list_tasks` | 列出任务 |
| `get_task` | 获取任务详情 |
| `cancel_task` | 取消任务 |
| `retry_step` | 重试指定步骤 |
| `approve_action` | 审批高风险 action |
| `request_tool_action_approval` | 为通用 `tool.*` 工具动作创建审批并挂起关联任务/步骤 |
| `reject_action` | 拒绝高风险 action |
| `list_project_files` | 获取项目文件树 |
| `read_project_file` | 读取项目文件 |
| `get_task_events` | 获取任务事件流 |
| `get_execution_log` | 获取执行日志 |
| `get_diff` | 获取 diff |
| `create_patch_proposal` | 创建补丁提案并生成审批 |
| `list_patch_proposals` | 列出补丁提案 |
| `apply_task_patch` | 应用任务 patch |
| `get_memory_items` | 查询记忆 |
| `accept_evolution_note` | 接受自进化建议 |
| `reject_evolution_note` | 拒绝自进化建议 |

同时建议使用 Tauri event 推送任务事件：

```text
task://created
task://updated
task://step-started
task://step-completed
task://step-timed-out
task://step-failed
task://approval-requested
task://patch-created
task://review-completed
task://completed
```

## 11. 分阶段实施计划

### 阶段 1：任务执行闭环 v1

目标：让系统真正完成一个简单软件任务闭环。

任务：

1. 新增 `task` 模块。
2. 改造 Orchestrator 为任务状态机。
3. 新增任务事件。
4. 新增结构化 AgentAction。
5. 新增 `create_task`、`get_task`、`list_tasks` IPC。
6. 前端新增 TaskBoard。
7. 保持 Agent 执行仍可用模拟输出。

验收标准：

- 用户创建任务后，能看到任务和步骤。
- Planner 生成步骤后，Orchestrator 能自动推进步骤状态。
- 任务状态能从 `Planning` 进入 `Running`，最终进入 `Completed` 或 `Failed`。

### 阶段 2：项目理解与文件读取

目标：Agent 能理解当前软件项目。

任务：

1. 新增 `project` 模块。
2. 新增 `workspace.read_file`、`workspace.search_text`、`workspace.list_files`。
3. 前端新增 ProjectPanel。
4. Planner 能使用项目快照制定计划。

验收标准：

- 系统能识别当前项目为 Rust + Tauri + React/Vite。
- Agent 能按需读取指定代码文件。
- 搜索工具能找到符号、文件和文本。

### 阶段 3：真实 LLM 与结构化计划

目标：Planner 使用真实 LLM 生成 JSON 计划。

任务：

1. 实现 DeepSeek/OpenAI-compatible LLM 调用。
2. 定义 Planner JSON schema。
3. 增加 LLM 输出解析和校验。
4. 增加 LLM 错误重试和降级策略。

验收标准：

- 用户输入软件需求后，Planner 返回结构化计划。
- 无效 JSON 不进入执行层。
- LLM 不可用时，任务进入可解释失败状态。

### 阶段 4：代码修改与 diff 审批

目标：CoderAgent 能提出代码修改，但先不自动应用高风险改动。

当前状态：补丁提案存储、单文件 diff 预览、审批请求链路、关联任务/步骤等待审批、审批决策后恢复或失败、审批后手动应用、应用后推荐验证命令自动运行、已应用补丁安全回滚、默认/可覆盖的验证失败自动回滚、任务 artifact 写回和验证失败任务/步骤 failed 标记已完成第一版；尚未由 CoderAgent 自动生成 proposal，也尚未接入验证失败自动返工和更细粒度的回滚策略。

任务：

1. 新增 patch/diff 模块。（已部分完成：proposal、diff 预览、审批创建）
2. CoderAgent 输出 PatchProposal。
3. 前端新增 DiffViewer。（已部分完成：项目页/审批页内置 diff 预览）
4. 用户可审批应用 patch。（已部分完成：审批通过后可手动应用）
5. patch 应用后生成 artifact。（已部分完成：写入任务 `patchApplied` / `patchVerification` / `patchReverted` artifact；验证失败会标记关联任务/步骤 `failed`）

验收标准：

- Agent 能针对一个小 bug 生成 patch。
- 前端能展示 diff。
- 用户审批后 patch 被应用到工作区。

### 阶段 5：测试验证与审查返工

目标：形成“改代码 -> 跑测试 -> 审查 -> 返工”的工程闭环。

任务：

1. 新增 runtime 命令执行模块。
2. 支持 `cargo check`、`cargo test`、`npm test`、`npm run build`。
3. 新增 TesterAgent。
4. 新增 ReviewerAgent。
5. Review 不通过时回到 CoderAgent。

验收标准：

- patch 应用后自动运行推荐验证命令。（已部分完成：`apply_approved_patch` 首次应用成功后会运行项目推荐 allowlist 命令并写入命令审计；验证通过或跳过会完成关联步骤，验证失败会标记关联任务/步骤 `failed`；已应用补丁可手动安全回滚，也可按默认策略或调用参数开启验证失败自动回滚）
- 测试失败能被记录并反馈给 CoderAgent。（已部分完成：失败可写回任务状态并通过 `retry_task` 重新调度；尚未自动反馈给 CoderAgent）
- Review 通过且验证通过后任务完成。

### 阶段 6：持久化与长期记忆

目标：任务历史、工具日志、经验沉淀都能保存。

任务：

1. 新增 storage 模块和 SQLite schema。
2. 保存 Task、Step、Event、ToolInvocation、PatchSet、ReviewReport。
3. MemoryAgent 接入 SQLite。
4. 新任务开始前检索相关 ProjectFact 和 FailureCase。

验收标准：

- 重启应用后仍能看到历史任务。
- 新任务能引用之前的失败经验。

### 阶段 7：受控自进化

目标：系统能总结经验并提出策略改进。

任务：

1. 新增 EvolutionAgent。
2. 完成任务后生成 EvolutionNote。
3. 前端新增 MemoryPanel 或 EvolutionPanel。
4. 支持接受/拒绝策略建议。
5. Prompt 和策略采用版本化存储。

验收标准：

- 每个任务结束后有经验总结。
- 用户可以查看、接受、拒绝建议。
- 被接受的建议能影响后续 Planner/Reviewer 的上下文。

## 12. 第一轮建议实现清单

第一轮只做“任务执行闭环 v1”，不碰真实代码修改。

优先改动：

```text
src-tauri/src/task/mod.rs
src-tauri/src/task/state.rs
src-tauri/src/task/event.rs
src-tauri/src/agent/action.rs
src-tauri/src/orchestrator/mod.rs
src-tauri/src/commands.rs
src-web/src/types/index.ts
src-web/src/store/useAgentStore.ts
src-web/src/components/TaskBoard.tsx
src-web/src/App.tsx
```

具体步骤：

1. 定义 `Task`、`TaskStep`、`TaskStatus`、`StepStatus`、`TaskEvent`。
2. Orchestrator 用 `HashMap<String, Task>` 替代当前简单 `TaskResult`。
3. `send_message` 创建任务后返回 `taskId`。
4. Planner 生成计划后，Orchestrator 将 plan 写入 Task.steps。
5. Orchestrator 顺序推进步骤，先用模拟执行结果。
6. 新增 `get_task` 和 `list_tasks`。
7. 前端新增任务看板，展示任务和步骤状态。

第一轮暂不做：

- 真实 LLM。
- 真实代码修改。
- 命令执行。
- diff 审批。
- SQLite 持久化。

这样可以先把“系统骨架”跑通，避免后续在不稳定的调度层上堆功能。

## 13. 当前文件改造清单

### 需要重点修改

| 文件 | 改造方向 |
| --- | --- |
| `src-tauri/src/orchestrator/mod.rs` | 从任务缓存升级为任务状态机和调度器 |
| `src-tauri/src/agent/traits.rs` | 支持结构化 action 和 context |
| `src-tauri/src/agent/planner_agent.rs` | 输出结构化 plan |
| `src-tauri/src/agent/executor_agent.rs` | 逐步拆成 Coder/Tester/Tool 调用者 |
| `src-tauri/src/agent/tool_agent.rs` | 工具 schema 化、权限化 |
| `src-tauri/src/memory/mod.rs` | 合并长期记忆和知识库能力 |
| `src-tauri/src/llm/mod.rs` | 实现真实 OpenAI-compatible 调用 |
| `src-tauri/src/commands.rs` | 增加任务、日志、审批、diff IPC |
| `src-web/src/store/useAgentStore.ts` | 增加任务状态、事件、日志状态 |
| `src-web/src/App.tsx` | 增加项目、任务、日志、diff 页面 |

### 需要新增

| 模块 | 用途 |
| --- | --- |
| `src-tauri/src/task/` | 任务和步骤状态机 |
| `src-tauri/src/project/` | 项目扫描和理解 |
| `src-tauri/src/workspace/` | 文件读写、搜索、diff、patch |
| `src-tauri/src/runtime/` | 命令运行和测试验证 |
| `src-tauri/src/policy/` | 权限和审批规则 |
| `src-tauri/src/review/` | diff 审查和测试失败分析 |
| `src-tauri/src/evolution/` | 经验总结和策略建议 |
| `src-tauri/src/storage/` | SQLite 持久化 |
| `src-web/src/components/TaskBoard.tsx` | 任务看板 |
| `src-web/src/components/ProjectPanel.tsx` | 项目视图 |
| `src-web/src/components/DiffViewer.tsx` | diff 预览 |
| `src-web/src/components/ExecutionLog.tsx` | 执行日志 |
| `src-web/src/components/ApprovalPanel.tsx` | 审批面板 |

## 14. 风险边界

必须禁止或审批的行为：

- 删除大量文件。
- 修改 `.git`。
- 修改密钥、证书、环境文件。
- 执行未列入 allowlist 的命令。
- 执行网络下载或安装依赖。
- 修改项目外路径。
- 自动提交、自动推送、自动发布。
- 静默放宽权限策略。

推荐默认允许的低风险行为：

- 读取 workspace 内普通文本文件。
- 搜索源码。
- 生成但不应用 patch。
- 运行只读检查命令。
- 运行项目测试命令。
- 保存任务日志和经验记录。

## 15. 完成定义

当系统具备以下能力时，可以认为第一版“软件工程自进化多 Agent”完成：

1. 能扫描并理解当前项目。
2. 能把用户需求拆成可执行软件任务步骤。
3. 能读取相关代码并生成 patch。
4. 能展示 diff 并等待用户审批。
5. 能应用 patch。
6. 能运行测试或构建验证。
7. 能审查修改质量。
8. 能根据失败反馈返工。
9. 能保存任务全过程。
10. 能总结经验并在后续任务中复用。
