# Agentman 端到端测试指南 (E2E Testing Guide)

> **版本**: v1.0  
> **日期**: 2025-04-25  
> **适用项目**: Agentman - Rust Agent Daemon for Feishu (Lark) Base  
> **测试环境**: 飞书多维表格 (真实 Base)

---

## 目录

1. [测试环境信息](#1-测试环境信息)
2. [测试前准备](#2-测试前准备)
3. [测试数据速查](#3-测试数据速查)
4. [测试用例](#4-测试用例)
   - [TC-E2E-01: 正常 Agent 任务流](#tc-e2e-01-正常-agent-任务流)
   - [TC-E2E-02: 审核通过](#tc-e2e-02-审核通过)
   - [TC-E2E-03: 审核驳回 + 自动重试](#tc-e2e-03-审核驳回--自动重试)
   - [TC-E2E-04: 人类任务催办](#tc-e2e-04-人类任务催办)
   - [TC-E2E-05: Agent 任务催办过滤](#tc-e2e-05-agent-任务催办过滤)
   - [TC-E2E-06: 多 Daemon 隔离执行](#tc-e2e-06-多-daemon-隔离执行)
   - [TC-E2E-07: 心跳与离线检测](#tc-e2e-07-心跳与离线检测)
5. [常见问题排查](#5-常见问题排查)
6. [测试数据重置](#6-测试数据重置)
7. [附录](#7-附录)

---

## 1. 测试环境信息

### 1.1 Base 基本信息

| 项目 | 值 |
|------|-----|
| **Base 名称** | Agent任务管理系统 |
| **Base Token** | `YOUR_BASE_TOKEN_HERE` |
| **Base URL** | https://dcnn71d2pd2o.feishu.cn/base/YOUR_BASE_TOKEN_HERE |
| **访问权限** | Bot 身份 (tenant_access_token), full_access |

### 1.2 表信息

| 表名 | 表 ID | 用途 |
|------|-------|------|
| 任务主表 | `YOUR_TASK_TABLE_ID` | 核心任务生命周期管理 |
| 运行时表 | `YOUR_RUNTIME_TABLE_ID` | Daemon 运行时注册与心跳 |
| 执行记录表 | `YOUR_EXECUTION_LOG_TABLE_ID` | 任务执行日志与审计 |

### 1.3 状态值映射

| 状态名称 | Option ID | 说明 |
|----------|-----------|------|
| 待办 | `YOUR_TODO_OPTION_ID` | 初始状态，等待执行 |
| 进行中 | `optTskIOtJ` | 正在处理中 |
| 待审核 | `YOUR_PENDING_REVIEW_OPTION_ID` | Agent 执行完成，等待人工审核 |
| 已完成 | `opt9oRyDDk` | 审核通过或人工完成 |
| 已取消 | `optQs4bX32` | 取消或超过重试上限 |

### 1.4 枚举值定义

**执行者类型**
- `human` - 人工执行
- `agent` - Agent 自动执行

**Agent 类型**
- `claude-code` - Claude Code CLI
- `codex` - GitHub Copilot / Codex CLI
- `opencode` - OpenCode CLI
- `cursor` - Cursor CLI
- `其他` / `other` - 其他 Agent

### 1.5 自动化工作流

| 工作流 ID | 名称 | 状态 | 触发条件 |
|-----------|------|------|----------|
| `YOUR_WORKFLOW_ID_1` | 审核驳回自动重试 | enabled | 任务主表 - 审核驳回理由字段变更为非空 |
| `YOUR_WORKFLOW_ID_2` | 任务催办提醒 | enabled | ReminderTrigger - 截止时间前1天 09:00 |

---

## 2. 测试前准备

### 2.1 环境要求

- [ ] Rust 工具链已安装 (`rustc --version` >= 1.75)
- [ ] `cargo` 可用
- [ ] 飞书应用凭证有效 (`app_id`, `app_secret`)
- [ ] `lark-cli` 已安装并登录 (`lark-cli auth login`)
- [ ] 至少一个 Agent CLI 已安装（如 `claude`, `codex`, `opencode` 等）
- [ ] Git 已配置（用于仓库克隆）
- [ ] 网络可访问 `https://open.feishu.cn`

### 2.2 配置文件

创建 `config.toml`：

```toml
runtime_id = "agentman-test-001"
runtime_name = "E2E Test Daemon"
base_url = "https://open.feishu.cn"
base_token = "YOUR_BASE_TOKEN_HERE"
app_id = "YOUR_APP_ID_HERE"
app_secret = "YOUR_APP_SECRET_HERE"
poll_interval_secs = 30
heartbeat_interval_secs = 60
max_concurrent_tasks = 3
workspace_dir = "./workspace"
log_level = "info"
```

> **注意**: `runtime_id` 必须唯一。多 Daemon 测试时需为每个实例分配不同的 `runtime_id`。

### 2.3 编译 Daemon

```bash
cd agentman-daemon
cargo build --release
```

或使用开发模式：

```bash
cd agentman-daemon
cargo build
```

### 2.4 验证 lark-cli 连接

```bash
# 验证 Base 可访问
lark-cli base +table-list --base-token YOUR_BASE_TOKEN_HERE

# 预期输出包含：任务主表、运行时表、执行记录表
```

---

## 3. 测试数据速查

### 3.1 创建任务的字段模板

```json
{
  "任务标题": "E2E测试-正常Agent任务流",
  "任务描述": "这是一个端到端测试任务，用于验证Agent任务完整生命周期。\n需求：在README.md中添加一行测试文本。",
  "执行者类型": "agent",
  "执行者": "agentman-test-001",
  "任务状态": "待办",
  "优先级": "P1",
  "Agent类型": "claude-code",
  "仓库地址": "https://github.com/example/test-repo",
  "分支名称": "main",
  "预计工时": 1,
  "重试次数": 0,
  "催办次数": 0
}
```

### 3.2 lark-cli 常用命令

```bash
# 查询任务主表记录
lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_TASK_TABLE_ID

# 查询运行时表记录
lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_RUNTIME_TABLE_ID

# 查询执行记录表
lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_EXECUTION_LOG_TABLE_ID

# 创建任务
lark-cli base +record-create --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_TASK_TABLE_ID \
  --json '{"任务标题":"测试任务","执行者类型":"agent","任务状态":"待办","Agent类型":"claude-code"}'

# 更新任务状态
lark-cli base +record-update --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_TASK_TABLE_ID \
  --record-id RECORD_ID --json '{"任务状态":"已完成","审核意见":"测试通过"}'

# 删除测试记录（谨慎使用）
lark-cli base +record-delete --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_TASK_TABLE_ID \
  --record-id RECORD_ID
```

---

## 4. 测试用例

---

### TC-E2E-01: 正常 Agent 任务流

#### 测试 ID
TC-E2E-01

#### 测试目的
验证 Agent 任务从创建到执行完成的完整生命周期：待办 → 进行中 → 待审核，并确认执行记录正确创建。

#### 前置条件
1. Agentman Daemon 已编译 (`cargo build`)
2. `config.toml` 已配置正确的 `app_id`, `app_secret`, `base_token`
3. 至少一个 Agent CLI 可用（如 `claude`）
4. 测试用 Git 仓库可访问

#### 测试步骤

**Step 1** - 在飞书 Base 中创建测试任务

在任务主表 (`YOUR_TASK_TABLE_ID`) 中新增一条记录，填写以下字段：

| 字段 | 值 |
|------|-----|
| 任务标题 | `E2E-01-正常Agent任务流` |
| 任务描述 | `在README.md末尾添加一行文本："E2E测试通过"。这是TC-E2E-01测试任务。` |
| 执行者类型 | `agent` |
| 执行者 | `agentman-test-001`（与 config.toml 中的 runtime_id 一致） |
| 任务状态 | `待办` |
| 优先级 | `P1` |
| Agent类型 | `claude-code` |
| 仓库地址 | `https://github.com/example/test-repo`（替换为可用的公开仓库） |
| 分支名称 | `main` |
| 预计工时 | `1` |

> 或使用 lark-cli 创建：
> ```bash
> lark-cli base +record-create --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_TASK_TABLE_ID \
>   --json '{"任务标题":"E2E-01-正常Agent任务流","任务描述":"在README.md末尾添加一行文本","执行者类型":"agent","执行者":"agentman-test-001","任务状态":"待办","Agent类型":"claude-code","仓库地址":"https://github.com/example/test-repo","分支名称":"main","预计工时":1}'
> ```

**Step 2** - 启动 Daemon

```bash
cd agentman-daemon
cargo run -- --config config.toml
```

预期日志输出：
```
INFO agentman_daemon: Agentman Daemon starting...
INFO agentman_daemon: Runtime ID: agentman-test-001
INFO agentman_daemon: Base URL: https://open.feishu.cn
INFO agentman_daemon: Starting main loop
```

**Step 3** - 观察任务状态流转

等待 Daemon 完成一个轮询周期（默认 30 秒），观察任务状态变化：

1. **待办** → **进行中**: Daemon 发现任务并更新状态
2. **进行中** → **待审核**: Agent CLI 执行完成，Daemon 更新状态

> 可在 Base 中实时刷新页面观察状态变化。

**Step 4** - 验证执行记录

检查执行记录表 (`YOUR_EXECUTION_LOG_TABLE_ID`) 中是否出现新记录：

```bash
lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_EXECUTION_LOG_TABLE_ID
```

预期新记录包含：
- 关联任务 = 刚创建的任务 record_id
- 执行序号 = 1
- Agent类型 = claude-code
- 执行状态 = 成功
- 执行输出 包含 Agent 执行日志
- 触发方式 = 自动

#### 预期结果

| 检查项 | 预期结果 |
|--------|----------|
| 任务状态流转 | 待办 → 进行中 → 待审核 |
| 执行记录创建 | 执行记录表新增1条记录，状态为"成功" |
| 执行日志 | 包含 Agent CLI 的输出内容 |
| 任务描述更新 | 如为驳回重试，追加驳回理由到描述 |
| 无报错 | Daemon 日志无 ERROR 级别输出 |

#### 验证方法

1. **Base UI 验证**: 在飞书 Base 中查看任务状态是否为"待审核"
   - [截图占位] Base 任务主表视图 - 任务状态列

2. **lark-cli 验证**:
   ```bash
   lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_TASK_TABLE_ID
   ```
   确认返回记录中 `任务状态` = `待审核`

3. **执行记录验证**:
   ```bash
   lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_EXECUTION_LOG_TABLE_ID
   ```
   确认新增记录的 `执行状态` = `成功`

4. **日志验证**: 检查 Daemon 控制台输出，确认包含以下日志：
   ```
   INFO agentman_daemon::task_executor: Found 1 pending tasks
   INFO agentman_daemon::task_executor: Processing task 1: E2E-01-正常Agent任务流
   INFO agentman_daemon::task_executor: Task 1 completed successfully, status set to 待审核
   ```

---

### TC-E2E-02: 审核通过

#### 测试 ID
TC-E2E-02

#### 测试目的
验证审核人通过任务后，工作流正确触发通知，任务状态变为"已完成"。

#### 前置条件
1. TC-E2E-01 已完成，任务当前状态为"待审核"
2. 审核人飞书账号可接收消息通知

#### 测试步骤

**Step 1** - 确认任务状态为"待审核"

在 Base 中找到 TC-E2E-01 创建的任务，确认状态为"待审核"。

**Step 2** - 审核人操作

1. 在 Base 中打开该任务的详情页
2. 将"任务状态"从"待审核"改为"已完成"
3. 在"审核意见"字段填写：
   ```
   代码审查通过，改动符合需求。
   ```
4. 保存记录

**Step 3** - 观察工作流触发

飞书工作流 `YOUR_WORKFLOW_ID_1`（审核驳回自动重试）不会触发（因为没有填写驳回理由）。

任务状态保持为"已完成"。

**Step 4** - 验证通知（如配置了通知工作流）

如有额外的"审核通过通知"工作流，验证相关通知已发送给任务创建人。

#### 预期结果

| 检查项 | 预期结果 |
|--------|----------|
| 任务状态 | 变为"已完成" |
| 审核意见 | 已保存填写的内容 |
| 驳回工作流 | 未触发（因为未填写驳回理由） |
| 重试次数 | 保持为 0 |

#### 验证方法

1. **Base UI 验证**: 确认任务状态为"已完成"
   - [截图占位] 任务详情页 - 状态字段

2. **lark-cli 验证**:
   ```bash
   lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_TASK_TABLE_ID
   ```
   确认目标记录的 `任务状态` = `已完成`，`审核意见` 不为空

---

### TC-E2E-03: 审核驳回 + 自动重试

#### 测试 ID
TC-E2E-03

#### 测试目的
验证审核驳回后工作流自动重置任务为"待办"，Daemon 重新执行，重试次数递增，达到上限后任务变为"已取消"。

#### 前置条件
1. TC-E2E-01 已完成，任务当前状态为"待审核"
2. 工作流 `YOUR_WORKFLOW_ID_1`（审核驳回自动重试）已启用
3. Daemon 正在运行

#### 测试步骤

**Step 1** - 第一次驳回

1. 在 Base 中找到状态为"待审核"的测试任务
2. 在"审核驳回理由"字段填写：
   ```
   第一次驳回：请在README.md中添加更详细的说明，包括测试日期和作者信息。
   ```
3. 保存记录

**Step 2** - 观察工作流自动重置

等待几秒后观察：
- 工作流 `YOUR_WORKFLOW_ID_1` 触发
- 任务状态自动从"待审核"变为"待办"
- "审核驳回理由"字段被清空（Daemon 执行时自动处理）
- "重试次数"从 0 变为 1

**Step 3** - Daemon 重新执行

等待 Daemon 下一个轮询周期（30 秒内）：
- Daemon 发现"待办"状态的任务
- 检测到这是驳回重试（通过检查驳回理由是否为空）
- 将驳回理由追加到任务描述作为上下文
- Agent 重新执行，携带驳回理由上下文
- 状态变为"进行中"，然后"待审核"

预期 Daemon 日志：
```
INFO agentman_daemon::task_executor: Task 1 is retrying after rejection: 第一次驳回...
INFO agentman_daemon::task_executor: 第1次重试（审核驳回: 第一次驳回...）
```

**Step 4** - 第二次驳回

1. 任务再次变为"待审核"后，在"审核驳回理由"填写：
   ```
   第二次驳回：格式不符合规范，请使用 Markdown 标题格式。
   ```
2. 保存，观察工作流重置和 Daemon 重试
3. 确认"重试次数"变为 2

**Step 5** - 第三次驳回

1. 任务再次变为"待审核"后，在"审核驳回理由"填写：
   ```
   第三次驳回：仍然不正确，请参照项目模板格式。
   ```
2. 保存，观察工作流重置和 Daemon 重试
3. 确认"重试次数"变为 3

**Step 6** - 第四次驳回（达到上限）

1. 任务再次变为"待审核"后，在"审核驳回理由"填写：
   ```
   第四次驳回：仍然不符合要求。
   ```
2. 保存
3. 工作流重置为"待办"
4. Daemon 执行失败处理逻辑：
   - 计算新的重试次数 = 3 + 1 = 4
   - 4 >= MAX_RETRIES (3)，所以状态变为"已取消"
   - 不再进行重试

#### 预期结果

| 轮次 | 操作 | 重试次数 | 任务状态 | Daemon 行为 |
|------|------|----------|----------|-------------|
| 初始 | 创建任务 | 0 | 待办 → 进行中 → 待审核 | 正常执行 |
| 第1次 | 填写驳回理由 | 0 → 1 | 待审核 → 待办 → 进行中 → 待审核 | 重试，带上下文 |
| 第2次 | 填写驳回理由 | 1 → 2 | 待审核 → 待办 → 进行中 → 待审核 | 重试，带上下文 |
| 第3次 | 填写驳回理由 | 2 → 3 | 待审核 → 待办 → 进行中 → 待审核 | 重试，带上下文 |
| 第4次 | 填写驳回理由 | 3 → 4 | 待审核 → 待办 → **已取消** | 检测到超限，取消 |

#### 验证方法

1. **Base UI 验证**: 每次驳回后截图记录状态变化
   - [截图占位] 第1次驳回后状态
   - [截图占位] 第2次驳回后状态
   - [截图占位] 第3次驳回后状态
   - [截图占位] 第4次驳回后状态（应为已取消）

2. **重试次数验证**:
   ```bash
   lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_TASK_TABLE_ID
   ```
   确认最终 `重试次数` = 4，`任务状态` = `已取消`

3. **执行记录验证**: 检查执行记录表，确认有 4 条执行记录（初始 + 3 次重试）
   ```bash
   lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_EXECUTION_LOG_TABLE_ID
   ```

4. **日志验证**: 确认第 4 次日志包含：
   ```
   INFO agentman_daemon::task_executor: Task 1 failed, retry count: 4/3
   INFO agentman_daemon::task_executor: Task 1 status updated to 已取消 after failure
   ```

5. **通知验证**: 每次驳回后，任务创建人应收到飞书消息：
   > "任务 [任务标题] 被驳回，已自动重置为待办状态，Daemon将重新执行。驳回理由: [理由]"

---

### TC-E2E-04: 人类任务催办

#### 测试 ID
TC-E2E-04

#### 测试目的
验证人类任务在截止前收到催办通知，审核人收到提醒消息，"最后催办时间"字段被更新。

#### 前置条件
1. 工作流 `YOUR_WORKFLOW_ID_2`（任务催办提醒）已启用
2. 审核人飞书账号可接收消息
3. 当前时间可以设置截止时间为"明天"（触发 ReminderTrigger）

#### 测试步骤

**Step 1** - 创建人类任务

在任务主表中创建一条新记录：

| 字段 | 值 |
|------|-----|
| 任务标题 | `E2E-04-人类任务催办测试` |
| 任务描述 | `这是一个人类任务的催办测试。` |
| 执行者类型 | `human` |
| 执行者 | `test-user` |
| 任务状态 | `待办` 或 `进行中` |
| 截止时间 | 设置为明天同一时间 |
| 审核人 | 选择一个可接收飞书消息的测试用户 |
| 最后催办时间 | 留空 |

**Step 2** - 等待或手动触发催办

由于 ReminderTrigger 是在截止时间前 1 天的 09:00 触发，等待自然触发可能需要较长时间。

**替代方案 - 手动验证工作流逻辑**:

1. 在飞书工作流编辑器中手动测试 `YOUR_WORKFLOW_ID_2`
2. 或使用 Base 的"测试工作流"功能
3. 选择刚创建的记录作为测试数据

**Step 3** - 验证催办通知

催办触发后，验证：

1. **审核人收到飞书消息**：
   > "任务催办提醒"
   > "您有一个任务即将截止：
   > 任务：E2E-04-人类任务催办测试
   > 截止时间：[明天日期]
   > 请及时处理。"
   > [查看任务] 按钮

2. **Base 记录更新**：
   - "最后催办时间"被更新为当前时间

#### 预期结果

| 检查项 | 预期结果 |
|--------|----------|
| 催办消息 | 审核人收到飞书催办通知 |
| 最后催办时间 | 被更新为催办触发时间 |
| 任务状态 | 保持"待办"或"进行中"（不变） |

#### 验证方法

1. **飞书消息验证**: 检查审核人的飞书消息列表
   - [截图占位] 审核人收到的催办消息

2. **Base 字段验证**:
   ```bash
   lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_TASK_TABLE_ID
   ```
   确认 `最后催办时间` 字段已更新

3. **工作流执行记录**: 在飞书工作流编辑器中查看 `YOUR_WORKFLOW_ID_2` 的执行历史

---

### TC-E2E-05: Agent 任务催办过滤

#### 测试 ID
TC-E2E-05

#### 测试目的
验证 Agent 任务不会被催办，创建人收到"Agent 任务自动执行中"的消息，而非催办通知。

#### 前置条件
1. 工作流 `YOUR_WORKFLOW_ID_2`（任务催办提醒）已启用
2. Daemon 正在运行或 Agent 任务已分配
3. 任务创建人飞书账号可接收消息

#### 测试步骤

**Step 1** - 创建 Agent 任务

在任务主表中创建一条新记录：

| 字段 | 值 |
|------|-----|
| 任务标题 | `E2E-05-Agent任务催办过滤测试` |
| 任务描述 | `验证Agent任务不会被催办。` |
| 执行者类型 | `agent` |
| 执行者 | `agentman-test-001` |
| 任务状态 | `待办` |
| 截止时间 | 设置为明天同一时间 |
| 审核人 | 选择一个可接收飞书消息的测试用户 |
| Agent类型 | `claude-code` |

**Step 2** - 等待或手动触发催办

同 TC-E2E-04，等待 ReminderTrigger 在截止时间前 1 天触发。

或使用工作流测试功能手动触发。

**Step 3** - 验证催办过滤

催办触发后，验证：

1. **审核人未收到催办消息**
2. **任务创建人收到消息**：
   > "Agent任务自动执行中"
   > "任务 E2E-05-Agent任务催办过滤测试 由Agent执行中，无需催办。"

3. **"最后催办时间"字段未被更新**（保持为空）

#### 预期结果

| 检查项 | 预期结果 |
|--------|----------|
| 审核人消息 | 未收到催办通知 |
| 创建人消息 | 收到"Agent任务自动执行中"通知 |
| 最后催办时间 | 保持为空（未更新） |
| 工作流分支 | 走了 `action_end` 分支（IfElse 条件为 false） |

#### 验证方法

1. **飞书消息验证**:
   - [截图占位] 审核人消息列表 - 无催办消息
   - [截图占位] 创建人收到的"Agent任务自动执行中"消息

2. **Base 字段验证**:
   ```bash
   lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_TASK_TABLE_ID
   ```
   确认 `最后催办时间` 字段为空

3. **工作流执行记录**: 在飞书工作流编辑器中查看 `YOUR_WORKFLOW_ID_2` 的执行历史，确认走了 `action_end` 分支

---

### TC-E2E-06: 多 Daemon 隔离执行

#### 测试 ID
TC-E2E-06

#### 测试目的
验证多 Daemon 运行时，任务预分配机制生效，每个 Daemon 只执行分配给它的任务。

#### 前置条件
1. 两台可运行 Daemon 的机器（或同一机器用不同 `runtime_id` 启动两个实例）
2. 两个不同的 `runtime_id`：
   - `agentman-test-001`（Daemon A）
   - `agentman-test-002`（Daemon B）
3. 运行时表中有对应的运行时记录

#### 测试步骤

**Step 1** - 准备两个运行时记录

在运行时表 (`YOUR_RUNTIME_TABLE_ID`) 中确保有两条记录：

| 运行时 ID | 状态 |
|-----------|------|
| `agentman-test-001` | 在线 |
| `agentman-test-002` | 在线 |

或使用 Daemon 的 `--register` 参数注册：

```bash
# 机器 A
cargo run -- --config config-daemon-a.toml --register

# 机器 B
cargo run -- --config config-daemon-b.toml --register
```

**Step 2** - 获取运行时 record_id**

```bash
lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_RUNTIME_TABLE_ID
```

记录两个运行时的 `record_id`：
- Runtime A: `recRuntimeAxxx`
- Runtime B: `recRuntimeBxxx`

**Step 3** - 创建两个预分配任务

创建任务 1（分配给 Daemon A）：

```bash
lark-cli base +record-create --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_TASK_TABLE_ID \
  --json '{
    "任务标题": "E2E-06-任务A-DaemonA",
    "任务描述": "预分配给Daemon A的任务",
    "执行者类型": "agent",
    "执行者": "agentman-test-001",
    "任务状态": "待办",
    "Agent类型": "claude-code",
    "仓库地址": "https://github.com/example/repo-a",
    "分配的运行时": [{"id": "recRuntimeAxxx"}]
  }'
```

创建任务 2（分配给 Daemon B）：

```bash
lark-cli base +record-create --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_TASK_TABLE_ID \
  --json '{
    "任务标题": "E2E-06-任务B-DaemonB",
    "任务描述": "预分配给Daemon B的任务",
    "执行者类型": "agent",
    "执行者": "agentman-test-002",
    "任务状态": "待办",
    "Agent类型": "claude-code",
    "仓库地址": "https://github.com/example/repo-b",
    "分配的运行时": [{"id": "recRuntimeBxxx"}]
  }'
```

**Step 4** - 启动两个 Daemon

在机器 A 上启动 Daemon A：

```bash
cargo run -- --config config-daemon-a.toml
```

在机器 B 上启动 Daemon B：

```bash
cargo run -- --config config-daemon-b.toml
```

**Step 5** - 观察执行隔离

观察两个 Daemon 的日志：

**Daemon A 预期日志**:
```
INFO agentman_daemon::task_executor: Found 1 pending tasks
INFO agentman_daemon::task_executor: Processing task X: E2E-06-任务A-DaemonA
# 不应该出现 E2E-06-任务B-DaemonB
```

**Daemon B 预期日志**:
```
INFO agentman_daemon::task_executor: Found 1 pending tasks
INFO agentman_daemon::task_executor: Processing task Y: E2E-06-任务B-DaemonB
# 不应该出现 E2E-06-任务A-DaemonA
```

#### 预期结果

| 检查项 | 预期结果 |
|--------|----------|
| Daemon A | 只执行任务 A，不执行任务 B |
| Daemon B | 只执行任务 B，不执行任务 A |
| 任务状态 | 两个任务都正常流转到"待审核" |
| 无竞争 | 两个 Daemon 不会同时尝试执行同一个任务 |

#### 验证方法

1. **日志对比验证**:
   - [截图占位] Daemon A 日志 - 只包含"任务A"
   - [截图占位] Daemon B 日志 - 只包含"任务B"

2. **Base 状态验证**:
   ```bash
   lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_TASK_TABLE_ID
   ```
   确认两个任务的状态都变为"待审核"

3. **执行记录验证**:
   ```bash
   lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_EXECUTION_LOG_TABLE_ID
   ```
   确认有两条独立的执行记录，分别关联两个任务

---

### TC-E2E-07: 心跳与离线检测

#### 测试 ID
TC-E2E-07

#### 测试目的
验证 Daemon 启动后正确注册运行时并发送心跳，停止后运行时状态在 90 秒后变为"离线"。

#### 前置条件
1. Daemon 可正常编译运行
2. 运行时表可读写

#### 测试步骤

**Step 1** - 清理旧运行时记录（可选）

```bash
# 查询现有运行时记录
lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_RUNTIME_TABLE_ID
```

**Step 2** - 启动 Daemon 并注册运行时

```bash
cd agentman-daemon
cargo run -- --config config.toml --register
```

预期日志：
```
INFO agentman_daemon: Registering runtime...
INFO agentman_daemon::client::base: Registered runtime agentman-test-001
INFO agentman_daemon: Starting main loop
```

**Step 3** - 验证运行时注册

立即检查运行时表：

```bash
lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_RUNTIME_TABLE_ID
```

确认：
- 新增一条运行时记录
- `运行时ID` = `agentman-test-001`
- `状态` = `在线`
- `最后心跳` = 当前时间（或非常接近）
- `主机名`、`IP地址`、`操作系统`、`版本号` 字段有值

- [截图占位] 运行时表 - 新注册的运行时记录

**Step 4** - 验证心跳更新

等待 60 秒（心跳间隔），再次查询：

```bash
lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_RUNTIME_TABLE_ID
```

确认 `最后心跳` 时间已更新。

**Step 5** - 停止 Daemon

按 `Ctrl+C` 停止 Daemon。

**Step 6** - 验证离线状态

等待 90 秒以上，再次查询运行时表：

```bash
lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_RUNTIME_TABLE_ID
```

#### 预期结果

| 时间点 | 状态 | 最后心跳 | 说明 |
|--------|------|----------|------|
| 启动后 | 在线 | 当前时间 | 刚注册/心跳 |
| 运行中 | 在线 | 持续更新 | 每 60 秒更新 |
| 停止后 < 90s | 在线 | 停止前时间 | 心跳超时检测未触发 |
| 停止后 > 90s | **离线** | 停止前时间 | 超过 90 秒无心跳，标记离线 |

#### 验证方法

1. **注册验证**:
   ```bash
   lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_RUNTIME_TABLE_ID
   ```
   启动后立即查询，确认记录存在且状态为"在线"
   - [截图占位] 运行时表 - 刚启动时的记录

2. **心跳更新验证**:
   ```bash
   # 等待60秒后再次查询
   lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_RUNTIME_TABLE_ID
   ```
   确认 `最后心跳` 时间已更新
   - [截图占位] 运行时表 - 心跳更新后的记录

3. **离线检测验证**:
   ```bash
   # 停止Daemon后等待90秒以上再查询
   lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_RUNTIME_TABLE_ID
   ```
   确认 `状态` = `离线`
   - [截图占位] 运行时表 - 离线状态的记录

> **注意**: 离线状态的自动更新可能需要 Base 中的公式字段或工作流支持。如果项目中未配置自动离线检测，需手动在 Base 中添加公式字段或定时工作流来实现。

---

## 5. 常见问题排查

### 5.1 Daemon 无法获取任务

**现象**: Daemon 启动后日志显示 `Found 0 pending tasks`

**排查步骤**:
1. 确认 `config.toml` 中的 `runtime_id` 与任务"执行者"字段一致
2. 确认任务状态为"待办"
3. 确认任务的"执行者类型"为 `agent`
4. 检查 lark-cli 查询任务是否存在：
   ```bash
   lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_TASK_TABLE_ID
   ```
5. 检查 Base API 过滤条件是否正确：
   ```
   AND(CurrentValue.[任务状态]="待办",CurrentValue.[执行者]="agentman-test-001")
   ```

### 5.2 仓库克隆失败

**现象**: Daemon 日志显示 `Failed to setup repository`

**排查步骤**:
1. 确认仓库地址可访问（`git clone <url>` 测试）
2. 检查是否有仓库访问权限（私有仓库需配置 SSH key 或 token）
3. 检查 `workspace_dir` 路径是否存在且可写
4. 检查磁盘空间是否充足

### 5.3 Agent CLI 执行失败

**现象**: 任务状态变为"已取消"，执行记录显示"失败"

**排查步骤**:
1. 确认 Agent CLI 已安装且在 PATH 中：
   ```bash
   which claude
   which codex
   which opencode
   ```
2. 检查执行记录表中的"错误信息"字段
3. 检查"执行输出"字段获取详细日志
4. 手动测试 Agent CLI 是否可用：
   ```bash
   claude --version
   ```

### 5.4 工作流未触发

**现象**: 填写驳回理由后状态未自动重置

**排查步骤**:
1. 确认工作流 `YOUR_WORKFLOW_ID_1` 已启用
2. 确认任务状态为"待审核"（工作流分支条件检查）
3. 在工作流编辑器中查看执行历史
4. 检查 Base 是否有工作流执行限制（如触发频率限制）

### 5.5 Token 过期或 API 限流

**现象**: Daemon 日志显示 HTTP 401 或 429 错误

**排查步骤**:
1. 确认 `app_id` 和 `app_secret` 正确
2. 确认应用有 Base 操作权限
3. 检查是否触发了飞书 API 限流（减少轮询频率或增加重试间隔）
4. 检查 token 缓存是否正常刷新

### 5.6 多 Daemon 竞争同一任务

**现象**: 两个 Daemon 同时执行同一个任务

**排查步骤**:
1. 确认每个任务都已正确设置"分配的运行时"字段
2. 确认两个 Daemon 使用不同的 `runtime_id`
3. 确认任务的"执行者"字段与目标 Daemon 的 `runtime_id` 匹配
4. 在 Base 中检查任务的"分配的运行时"关联记录是否正确

---

## 6. 测试数据重置

### 6.1 删除测试任务

```bash
# 查询所有测试任务（按任务标题过滤）
lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_TASK_TABLE_ID

# 删除指定测试记录（谨慎操作）
lark-cli base +record-delete --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_TASK_TABLE_ID \
  --record-id REC_ID_1

lark-cli base +record-delete --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_TASK_TABLE_ID \
  --record-id REC_ID_2
```

### 6.2 删除测试执行记录

```bash
# 查询所有执行记录
lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_EXECUTION_LOG_TABLE_ID

# 批量删除（需要逐个删除）
for rec_id in REC_ID_1 REC_ID_2 REC_ID_3; do
  lark-cli base +record-delete --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_EXECUTION_LOG_TABLE_ID \
    --record-id "$rec_id"
done
```

### 6.3 清理运行时记录

```bash
# 查询所有运行时记录
lark-cli base +record-list --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_RUNTIME_TABLE_ID

# 删除测试运行时记录
lark-cli base +record-delete --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_RUNTIME_TABLE_ID \
  --record-id REC_RUNTIME_ID
```

### 6.4 批量重置脚本

```bash
#!/bin/bash
# reset_test_data.sh
# 批量删除测试数据（请根据实际情况修改 REC_IDS）

BASE_TOKEN="YOUR_BASE_TOKEN_HERE"
TASK_TABLE="YOUR_TASK_TABLE_ID"
LOG_TABLE="YOUR_EXECUTION_LOG_TABLE_ID"
RUNTIME_TABLE="YOUR_RUNTIME_TABLE_ID"

# 删除任务（替换为实际 record_id）
TASK_RECS=("rec1" "rec2" "rec3")
for rec in "${TASK_RECS[@]}"; do
  echo "Deleting task: $rec"
  lark-cli base +record-delete --base-token "$BASE_TOKEN" --table-id "$TASK_TABLE" --record-id "$rec"
done

# 删除执行记录
LOG_RECS=("recA" "recB" "recC")
for rec in "${LOG_RECS[@]}"; do
  echo "Deleting log: $rec"
  lark-cli base +record-delete --base-token "$BASE_TOKEN" --table-id "$LOG_TABLE" --record-id "$rec"
done

# 删除运行时记录
RUNTIME_RECS=("recR1" "recR2")
for rec in "${RUNTIME_RECS[@]}"; do
  echo "Deleting runtime: $rec"
  lark-cli base +record-delete --base-token "$BASE_TOKEN" --table-id "$RUNTIME_TABLE" --record-id "$rec"
done

echo "Test data reset complete."
```

### 6.5 快速创建标准测试任务

```bash
#!/bin/bash
# create_test_task.sh
# 快速创建标准 E2E 测试任务

BASE_TOKEN="YOUR_BASE_TOKEN_HERE"
TASK_TABLE="YOUR_TASK_TABLE_ID"

RUNTIME_ID="${1:-agentman-test-001}"
AGENT_TYPE="${2:-claude-code}"
REPO_URL="${3:-https://github.com/example/test-repo}"

echo "Creating test task for runtime: $RUNTIME_ID"

lark-cli base +record-create --base-token "$BASE_TOKEN" --table-id "$TASK_TABLE" \
  --json "{
    \"任务标题\": \"E2E-自动创建测试任务\",
    \"任务描述\": \"这是通过脚本自动创建的测试任务。\",
    \"执行者类型\": \"agent\",
    \"执行者\": \"$RUNTIME_ID\",
    \"任务状态\": \"待办\",
    \"优先级\": \"P1\",
    \"Agent类型\": \"$AGENT_TYPE\",
    \"仓库地址\": \"$REPO_URL\",
    \"分支名称\": \"main\",
    \"预计工时\": 1,
    \"重试次数\": 0,
    \"催办次数\": 0
  }"

echo "Test task created."
```

---

## 7. 附录

### 7.1 工作流详情

#### 审核驳回自动重试 (YOUR_WORKFLOW_ID_1)

**触发器**: SetRecordTrigger
- 监听表: 任务主表
- 监听字段: 审核驳回理由
- 条件: 字段值 isNotEmpty

**流程**:
```
触发器(审核驳回理由变更) → IfElse(状态是否为待审核)
  ├─ true → 重置状态为待办 → 通知创建人
  └─ false → 记录日志(状态不为待审核)
```

**通知内容模板**:
```
标题: 任务审核驳回
内容: 任务 [任务标题] 被驳回，已自动重置为待办状态，Daemon将重新执行。
      驳回理由: [审核驳回理由]
```

#### 任务催办提醒 (YOUR_WORKFLOW_ID_2)

**触发器**: ReminderTrigger
- 监听表: 任务主表
- 监听字段: 截止时间
- 偏移: 1 天前
- 触发时间: 09:00

**流程**:
```
触发器(截止前1天09:00) → IfElse(执行者类型是否为human)
  ├─ true → 更新最后催办时间=now → 发送催办通知给审核人
  └─ false → 发送消息给创建人(Agent任务自动执行中)
```

**催办通知模板**:
```
标题: 任务催办提醒
内容: 您有一个任务即将截止：
      任务：[任务标题]
      截止时间：[截止时间]
      请及时处理。
按钮: [查看任务] → 打开任务链接
```

**Agent 跳过通知模板**:
```
标题: Agent任务自动执行中
内容: 任务 [任务标题] 由Agent执行中，无需催办。
```

### 7.2 核心字段 ID 速查

#### 任务主表 (YOUR_TASK_TABLE_ID)

| 字段名 | 字段 ID | 类型 |
|--------|---------|------|
| ID (自动编号) | `YOUR_ID_FIELD_ID` | auto_number |
| 任务标题 | `YOUR_TITLE_FIELD_ID` | text |
| 任务描述 | `YOUR_DESCRIPTION_FIELD_ID` | text |
| 执行者类型 | `YOUR_EXECUTOR_TYPE_FIELD_ID` | select |
| 执行者 | `YOUR_EXECUTOR_FIELD_ID` | text |
| 任务状态 | `YOUR_STATUS_FIELD_ID` | select |
| 优先级 | `YOUR_PRIORITY_FIELD_ID` | select |
| 开始时间 | `YOUR_START_TIME_FIELD_ID` | datetime |
| 截止时间 | `YOUR_DEADLINE_FIELD_ID` | datetime |
| 完成时间 | `YOUR_COMPLETION_TIME_FIELD_ID` | datetime |
| 最后催办时间 | `YOUR_LAST_URGE_FIELD_ID` | datetime |
| Agent类型 | `YOUR_AGENT_TYPE_FIELD_ID` | select |
| 工作目录 | `YOUR_WORKSPACE_FIELD_ID` | text |
| 仓库地址 | `YOUR_REPO_URL_FIELD_ID` | text |
| 分支名称 | `YOUR_BRANCH_FIELD_ID` | text |
| 审核人 | `YOUR_REVIEWER_FIELD_ID` | user |
| 审核意见 | `YOUR_REVIEW_COMMENT_FIELD_ID` | text |
| 审核驳回理由 | `YOUR_REJECTION_REASON_FIELD_ID` | text |
| 重试次数 | `YOUR_RETRY_COUNT_FIELD_ID` | number |
| 催办次数 | `YOUR_URGE_COUNT_FIELD_ID` | number |
| 预计工时 | `YOUR_ESTIMATED_HOURS_FIELD_ID` | number |
| 分配的运行时 | `YOUR_ASSIGNED_RUNTIME_FIELD_ID` | link |

#### 运行时表 (YOUR_RUNTIME_TABLE_ID)

| 字段名 | 字段 ID | 类型 |
|--------|---------|------|
| ID | `YOUR_RUNTIME_ID_AUTO_FIELD_ID` | auto_number |
| 运行时ID | `YOUR_RUNTIME_ID_FIELD_ID` | text |
| 主机名 | `YOUR_HOSTNAME_FIELD_ID` | text |
| IP地址 | `YOUR_IP_ADDRESS_FIELD_ID` | text |
| 可用Agent | `YOUR_AVAILABLE_AGENTS_FIELD_ID` | text |
| 状态 | `YOUR_RUNTIME_STATUS_FIELD_ID` | select |
| 最后心跳 | `YOUR_LAST_HEARTBEAT_FIELD_ID` | datetime |
| 操作系统 | `YOUR_OS_FIELD_ID` | text |
| 版本号 | `YOUR_VERSION_FIELD_ID` | text |
| 关联任务 | `YOUR_LINKED_TASKS_FIELD_ID` | link |

#### 执行记录表 (YOUR_EXECUTION_LOG_TABLE_ID)

| 字段名 | 字段 ID | 类型 |
|--------|---------|------|
| ID | `YOUR_EXEC_LOG_ID_AUTO_FIELD_ID` | auto_number |
| 关联任务 | `YOUR_EXEC_LOG_TASK_LINK_FIELD_ID` | link |
| 执行序号 | `YOUR_EXEC_LOG_SEQUENCE_FIELD_ID` | number |
| Agent类型 | `YOUR_EXEC_LOG_AGENT_TYPE_FIELD_ID` | select |
| 执行状态 | `YOUR_EXEC_LOG_STATUS_FIELD_ID` | select |
| 开始时间 | `YOUR_EXEC_LOG_START_TIME_FIELD_ID` | datetime |
| 结束时间 | `YOUR_EXEC_LOG_END_TIME_FIELD_ID` | datetime |
| 执行输出 | `YOUR_EXEC_LOG_OUTPUT_FIELD_ID` | text |
| 错误信息 | `YOUR_EXEC_LOG_ERROR_FIELD_ID` | text |
| 提交记录 | `YOUR_EXEC_LOG_COMMIT_FIELD_ID` | text |
| 触发方式 | `fldapMtbZ` | select |

### 7.3 Daemon CLI 参数参考

```
agentman-daemon

Usage: agentman-daemon [OPTIONS]

Options:
  -c, --config <CONFIG>  Configuration file path
  -o, --once             Run once and exit
  -r, --register         Register this runtime
  -h, --help             Print help
  -V, --version          Print version
```

### 7.4 状态流转图

```
                    ┌─────────┐
                    │  已取消  │
                    └─────────┘
                         ↑
    ┌─────────┐    ┌────┴────┐    ┌─────────┐    ┌─────────┐
    │  待办   │───→│ 进行中  │───→│ 待审核  │───→│ 已完成  │
    └─────────┘    └────┬────┘    └────┬────┘    └─────────┘
         │              │              │
         │              │              └──── 审核驳回 ────┘
         │              │              (重试次数+1)        │
         │              │              (重试≤3)            │
         │              │                         │
         │              │                         ↓
         │              │←───────────────────────┘
         │              │ (Daemon自动重新执行)
         │              │ (携带驳回理由)
         │              │
         │              └── 催办（仅human）──→ 通知
         │
         └── 预分配（agent）──→ Daemon轮询执行
```

### 7.5 测试执行检查清单

- [ ] TC-E2E-01: 正常 Agent 任务流通过
- [ ] TC-E2E-02: 审核通过通过
- [ ] TC-E2E-03: 审核驳回 + 自动重试通过
- [ ] TC-E2E-04: 人类任务催办通过
- [ ] TC-E2E-05: Agent 任务催办过滤通过
- [ ] TC-E2E-06: 多 Daemon 隔离执行通过
- [ ] TC-E2E-07: 心跳与离线检测通过
- [ ] 所有测试数据已清理
- [ ] 测试报告已归档

---

*文档结束*
