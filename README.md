# Agentman — Agent 任务管理系统

<div align="center">

基于 Rust 的飞书多维表格 Agent 任务自治管理守护进程

</div>

## 项目概述

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
|  | 工作区管理器   |  | Agent工厂      |  | 配置           |   |
|  |   - 按任务隔离 |  |   - claude     |  |   - TOML       |   |
|  |   - 自动清理   |  |   - codex      |  |                |   |
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

## 快速开始

```bash
# 1. 进入项目目录
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
EOF

# 3. 构建并运行
cargo build --release
./target/release/agentman-daemon --register

# 或使用开发模式运行
cargo run -- --register
```

详细部署说明请参阅 [**部署文档**](docs/DEPLOYMENT.md)。

## 核心功能

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
| **任务依赖** | 任务可声明前置依赖；阻塞型依赖必须完成后才能执行 |

## 技术栈

| 层级 | 技术 |
|------|------|
| **语言** | Rust 2024 edition |
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

## 项目结构

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
│       │   ├── core.rs          # BaseClient：Token缓存、重试
│       │   ├── parser.rs        # API响应解析
│       │   ├── task.rs          # 任务相关API方法
│       │   ├── runtime.rs       # 运行时相关API方法
│       │   └── execution.rs     # 执行记录相关API方法
│       ├── models/
│       │   ├── mod.rs
│       │   ├── dependency.rs    # DependencyType, TaskDependency
│       │   ├── task.rs          # Task结构体
│       │   ├── runtime.rs       # RuntimeInfo, RuntimeStatus
│       │   └── execution.rs     # ExecutionLog, ExecutionStatus, TriggerMode
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

## 状态流转

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

## 测试

```bash
# 运行所有测试
cargo test

# 带输出运行
cargo test -- --nocapture

# 运行特定模块
cargo test config::tests
cargo test agent::tests
cargo test models::tests
cargo test task_executor::tests
```

当前测试覆盖：**59个测试**（51个单元测试 + 8个集成测试）全部通过。

## 开源协议

MIT License - 详见 [LICENSE](LICENSE)。

## 友情链接

[linux.do 真诚、友善、团结、专业，共建你我引以为荣之社区](https://linux.do/)
