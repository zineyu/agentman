# 飞书Base数据层设计文档

> 创建时间: 2025-04-25
> Base名称: Agent任务管理系统
> Base Token: `YOUR_BASE_TOKEN_HERE`
> 访问链接: https://dcnn71d2pd2o.feishu.cn/base/YOUR_BASE_TOKEN_HERE

## 概述

数据层采用飞书多维表格作为唯一数据源，Agent Daemon直接通过lark-cli读写Base。Bot身份已授予用户`full_access`权限。

---

## 表1: 任务主表 (Tasks)

**表ID**: `YOUR_TASK_TABLE_ID`

核心任务管理表，记录所有任务的完整生命周期。

### 字段清单

| 字段名 | 类型 | 说明 | 字段ID |
|--------|------|------|--------|
| ID | auto_number | 自动编号 (NO.001) | YOUR_ID_FIELD_ID |
| 任务标题 | text | 简短描述任务内容 | YOUR_TITLE_FIELD_ID |
| 任务描述 | text | 详细需求描述，支持多行文本 | YOUR_DESCRIPTION_FIELD_ID |
| 执行者类型 | select(单选) | `human` / `agent` | YOUR_EXECUTOR_TYPE_FIELD_ID |
| 执行者 | text | human填人员标识，agent填daemon-id | YOUR_EXECUTOR_FIELD_ID |
| 任务状态 | select(单选) | 待办/进行中/待审核/已完成/已取消 | YOUR_STATUS_FIELD_ID |
| 优先级 | select(单选) | P0/P1/P2/P3 | YOUR_PRIORITY_FIELD_ID |
| 开始时间 | datetime | 任务开始执行时间 | YOUR_START_TIME_FIELD_ID |
| 截止时间 | datetime | 期望完成时间 | YOUR_DEADLINE_FIELD_ID |
| 完成时间 | datetime | 实际完成时间 | YOUR_COMPLETION_TIME_FIELD_ID |
| 最后催办时间 | datetime | 最后一次催办的时间戳 | YOUR_LAST_URGE_FIELD_ID |
| Agent类型 | select(单选) | claude-code/codex/opencode/cursor/其他 | YOUR_AGENT_TYPE_FIELD_ID |
| 工作目录 | text | Agent执行时的本地工作目录路径 | YOUR_WORKSPACE_FIELD_ID |
| 前置任务 | link(单向) | 指向任务主表，表示执行前必须完成的任务 | YOUR_DEPENDENCIES_FIELD_ID |
| 审核人 | user(单选) | 待审核状态的审核责任人 | YOUR_REVIEWER_FIELD_ID |
| 审核意见 | text | 审核通过时的反馈或建议 | YOUR_REVIEW_COMMENT_FIELD_ID |
| 审核驳回理由 | text | 审核驳回时的具体原因，Agent重试时会携带此上下文 | YOUR_REJECTION_REASON_FIELD_ID |
| 重试次数 | number(整数) | 审核驳回后重新执行的次数，最大3次 | YOUR_RETRY_COUNT_FIELD_ID |
| 催办次数 | number(整数) | 催办统计，agent任务不增加 | YOUR_URGE_COUNT_FIELD_ID |
| 预计工时 | number(1位小数) | 预计需要的小时数 | YOUR_ESTIMATED_HOURS_FIELD_ID |
| 分配的运行时 | link(双向) | 预分配的Agent Daemon运行时，关联运行时表 | YOUR_ASSIGNED_RUNTIME_FIELD_ID |

### 状态流转规则

```
待办 → 进行中 → 待审核 → 已完成
       ↓           ↓
      已取消      进行中(驳回)
```

- **待办→进行中**: 执行者开始处理
- **进行中→待审核**: Agent任务完成后自动流转
- **待审核→已完成**: 人工审核通过
- **待审核→进行中**: 审核驳回，Agent自动重试(最多3次)
- **任意→已取消**: 人工取消

---

## 表2: 运行时表 (Runtimes)

**表ID**: `YOUR_RUNTIME_TABLE_ID`

Agent Daemon运行时注册表，记录所有在线Daemon的状态和能力。

### 字段清单

| 字段名 | 类型 | 说明 | 字段ID |
|--------|------|------|--------|
| ID | auto_number | 自动编号 | YOUR_RUNTIME_ID_AUTO_FIELD_ID |
| 运行时ID | text | Daemon的唯一标识UUID | YOUR_RUNTIME_ID_FIELD_ID |
| 主机名 | text | 机器标识 | YOUR_HOSTNAME_FIELD_ID |
| IP地址 | text | Daemon所在机器的IP | YOUR_IP_ADDRESS_FIELD_ID |
| 可用Agent | text | 逗号分隔的CLI列表，如claude,codex,opencode | YOUR_AVAILABLE_AGENTS_FIELD_ID |
| 状态 | select(单选) | 在线/离线/忙碌 | YOUR_RUNTIME_STATUS_FIELD_ID |
| 最后心跳 | datetime | 最后一次心跳上报时间 | YOUR_LAST_HEARTBEAT_FIELD_ID |
| 操作系统 | text | 如Linux/macOS/Windows | YOUR_OS_FIELD_ID |
| 版本号 | text | Daemon版本号 | YOUR_VERSION_FIELD_ID |
| 关联任务 | link(双向) | 反向关联任务主表的"分配的运行时"字段 | YOUR_LINKED_TASKS_FIELD_ID |

### 心跳机制

- Daemon每30秒上报一次心跳
- 超过90秒无心跳自动标记为"离线"
- 正在执行任务时标记为"忙碌"

---

## 表3: 执行记录表 (ExecutionLogs)

**表ID**: `YOUR_EXECUTION_LOG_TABLE_ID`

记录每次任务执行的详细日志，支持追踪和审计。

### 字段清单

| 字段名 | 类型 | 说明 | 字段ID |
|--------|------|------|--------|
| ID | auto_number | 自动编号 | YOUR_EXEC_LOG_ID_AUTO_FIELD_ID |
| 关联任务 | link(单向) | 指向任务主表 | YOUR_EXEC_LOG_TASK_LINK_FIELD_ID |
| 执行序号 | number(整数) | 第几次执行尝试 | YOUR_EXEC_LOG_SEQUENCE_FIELD_ID |
| Agent类型 | select(单选) | 实际使用的Agent CLI | YOUR_EXEC_LOG_AGENT_TYPE_FIELD_ID |
| 执行状态 | select(单选) | 成功/失败/进行中/超时 | YOUR_EXEC_LOG_STATUS_FIELD_ID |
| 开始时间 | datetime | 执行开始时间 | YOUR_EXEC_LOG_START_TIME_FIELD_ID |
| 结束时间 | datetime | 执行结束时间 | YOUR_EXEC_LOG_END_TIME_FIELD_ID |
| 执行输出 | text | Agent的标准输出日志 | YOUR_EXEC_LOG_OUTPUT_FIELD_ID |
| 错误信息 | text | 错误日志和异常信息 | YOUR_EXEC_LOG_ERROR_FIELD_ID |
| 触发方式 | select(单选) | 手动/自动/工作流 | fldapMtbZ |

---

## 表关系

```
任务主表 ──link(双向)──→ 运行时表
   │
   │ link(单向)
   ↓
执行记录表
```

---

## 使用说明

### 写入记录格式

使用 `lark-cli base +record-upsert` 命令，值格式为简化的JSON（非lark API原生格式）：

```json
{
  "任务标题": "实现用户认证模块",
  "任务描述": "为系统添加JWT认证支持",
  "执行者类型": "agent",
  "执行者": "daemon-prod-001",
  "任务状态": "待办",
  "优先级": "P1",
  "Agent类型": "claude-code",
  "工作目录": "/workspace/my-project",
  "预计工时": 4,
  "重试次数": 0,
  "催办次数": 0
}
```

### 各类型值格式

| 字段类型 | 写入值格式 | 示例 |
|----------|-----------|------|
| text | 字符串 | `"任务标题"` |
| number | 数字 | `4` |
| select(单选) | 字符串 | `"待办"` |
| select(多选) | 字符串数组 | `["后端", "高优"]` |
| datetime | `"YYYY-MM-DD HH:mm:ss"` | `"2025-04-25 14:00:00"` |
| user | `[{"id": "ou_xxx"}]` | `[{"id": "ou_abc123"}]` |
| link | `[{"id": "rec_xxx"}]` | `[{"id": "recABC123"}]` |

---

## 创建脚本

以下命令用于重新创建表结构（保留作参考）：

```bash
# 创建Base
lark-cli base +base-create --name "Agent任务管理系统"

# 创建任务主表
lark-cli base +table-create --base-token YOUR_BASE_TOKEN_HERE --name "任务主表"

# 添加字段（示例）
lark-cli base +field-create --base-token YOUR_BASE_TOKEN_HERE --table-id YOUR_TASK_TABLE_ID \
  --json '{"type":"text","name":"任务标题"}'

# 完整字段创建命令见实际操作记录
```

---

## 注意事项

1. **并发限制**: `+xxx-list` 命令禁止并发调用，需串行执行
2. **批量写入**: 单次最多500条记录，建议串行写入并在批次间延迟0.5-1秒
3. **认证**: 使用bot身份（tenant_access_token），具有完整Base操作权限
4. **字段只读性**: 公式/lookup/系统字段（创建时间、更新人等）不可写入
5. **测试记录**: 已创建1条测试记录（record_id: YOUR_RECORD_ID_5）用于验证
