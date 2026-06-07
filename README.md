# 多 Agent 协同智能体 (Multi-Agent Cooperative Intelligence)

基于 Rust 构建的多 Agent 协同智能体系统，提供跨平台桌面应用。

## 项目概述

本项目构建了一个多 Agent 协同工作框架，多个独立的智能体（Agent）通过消息总线进行通信与协作，完成复杂的任务流程。

**核心特性：**
- 🤖 **多 Agent 协同**：Planner、Executor、Memory、Tool 等异构 Agent 动态协作
- ⚡ **纯 Rust 后端**：基于 Tokio 异步运行时，高并发、内存安全
- 🖥️ **跨平台 GUI**：使用 Tauri v2 构建桌面应用（Windows/macOS/Linux）
- 🔒 **本地优先**：支持离线运行，敏感数据保留在用户设备
- 🔌 **可扩展**：插件化工具注册机制，用户可自定义 Agent 和工具

## 架构概览

```
┌─────────────────────────────────────────────────────────────┐
│                    GUI Layer (Tauri)                        │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │
│  │ 对话面板 │  │ 任务看板 │  │ Agent 管 │  │ 设置面板 │   │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘   │
└─────────────────────────────────────────────────────────────┘
                              │ IPC
┌─────────────────────────────────────────────────────────────┐
│                 Agent Runtime (Rust)                        │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                 Orchestrator (调度器)                 │  │
│  └───────────┬──────────┬──────────┬───────────────────┘  │
│  ┌───────────▼──┐  ┌────▼────┐ ┌───▼──────┐ ┌───────────┐ │
│  │ PlannerAgent │  │ExecAgent│ │MemoryAgent│ │ToolAgent  │ │
│  └──────────────┘  └─────────┘ └──────────┘ └───────────┘ │
│                     Message Bus (broadcast)                 │
│  ┌──────────────────────────────────────────────────────┐  │
│  │   Knowledge Base (SQLite)     │   Tool Registry      │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## 项目结构

```
rust-mutil-agent/
├── src-tauri/               # Tauri 后端 (Rust)
│   ├── src/
│   │   ├── main.rs          # 应用入口，启动 Tauri + Agent 系统
│   │   ├── agent/           # Agent 定义与实现
│   │   │   ├── mod.rs
│   │   │   ├── traits.rs    # Agent trait、AgentMessage、Capability
│   │   │   ├── echo_agent.rs      # 回显 Agent（测试用）
│   │   │   ├── planner_agent.rs   # 任务规划 Agent
│   │   │   ├── executor_agent.rs  # 执行 Agent
│   │   │   ├── memory_agent.rs    # 记忆 Agent
│   │   │   └── tool_agent.rs      # 工具 Agent
│   │   ├── bus/             # 消息总线
│   │   │   └── mod.rs
│   │   ├── orchestrator/    # 调度器
│   │   │   └── mod.rs
│   │   ├── memory/          # 记忆与知识库
│   │   │   └── mod.rs
│   │   ├── tool/            # 工具注册表
│   │   │   └── mod.rs
│   │   ├── llm/             # LLM 客户端封装
│   │   │   └── mod.rs
│   │   ├── commands.rs      # Tauri 命令（前后端接口）
│   │   └── error.rs         # 错误类型定义
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── build.rs
├── src-web/                 # 前端 (React/Next.js)
│   └── .gitkeep
├── plugins/                 # Agent 插件目录
│   └── .gitkeep
├── docs/                    # 文档
│   └── .gitkeep
├── scripts/                 # 构建脚本
│   └── .gitkeep
├── .env.example             # 环境变量模板
├── .gitignore
└── README.md
```

## 快速开始

### 前置要求

- **Rust** 1.80+ (stable)
- **Node.js** 18+
- **Tauri CLI**：`cargo install tauri-cli`
- 平台特定依赖：参考 [Tauri 文档](https://v2.tauri.app/start/prerequisites/)

### 开发运行

```bash
# 1. 克隆项目
git clone <repository-url>
cd rust-mutil-agent

# 2. 配置环境变量
cp .env.example .env
# 编辑 .env，填入 API Key 等配置

# 3. 运行单元测试（纯后端逻辑）
cd src-tauri && cargo test

# 4. 启动 Tauri 开发环境（含前端热更新）
cargo tauri dev
# 或从项目根目录：cd src-tauri && cargo tauri dev
```

### 生产构建

```bash
cargo tauri build
# 输出在 src-tauri/target/release/bundle/
```

## 核心 Agent 说明

| Agent | 职责 | 能力 |
|-------|------|------|
| **Planner** | 将用户目标分解为可执行步骤 | `planning` |
| **Executor** | 执行具体操作（HTTP 请求、命令） | `code_execution` |
| **Memory** | 管理对话记忆与知识检索 | `memory`, `retrieval` |
| **Tool** | 注册和调用外部工具/函数 | `tool_use` |
| **Echo** | 测试回显（开发调试用） | `chat` |

## 内置工具

| 工具 | 功能 | 状态 |
|------|------|------|
| `calculator` | 数学表达式计算 | ✅ 已实现 |
| `datetime` | 获取当前日期时间 | ✅ 已实现 |
| `file_read` | 读取文本文件 | ✅ 已实现 |
| `web_search` | 互联网搜索 | 🔧 占位 |

## 技术栈

- **后端**：Rust + Tokio + Tauri v2
- **通信**：broadcast/mpsc 消息总线
- **存储**：SQLite (rusqlite)
- **序列化**：serde + serde_json
- **日志**：tracing
- **前端**：React/Next.js (预留)

## 测试

```bash
# 运行所有测试
cargo test

# 运行特定模块测试
cargo test --package rust-mutil-agent -- agent
cargo test --package rust-mutil-agent -- bus
cargo test --package rust-mutil-agent -- orchestrator

# 显示测试输出
cargo test -- --nocapture
```

## 贡献指南

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'feat: 添加新功能'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

## 许可证

MIT License

---

*文档版本 1.0 · 最后更新 2026年6月*
