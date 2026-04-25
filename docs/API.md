# Agentman API Documentation

This document describes the internal APIs, data models, and error handling of the Agentman Daemon.

## Table of Contents

- [BaseClient](#baseclient)
- [Data Models](#data-models)
- [Error Handling](#error-handling)
- [Rate Limiting](#rate-limiting)
- [Table Schemas](#table-schemas)

---

## BaseClient

The `BaseClient` is the core HTTP client for interacting with Lark Base OpenAPI. It provides token caching, automatic refresh, and retry logic.

### Construction

```rust
use agentman_daemon::{client::BaseClient, config::DaemonConfig};

let config = DaemonConfig::load(Some("config.toml"))?;
let client = BaseClient::new(&config)?;
```

### HTTP Client Configuration

The underlying `reqwest::Client` is configured with:
- **Timeout**: 30 seconds per request
- **Pool idle timeout**: 60 seconds
- **Connection pooling**: Enabled for reuse across requests

### Methods

#### 1. `get_pending_tasks`

Fetches tasks assigned to this runtime with status "待办".

```rust
pub async fn get_pending_tasks(
    &self,
    runtime_id: &str
) -> Result<Vec<Task>, BaseClientError>
```

**API Call**:
- **Method**: GET
- **Path**: `/open-apis/bitable/v1/apps/{base_token}/tables/{task_table_id}/records`
- **Query Parameters**:
  - `filter`: `AND(CurrentValue.[任务状态]="待办",CurrentValue.[执行者]="{runtime_id}")`
  - `page_size`: `500`
  - `automatic_fields`: `true`

**Returns**: Vector of `Task` structs parsed from API response `data.items`.

**Example**:
```rust
let tasks = client.get_pending_tasks("agentman-prod-01").await?;
for task in tasks {
    println!("Task #{}: {}", task.id, task.title);
}
```

---

#### 2. `update_task_status`

Updates task status and appends to the execution log field.

```rust
pub async fn update_task_status(
    &self,
    task_id: &str,
    status: &str,
    execution_log: &str
) -> Result<(), BaseClientError>
```

**Behavior**:
1. Reads current value of "执行日志" field
2. Appends new log entry with timestamp: `[YYYY-MM-DD HH:MM:SS] {execution_log}`
3. Updates both "任务状态" and "执行日志" fields

**API Call**:
- **Method**: PUT
- **Path**: `/open-apis/bitable/v1/apps/{base_token}/tables/{task_table_id}/records/{task_id}`
- **Body**:
```json
{
  "fields": {
    "任务状态": "进行中",
    "执行日志": "[2024-01-15 10:30:00] Task started\n[2024-01-15 10:35:00] Execution complete"
  }
}
```

---

#### 3. `clear_task_rejection_reason`

Clears the review rejection reason field after processing a retry.

```rust
pub async fn clear_task_rejection_reason(
    &self,
    task_id: &str
) -> Result<(), BaseClientError>
```

**API Call**:
- **Method**: PUT
- **Path**: `/open-apis/bitable/v1/apps/{base_token}/tables/{task_table_id}/records/{task_id}`
- **Body**:
```json
{
  "fields": {
    "审核驳回理由": ""
  }
}
```

---

#### 4. `register_runtime`

Creates a new record in the Runtimes table.

```rust
pub async fn register_runtime(
    &self,
    runtime_info: &RuntimeInfo
) -> Result<(), BaseClientError>
```

**API Call**:
- **Method**: POST
- **Path**: `/open-apis/bitable/v1/apps/{base_token}/tables/{runtime_table_id}/records`
- **Body**:
```json
{
  "fields": {
    "运行时ID": "agentman-prod-01",
    "运行时名称": "Production Daemon #1",
    "主机名": "web-server-01",
    "IP地址": "192.168.1.100",
    "状态": "在线",
    "最后心跳": 1705313400000,
    "操作系统": "linux",
    "版本号": "0.1.0"
  }
}
```

**Note**: Timestamp is in milliseconds (UTC).

---

#### 5. `update_heartbeat`

Updates the last heartbeat timestamp and status for a runtime.

```rust
pub async fn update_heartbeat(
    &self,
    runtime_info: &RuntimeInfo
) -> Result<(), BaseClientError>
```

**Behavior**:
1. Searches for existing runtime record by `runtime_id`
2. If not found, calls `register_runtime` instead
3. Updates "最后心跳" and "状态" fields

**API Call**:
- **Method**: PUT
- **Path**: `/open-apis/bitable/v1/apps/{base_token}/tables/{runtime_table_id}/records/{record_id}`
- **Body**:
```json
{
  "fields": {
    "最后心跳": 1705313460000,
    "状态": "在线"
  }
}
```

---

#### 6. `create_execution_log`

Creates a new execution history record.

```rust
pub async fn create_execution_log(
    &self,
    log: &ExecutionLog
) -> Result<String, BaseClientError>
```

**Returns**: The `record_id` of the newly created log entry.

**API Call**:
- **Method**: POST
- **Path**: `/open-apis/bitable/v1/apps/{base_token}/tables/{execution_log_table_id}/records`
- **Body**:
```json
{
  "fields": {
    "关联任务": ["rec_abc123"],
    "执行序号": 1,
    "Agent类型": "claude-code",
    "执行状态": "进行中",
    "开始时间": 1705313400000,
    "结束时间": null,
    "执行输出": "",
    "错误信息": "",
    "提交记录": "",
    "触发方式": "自动"
  }
}
```

---

#### 7. `update_execution_log`

Updates an existing execution log with final results.

```rust
pub async fn update_execution_log(
    &self,
    record_id: &str,
    log: &ExecutionLog
) -> Result<(), BaseClientError>
```

**API Call**:
- **Method**: PUT
- **Path**: `/open-apis/bitable/v1/apps/{base_token}/tables/{execution_log_table_id}/records/{record_id}`
- **Body**:
```json
{
  "fields": {
    "执行状态": "成功",
    "结束时间": 1705313700000,
    "执行输出": "Build completed successfully...",
    "错误信息": "",
    "提交记录": "a1b2c3d4"
  }
}
```

---

### Internal Methods

#### `get_access_token`

Fetches and caches the Lark `tenant_access_token`.

```rust
pub(crate) async fn get_access_token(&self) -> Result<String, BaseClientError>
```

**Token Lifecycle**:
1. Check cache — if valid for >5 minutes, return cached token
2. Acquire write lock (double-checked locking pattern)
3. Call `/open-apis/auth/v3/tenant_access_token/internal`
4. Parse response: `tenant_access_token` + `expire` (seconds)
5. Store in cache with expiration `Instant`

**API Call**:
- **Method**: POST
- **Path**: `/open-apis/auth/v3/tenant_access_token/internal`
- **Body**:
```json
{
  "app_id": "cli_xxx",
  "app_secret": "xxx"
}
```

#### `api_request`

Low-level API request method with comprehensive retry logic.

```rust
async fn api_request(
    &self,
    method: reqwest::Method,
    path: &str,
    body: Option<Value>,
    query: Option<Vec<(&str, String)>>,
) -> Result<Value, BaseClientError>
```

**Retry Logic**:
- **Max retries**: 3
- **Retry conditions**:
  - HTTP 429 (Too Many Requests)
  - HTTP 401 (Unauthorized) — clears token cache
  - API error codes: `1254290`, `1254291`, `1255040`, `1254607`
  - Network timeout or connection errors
- **Backoff**: Exponential, `2^attempt + 1` seconds
  - Attempt 1: 2s
  - Attempt 2: 5s
  - Attempt 3: 9s

---

## Data Models

### Task (任务主表)

Represents a task record in the Tasks table.

| Field | Rust Type | Base Field | Description |
|-------|-----------|------------|-------------|
| `record_id` | `String` | — | Base API record ID |
| `id` | `u64` | 自动编号 | Auto-incrementing task number |
| `title` | `String` | 任务标题 | Task title |
| `description` | `String` | 任务描述 | Task description/prompt |
| `executor_type` | `ExecutorType` | 执行者类型 | `Human` or `Agent` |
| `executor` | `String` | 执行者 | Human ID or daemon runtime_id |
| `status` | `Status` | 任务状态 | Todo/InProgress/PendingReview/Completed/Cancelled |
| `priority` | `Priority` | 优先级 | P0/P1/P2/P3 |
| `start_time` | `Option<NaiveDateTime>` | 开始时间 | Task start time |
| `deadline` | `Option<NaiveDateTime>` | 截止时间 | Task deadline |
| `completed_at` | `Option<NaiveDateTime>` | 完成时间 | Completion timestamp |
| `last_urge_time` | `Option<NaiveDateTime>` | 最后催办时间 | Last urge reminder time |
| `agent_type` | `Option<AgentType>` | Agent类型 | ClaudeCode/Codex/Opencode/Cursor/Other |
| `work_dir` | `String` | 工作目录 | Working directory hint |
| `repo_url` | `String` | 仓库地址 | Git repository URL |
| `branch` | `String` | 分支名称 | Git branch name |
| `reviewer` | `Option<String>` | 审核人 | Reviewer identifier |
| `review_comment` | `String` | 审核意见 | Review comments |
| `review_rejection_reason` | `String` | 审核驳回理由 | Rejection reason |
| `retry_count` | `u32` | 重试次数 | Current retry count (max 3) |
| `urge_count` | `u32` | 催办次数 | Urge reminder count |
| `estimated_hours` | `f64` | 预计工时 | Estimated hours (1 decimal) |
| `assigned_runtime` | `Vec<LinkRecord>` | 分配的运行时 | Linked runtime record(s) |

**Table ID**: `YOUR_TASK_TABLE_ID`

### RuntimeInfo (运行时表)

Represents a registered daemon runtime.

| Field | Rust Type | Base Field | Description |
|-------|-----------|------------|-------------|
| `id` | `u64` | 自动编号 | Auto-incrementing runtime number |
| `runtime_id` | `String` | 运行时ID | Daemon UUID |
| `runtime_name` | `String` | 运行时名称 | Human-readable name |
| `hostname` | `String` | 主机名 | Machine hostname |
| `ip_address` | `String` | IP地址 | Local IP address |
| `available_agents` | `String` | 可用Agent | Comma-separated CLI list |
| `status` | `RuntimeStatus` | 状态 | Online/Offline/Busy |
| `last_heartbeat` | `NaiveDateTime` | 最后心跳 | Last heartbeat timestamp |
| `os` | `String` | 操作系统 | Linux/macOS/Windows |
| `version` | `String` | 版本号 | Daemon version |
| `linked_tasks` | `Vec<LinkRecord>` | 关联任务 | Reverse-linked tasks |

**Table ID**: `YOUR_RUNTIME_TABLE_ID`

### ExecutionLog (执行记录表)

Records each task execution attempt.

| Field | Rust Type | Base Field | Description |
|-------|-----------|------------|-------------|
| `id` | `u64` | 自动编号 | Auto-incrementing log number |
| `linked_task` | `Vec<LinkRecord>` | 关联任务 | Linked task record(s) |
| `execution_sequence` | `u32` | 执行序号 | Attempt number (1, 2, 3...) |
| `agent_type` | `AgentType` | Agent类型 | Which CLI was used |
| `execution_status` | `ExecutionStatus` | 执行状态 | Success/Failed/InProgress/Timeout |
| `start_time` | `NaiveDateTime` | 开始时间 | Execution start |
| `end_time` | `Option<NaiveDateTime>` | 结束时间 | Execution end |
| `execution_output` | `String` | 执行输出 | Full stdout/stderr output |
| `error_info` | `String` | 错误信息 | Error description |
| `commit_hash` | `String` | 提交记录 | Git commit hash |
| `trigger_mode` | `TriggerMode` | 触发方式 | Manual/Auto/Workflow |

**Table ID**: `YOUR_EXECUTION_LOG_TABLE_ID`

### Enums

#### ExecutorType

```rust
pub enum ExecutorType {
    Human,   // 人工执行
    Agent,   // Agent自动执行
}
```

Serialized as lowercase: `"human"`, `"agent"`.

#### Status

```rust
pub enum Status {
    Todo,           // 待办
    InProgress,     // 进行中
    PendingReview,  // 待审核
    Completed,      // 已完成
    Cancelled,      // 已取消
}
```

Serialized as Chinese strings.

#### AgentType

```rust
pub enum AgentType {
    ClaudeCode,  // claude-code
    Codex,       // codex
    Opencode,    // opencode
    Cursor,      // cursor
    Other,       // 其他
}
```

Serialized as kebab-case for English, Chinese for "Other".

#### ExecutionStatus

```rust
pub enum ExecutionStatus {
    Success,     // 成功
    Failed,      // 失败
    InProgress,  // 进行中
    Timeout,     // 超时
}
```

Serialized as Chinese strings.

#### TriggerMode

```rust
pub enum TriggerMode {
    Manual,   // 手动
    Auto,     // 自动
    Workflow, // 工作流
}
```

Serialized as Chinese strings.

#### RuntimeStatus

```rust
pub enum RuntimeStatus {
    Online,  // 在线
    Offline, // 离线
    Busy,    // 忙碌
}
```

Serialized as Chinese strings.

### LinkRecord

```rust
pub struct LinkRecord {
    pub id: String,  // Base record_id
}
```

Used for bi-directional table linking in Lark Base.

---

## Error Handling

### BaseClientError

```rust
#[derive(Debug, Error)]
pub enum BaseClientError {
    #[error("HTTP request failed: {0}")]
    HttpError(reqwest::Error),

    #[error("API error {code}: {msg}")]
    ApiError { code: i32, msg: String },

    #[error("Token refresh failed: {0}")]
    TokenRefreshError(String),

    #[error("Serialization error: {0}")]
    SerializationError(serde_json::Error),

    #[error("Max retries exceeded")]
    MaxRetriesExceeded,

    #[error("Record not found")]
    RecordNotFound,
}
```

### GitError

```rust
#[derive(Debug, Error)]
pub enum GitError {
    #[error("Git command failed: {0}")]
    CommandFailed(String),

    #[error("Invalid repository URL: {0}")]
    InvalidUrl(String),

    #[error("Repository not found at {0}")]
    RepoNotFound(String),

    #[error("IO error: {0}")]
    IoError(std::io::Error),

    #[error("UTF-8 decode error: {0}")]
    Utf8Error(std::string::FromUtf8Error),
}
```

### ConfigError

```rust
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(String),

    #[error("Failed to parse config: {0}")]
    ParseError(String),

    #[error("Missing required field: {0}")]
    MissingField(String),
}
```

### Error Propagation

The daemon uses a layered error approach:

1. **BaseClientError**: API-level errors with automatic retry
2. **GitError**: Git operation failures (non-retryable)
3. **ConfigError**: Configuration validation failures (fatal on startup)
4. **anyhow::Result**: Top-level error propagation with context

```rust
// Example: Task execution error handling
async fn process_single_task(&self, task: Task) -> anyhow::Result<()> {
    let tasks = self.client.get_pending_tasks(&self.config.runtime_id).await?;
    // BaseClientError -> automatically retried -> if max retries, returns Err
    
    self.git_manager.clone_repo(&task.repo_url, &repo_dir)?;
    // GitError -> immediately propagated, task marked as failed
    
    Ok(())
}
```

---

## Rate Limiting

### Lark Base API Limits

| Endpoint | Limit | Notes |
|----------|-------|-------|
| `tenant_access_token` | 20/min per app | Cached for token lifetime (~2 hours) |
| Read records | 1000/min per app | Batch with `page_size=500` |
| Write records | 500/min per app | Batched when possible |
| Search records | 500/min per app | Uses filter queries |

### Client-Side Rate Limiting

The `BaseClient` implements exponential backoff for rate-limited requests:

```
Attempt 1: Immediate
Attempt 2: Wait 2^1 + 1 = 3 seconds
Attempt 3: Wait 2^2 + 1 = 5 seconds
```

**Retryable conditions**:
- HTTP 429 (Too Many Requests)
- API code 1254290 / 1254291 (rate limit)
- API code 1255040 (write conflict)
- API code 1254607 (data not ready)
- Network timeout
- Connection errors

### Recommended Polling Intervals

| Scenario | `poll_interval_secs` | `heartbeat_interval_secs` |
|----------|---------------------|--------------------------|
| Development / Testing | 10-30 | 30 |
| Production (light load) | 30-60 | 60 |
| Production (heavy load) | 60-120 | 60 |
| Multiple daemons | 60+ | 60 |

### Concurrency Control

The `max_concurrent_tasks` config limits parallel execution. Note that tasks are processed sequentially within a single daemon; this parameter is reserved for future parallel execution support.

Currently, the daemon processes one task at a time per poll cycle.

---

## Table Schemas

### Tasks Table (任务主表)

**Table ID**: `YOUR_TASK_TABLE_ID`

| Field Name | Type | Required | Description |
|------------|------|----------|-------------|
| 自动编号 | AutoNumber | Yes | Task ID (NO.001 format) |
| 任务标题 | Text | Yes | Brief task description |
| 任务描述 | Text | Yes | Detailed instructions/prompt |
| 执行者类型 | SingleSelect | Yes | human / agent |
| 执行者 | Text | Yes | Person ID or runtime_id |
| 任务状态 | SingleSelect | Yes | 待办/进行中/待审核/已完成/已取消 |
| 优先级 | SingleSelect | Yes | P0/P1/P2/P3 |
| 开始时间 | DateTime | No | Task start timestamp |
| 截止时间 | DateTime | No | Task deadline |
| 完成时间 | DateTime | No | Completion timestamp |
| 最后催办时间 | DateTime | No | Last urge timestamp |
| Agent类型 | SingleSelect | No | claude-code/codex/opencode/cursor/其他 |
| 工作目录 | Text | No | Workspace directory path |
| 仓库地址 | Text | Yes | Git repository URL |
| 分支名称 | Text | Yes | Git branch to checkout |
| 审核人 | Text | No | Reviewer identifier |
| 审核意见 | Text | No | Review feedback |
| 审核驳回理由 | Text | No | Rejection reason (triggers retry) |
| 重试次数 | Number | No | Retry counter (0-3) |
| 催办次数 | Number | No | Urge counter |
| 预计工时 | Number | No | Estimated hours |
| 分配的运行时 | Link | No | Link to Runtime record |
| 执行日志 | Text | No | Execution log (appended by daemon) |

### Runtimes Table (运行时表)

**Table ID**: `YOUR_RUNTIME_TABLE_ID`

| Field Name | Type | Required | Description |
|------------|------|----------|-------------|
| 自动编号 | AutoNumber | Yes | Runtime ID |
| 运行时ID | Text | Yes | Daemon UUID |
| 运行时名称 | Text | Yes | Human-readable name |
| 主机名 | Text | Yes | Machine hostname |
| IP地址 | Text | Yes | Local IP address |
| 可用Agent | Text | Yes | Comma-separated CLI names |
| 状态 | SingleSelect | Yes | 在线/离线/忙碌 |
| 最后心跳 | DateTime | Yes | Last heartbeat timestamp |
| 操作系统 | Text | Yes | linux/macOS/windows |
| 版本号 | Text | Yes | Daemon version |
| 关联任务 | Link | No | Reverse link to Tasks |

### ExecutionLogs Table (执行记录表)

**Table ID**: `YOUR_EXECUTION_LOG_TABLE_ID`

| Field Name | Type | Required | Description |
|------------|------|----------|-------------|
| 自动编号 | AutoNumber | Yes | Log entry ID |
| 关联任务 | Link | Yes | Link to Task record |
| 执行序号 | Number | Yes | Execution attempt number |
| Agent类型 | SingleSelect | Yes | Which CLI was used |
| 执行状态 | SingleSelect | Yes | 成功/失败/进行中/超时 |
| 开始时间 | DateTime | Yes | Execution start |
| 结束时间 | DateTime | No | Execution end |
| 执行输出 | Text | No | Full stdout/stderr |
| 错误信息 | Text | No | Error description |
| 提交记录 | Text | No | Git commit hash |
| 触发方式 | SingleSelect | Yes | 手动/自动/工作流 |

---

## Workflows

### Rejection Retry Workflow (rejection-retry.json)

**Trigger**: When "审核驳回理由" field changes from empty to non-empty

**Actions**:
1. Increment "重试次数" by 1
2. If retry_count < 3:
   - Set status to "待办"
   - Daemon will pick up on next poll
3. If retry_count >= 3:
   - Set status to "已取消"

### Urge Reminder Workflow (urge-reminder.json)

**Trigger**: Scheduled or manual urge action

**Logic**:
- If `executor_type` == "Agent": Skip reminder (Agent tasks are auto-executed)
- If `executor_type` == "Human": Send notification via Lark message
- Increment "催办次数"
- Update "最后催办时间"

---

## Authentication Flow

```
┌─────────┐     POST /auth/v3/tenant_access_token/internal     ┌─────────┐
│ Daemon  │ ──────────────────────────────────────────────────> │  Lark   │
│         │  { app_id, app_secret }                             │  Auth   │
│         │ <────────────────────────────────────────────────── │ Server  │
│         │  { tenant_access_token, expire: 7200 }              │         │
└────┬────┘                                                    └─────────┘
     │
     │ Cache token with expiration
     │
     │ All subsequent API calls:
     │ Authorization: Bearer {tenant_access_token}
     │
     v
┌─────────┐     API Request with Bearer token                  ┌─────────┐
│ Daemon  │ ──────────────────────────────────────────────────> │  Lark   │
│         │                                                     │  Base   │
│         │ <────────────────────────────────────────────────── │  API    │
│         │  { code, data, msg }                                │         │
└─────────┘                                                    └─────────┘
```

**Token Refresh**:
- Token cached in `Arc<RwLock<Option<TokenInfo>>>`
- Refreshed when < 5 minutes remaining until expiry
- Double-checked locking prevents thundering herd
- HTTP 401 automatically clears cache and retries once
