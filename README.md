# Agentman - Agent Task Management System

<div align="center">

**Agentman** — Agent Task Management Daemon for Lark Base

[English](#english) | [中文](#中文)

</div>

---

<a name="english"></a>
## English

### Overview

Agentman is a Rust-based daemon that connects to Feishu (Lark) Base for autonomous Agent task management. Unlike traditional task management systems that require a backend server, Agentman uses **Lark Base Direct Connect** architecture — the daemon reads from and writes to Lark Base tables directly via OpenAPI, making it lightweight, stateless, and easy to deploy.

It enables AI coding agents (Claude Code, GitHub Copilot/Codex, OpenCode, Cursor) to automatically pick up tasks from Lark Base, execute them in isolated Git workspaces, and report results back in real-time.

### Architecture

```
+------------------------------------------------------------------+
|                         Lark Base (Cloud)                         |
|  +----------------+  +----------------+  +---------------------+ |
|  |  Tasks Table   |  | Runtimes Table |  | ExecutionLogs Table | |
|  | (任务主表)      |  | (运行时表)      |  | (执行记录表)         | |
|  +--------+-------+  +--------+-------+  +----------+----------+ |
|           |                   |                     |             |
|           |<--- OpenAPI ----->|                     |             |
+-----------|-------------------|---------------------|-------------+
            |                   |
            v                   v
+---------------------------------------------------------------+
|                    Agentman Daemon (Rust)                      |
|  +----------------+  +----------------+  +----------------+   |
|  | TaskExecutor   |  | Heartbeat      |  | BaseClient     |   |
|  |   - Poll loop  |  |   - 30-60s     |  |   - Token cache|   |
|  |   - Agent exec |  |   - Register   |  |   - Retry(3x)  |   |
|  |   - Retry      |  |   - Status     |  |   - 6 methods  |   |
|  +--------+-------+  +--------+-------+  +--------+-------+   |
|           |                   |                     |           |
|           v                   v                     v           |
|  +----------------+  +----------------+  +----------------+   |
|  | WorkspaceMgr   |  | AgentFactory   |  | Config/i18n    |   |
|  |   - per-task   |  |   - claude     |  |   - TOML       |   |
|  |   - isolated   |  |   - codex      |  |   - en/zh      |   |
|  |                |  |   - opencode   |  |                |   |
|  |                |  |   - cursor     |  |                |   |
|  +----------------+  +----------------+  +----------------+   |
+---------------------------------------------------------------+
            |
            v
+---------------------------------------------------------------+
|                     Agent CLI Tools (PATH)                    |
|  +---------+  +---------+  +---------+  +---------+          |
|  | claude  |  |  codex  |  |opencode |  | cursor  |          |
|  |  code   |  |   cli   |  |   cli   |  |   cli   |          |
|  +---------+  +---------+  +---------+  +---------+          |
+---------------------------------------------------------------+
```

### Quick Start

```bash
# 1. Clone the repository
git clone <repo-url>
cd agentman/agentman-daemon

# 2. Create configuration
cat > config.toml << 'EOF'
# Daemon identity (optional - auto-generated from hostname if omitted)
runtime_name = "Production Daemon #1"

# Lark OpenAPI endpoints
base_url = "https://open.feishu.cn"
base_token = "YOUR_BASE_TOKEN_HERE"

# Lark App credentials (from Developer Console)
app_id = "cli_xxxxxxxxxxxxxxxx"
app_secret = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"

# Polling and heartbeat intervals (seconds)
poll_interval_secs = 30
heartbeat_interval_secs = 60

# Concurrency control
max_concurrent_tasks = 3

# Workspace directory for task outputs
workspace_dir = "./workspace"

# Logging level: trace, debug, info, warn, error
log_level = "info"

# Interface language: en, zh
language = "en"
EOF

# 3. Build and run
cargo build --release
./target/release/agentman-daemon --register

# Or run in development mode
cargo run -- --register
```

### Features

| Feature | Description |
|---------|-------------|
| **Auto Agent Detection** | Automatically detects installed Agent CLIs (claude, codex, opencode, cursor) in PATH |
| **Task Pre-allocation** | Tasks are pre-allocated to specific Daemon runtimes via Lark Base link fields |
| **Real-time Streaming** | Execution logs stream back to Lark Base every 10 seconds via background flush |
| **Status Workflow** | 待办 → 进行中 → 待审核 → 已完成 (Todo → In Progress → Pending Review → Completed) |
| **Review Rejection Retry** | Auto-retry up to 3 times when review is rejected, with rejection reason appended as context |
| **Urge Reminder Filtering** | Agent tasks skip urge reminders; human tasks receive notifications via Base workflow |
| **Heartbeat Registration** | Daemon self-registers in Runtimes table with hostname, IP, OS, available agents |
| **Execution History** | Every execution attempt logged to ExecutionLogs table with output, timing |
| **Token Caching** | Lark tenant_access_token cached with 5-minute pre-expiry refresh |
| **Retry Logic** | Exponential backoff retry (3x) for network errors, rate limits, and token expiry |
| **CLI Modes** | Supports `--once` (single execution) and continuous loop modes; `--register` for initial setup |
| **i18n Support** | Multi-language interface (English / 中文) with configurable language setting |

### Tech Stack

| Layer | Technology |
|-------|-----------|
| **Language** | Rust 1.75+ |
| **Async Runtime** | Tokio (full features) |
| **HTTP Client** | reqwest with rustls-tls |
| **Serialization** | serde + serde_json |
| **Config** | TOML |
| **CLI** | clap v4 |
| **Logging** | tracing + tracing-subscriber |
| **Error Handling** | thiserror + anyhow |
| **Time** | chrono |
| **UUID** | uuid v4 |
| **Testing** | tokio-test, mockito, tempfile |

### Project Structure

```
agentman/
├── agentman-daemon/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs              # Entry point, CLI with clap
│       ├── lib.rs               # Module exports
│       ├── config.rs            # TOML config parsing
│       ├── client/
│       │   ├── mod.rs
│       │   └── base.rs          # BaseClient with token caching, retry, 6 API methods
│       ├── models/
│       │   ├── mod.rs
│       │   ├── task.rs          # Task struct with 22 fields
│       │   ├── runtime.rs       # RuntimeInfo, RuntimeStatus
│       │   └── execution.rs     # ExecutionLog, ExecutionStatus, TriggerMode
│       ├── git/
│       │   ├── mod.rs           # Git utilities (deprecated)
│       │   └── workspace.rs     # WorkspaceManager (per-task dirs)
│       ├── agent/
│       │   ├── mod.rs           # AgentAdapter trait + ExecutionResult
│       │   ├── cli_adapter.rs   # CommandLineAdapter for CLIs
│       │   ├── factory.rs       # AgentFactory
│       │   ├── openclaw_adapter.rs
│       │   ├── hermes_adapter.rs
│       │   └── tests.rs
│       ├── task_executor.rs     # Main execution loop
│       ├── heartbeat.rs         # Periodic heartbeat to Base
│       └── utils.rs             # Helpers
├── workflows/
│   ├── rejection-retry.json     # Base workflow: auto-retry on rejection
│   └── urge-reminder.json       # Base workflow: urge reminders
└── docs/
    ├── DEPLOYMENT.md            # Deployment guide
    └── API.md                   # API documentation
```

### Screenshots

> 📸 **Dashboard View**
> Place screenshot of Lark Base Tasks table here showing task list with status columns.

> 📸 **Runtime Registration**
> Place screenshot of Runtimes table showing registered daemon with heartbeat.

> 📸 **Execution Logs**
> Place screenshot of ExecutionLogs table showing detailed execution output.

### Status Flow

```
┌─────────┐     Daemon poll      ┌─────────┐     Agent execute      ┌─────────┐
│  待办   │ ───────────────────> │  进行中  │ ────────────────────> │  待审核  │
│ (Todo)  │   Fetch & assign     │(InProgress│   Stream logs         │(Pending) │
└─────────┘                      └─────────┘                       └────┬────┘
     ▲                                                                  │
     │                                                                  │ Human review
     │                                                                  │
     │    ┌───────────────────────────────────────────────────────────┘
     │    │ Approve
     │    ▼
     │ ┌─────────┐
     └─│  已完成  │
       │(Completed)│
       └─────────┘
       
     Reject ──> Auto-retry (max 3x)
     ┌──────────────────────────────────────────┐
     │  Append rejection reason to description  │
     │  Clear rejection reason field            │
     │  Increment retry_count                   │
     └──────────────────────────────────────────┘
```

### Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/amazing-feature`
3. Make your changes
4. Run tests: `cargo test`
5. Commit with conventional commits: `feat:`, `fix:`, `docs:`, etc.
6. Push and open a Pull Request

### Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific module
cargo test config::tests
cargo test agent::tests
cargo test git::tests
```

Current test coverage: **24 tests** (18 unit + 6 integration) all passing.

### License

MIT License - see [LICENSE](LICENSE) for details.

---

<a name="中文"></a>
## 中文

### 项目概述

Agentman 是一个基于 Rust 的守护进程，连接飞书多维表格（Lark Base）实现 Agent 任务自治管理。与传统需要后端服务器的任务管理系统不同，Agentman 采用 **Lark Base 直连架构** —— 守护进程通过 OpenAPI 直接读写飞书多维表格，无需中间后端，轻量、无状态、易于部署。

它支持 AI 编程 Agent（Claude Code、GitHub Copilot/Codex、OpenCode、Cursor）自动从飞书表格领取任务，在隔离的工作区中执行，并实时回传执行结果。

### 架构设计

```
+------------------------------------------------------------------+
|                         飞书多维表格 (云端)                         |
|  +----------------+  +----------------+  +---------------------+ |
|  |   任务主表      |  |   运行时表      |  |     执行记录表       | |
|  | (22个字段)      |  | (运行时注册)     |  | (执行历史追踪)       | |
|  +--------+-------+  +--------+-------+  +----------+----------+ |
|           |                   |                     |             |
|           |<--- OpenAPI ----->|                     |             |
+-----------|-------------------|---------------------|-------------+
            |                   |
            v                   v
+---------------------------------------------------------------+
|                    Agentman Daemon (Rust)                      |
|  +----------------+  +----------------+  +----------------+   |
|  | 任务执行器      |  | 心跳服务        |  | Base客户端     |   |
|  |   - 轮询循环    |  |   - 30-60秒    |  |   - Token缓存  |   |
|  |   - Agent执行  |  |   - 自动注册    |  |   - 3次重试    |   |
|  |   - 重试机制    |  |   - 状态上报    |  |   - 6个方法    |   |
|  +--------+-------+  +--------+-------+  +--------+-------+   |
|           |                   |                     |           |
|           v                   v                     v           |
|  +----------------+  +----------------+  +----------------+   |
|  | 工作区管理器   |  | Agent工厂      |  | 配置/i18n      |   |
|  |   - 按任务隔离 |  |   - claude     |  |   - TOML       |   |
|  |   - 自动清理   |  |   - codex      |  |   - 中英文     |   |
|  |                |  |   - opencode   |  |                |   |
|  |                |  |   - cursor     |  |                |   |
|  +----------------+  +----------------+  +----------------+   |
+---------------------------------------------------------------+
            |
            v
+---------------------------------------------------------------+
|                     Agent CLI 工具 (PATH)                     |
|  +---------+  +---------+  +---------+  +---------+          |
|  | claude  |  |  codex  |  |opencode |  | cursor  |          |
|  |  code   |  |   cli   |  |   cli   |  |   cli   |          |
|  +---------+  +---------+  +---------+  +---------+          |
+---------------------------------------------------------------+
```

### 快速开始

```bash
# 1. 克隆仓库
git clone <仓库地址>
cd agentman/agentman-daemon

# 2. 创建配置文件
cat > config.toml << 'EOF'
# 守护进程标识（可选 - 省略时自动从主机名生成）
runtime_name = "生产环境守护进程 #1"

# 飞书 OpenAPI 地址
base_url = "https://open.feishu.cn"
base_token = "YOUR_BASE_TOKEN_HERE"

# 飞书应用凭证（从开发者后台获取）
app_id = "cli_xxxxxxxxxxxxxxxx"
app_secret = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"

# 轮询和心跳间隔（秒）
poll_interval_secs = 30
heartbeat_interval_secs = 60

# 并发控制
max_concurrent_tasks = 3

# 任务输出工作目录
workspace_dir = "./workspace"

# 日志级别：trace, debug, info, warn, error
log_level = "info"

# 界面语言：en, zh
language = "zh"
EOF

# 3. 构建并运行
cargo build --release
./target/release/agentman-daemon --register

# 或使用开发模式运行
cargo run -- --register
```

### 核心功能

| 功能 | 说明 |
|------|------|
| **自动Agent检测** | 自动检测 PATH 中的 Agent CLI（claude, codex, opencode, cursor） |
| **任务预分配** | 通过飞书表格关联字段将任务预分配给特定 Daemon 运行时 |
| **实时日志流** | 执行日志每10秒通过后台刷新流式回写到飞书表格 |
| **状态工作流** | 待办 → 进行中 → 待审核 → 已完成 |
| **审核驳回重试** | 审核驳回后自动重试（最多3次），驳回理由自动追加到任务描述 |
| **催办提醒过滤** | Agent 任务跳过催办提醒；人工任务通过 Base 工作流接收通知 |
| **心跳注册** | Daemon 自动在运行时表注册，上报主机名、IP、操作系统、可用Agent |
| **执行历史** | 每次执行尝试记录到执行记录表，包含输出、耗时 |
| **Token缓存** | Lark tenant_access_token 缓存，过期前5分钟自动刷新 |
| **重试机制** | 网络错误、速率限制、Token过期均支持指数退避重试（最多3次） |
| **CLI模式** | 支持 `--once`（单次执行）和连续轮询模式；`--register` 首次注册 |
| **多语言支持** | 支持英文/中文界面，可通过配置切换语言 |

### 技术栈

| 层级 | 技术 |
|------|------|
| **语言** | Rust 1.75+ |
| **异步运行时** | Tokio (full features) |
| **HTTP客户端** | reqwest with rustls-tls |
| **序列化** | serde + serde_json |
| **配置** | TOML |
| **CLI** | clap v4 |
| **日志** | tracing + tracing-subscriber |
| **错误处理** | thiserror + anyhow |
| **时间** | chrono |
| **UUID** | uuid v4 |
| **测试** | tokio-test, mockito, tempfile |

### 项目结构

```
agentman/
├── agentman-daemon/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs              # 入口点，clap CLI
│       ├── lib.rs               # 模块导出
│       ├── config.rs            # TOML配置解析
│       ├── client/
│       │   ├── mod.rs
│       │   └── base.rs          # BaseClient：Token缓存、重试、6个API方法
│       ├── models/
│       │   ├── mod.rs
│       │   ├── task.rs          # Task结构体（22个字段）
│       │   ├── runtime.rs       # RuntimeInfo, RuntimeStatus
│       │   └── execution.rs     # ExecutionLog, ExecutionStatus, TriggerMode
│       ├── git/
│       │   ├── mod.rs           # Git 工具（已弃用）
│       │   └── workspace.rs     # WorkspaceManager（按任务隔离目录）
│       ├── agent/
│       │   ├── mod.rs           # AgentAdapter trait + ExecutionResult
│       │   ├── cli_adapter.rs   # 命令行Agent适配器
│       │   ├── factory.rs       # Agent工厂
│       │   ├── openclaw_adapter.rs
│       │   ├── hermes_adapter.rs
│       │   └── tests.rs
│       ├── task_executor.rs     # 主执行循环
│       ├── heartbeat.rs         # 定时心跳上报
│       └── utils.rs             # 工具函数
├── workflows/
│   ├── rejection-retry.json     # Base工作流：驳回自动重试
│   └── urge-reminder.json       # Base工作流：催办提醒
└── docs/
    ├── DEPLOYMENT.md            # 部署指南
    └── API.md                   # API文档
```

### 截图

> 📸 **任务看板**
> 在此处放置飞书多维表格任务列表截图，展示状态列。

> 📸 **运行时注册**
> 在此处放置运行时表截图，展示已注册的守护进程及心跳信息。

> 📸 **执行记录**
> 在此处放置执行记录表截图，展示详细的执行输出。

### 状态流转

```
┌─────────┐     Daemon轮询       ┌─────────┐     Agent执行        ┌─────────┐
│  待办   │ ───────────────────> │  进行中  │ ────────────────────> │  待审核  │
│ (Todo)  │   获取并分配          │(InProgress│   流式日志          │(Pending) │
└─────────┘                      └─────────┘                       └────┬────┘
     ▲                                                                  │
     │                                                                  │ 人工审核
     │                                                                  │
     │    ┌───────────────────────────────────────────────────────────┘
     │    │ 通过
     │    ▼
     │ ┌─────────┐
     └─│  已完成  │
       │(Completed)│
       └─────────┘

     驳回 ──> 自动重试（最多3次）
     ┌──────────────────────────────────────────┐
     │  将驳回理由追加到任务描述中                │
     │  清空审核驳回理由字段                      │
     │  增加重试次数计数器                        │
     └──────────────────────────────────────────┘
```

### 参与贡献

1. Fork 本仓库
2. 创建功能分支：`git checkout -b feature/神奇功能`
3. 提交更改
4. 运行测试：`cargo test`
5. 使用约定式提交：`feat:`、`fix:`、`docs:` 等
6. 推送并发起 Pull Request

### 测试

```bash
# 运行所有测试
cargo test

# 带输出运行
cargo test -- --nocapture

# 运行特定模块
cargo test config::tests
cargo test agent::tests
```

当前测试覆盖：**24个测试**（18个单元测试 + 6个集成测试）全部通过。

### 开源协议

MIT License - 详见 [LICENSE](LICENSE)。
