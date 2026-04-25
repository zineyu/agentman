use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

/// 执行者类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutorType {
    /// 人工执行
    Human,
    /// Agent自动执行
    Agent,
}

/// 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    /// 待办
    #[serde(rename = "待办")]
    Todo,
    /// 进行中
    #[serde(rename = "进行中")]
    InProgress,
    /// 待审核
    #[serde(rename = "待审核")]
    PendingReview,
    /// 已完成
    #[serde(rename = "已完成")]
    Completed,
    /// 已取消
    #[serde(rename = "已取消")]
    Cancelled,
}

/// 优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    /// P0 - 最高优先级
    P0,
    /// P1 - 高优先级
    P1,
    /// P2 - 中优先级
    P2,
    /// P3 - 低优先级
    P3,
}

/// Agent CLI类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentType {
    /// Claude Code
    ClaudeCode,
    /// GitHub Copilot/Codex CLI
    Codex,
    /// OpenCode
    Opencode,
    /// Cursor
    Cursor,
    /// 其他Agent
    #[serde(rename = "其他")]
    Other,
}

/// 关联记录结构（用于link字段）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkRecord {
    pub id: String,
}

/// 任务主表 (Tasks)
/// 表ID: YOUR_TASK_TABLE_ID
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// 记录ID (Base API record_id)
    pub record_id: String,

    /// 自动编号 (NO.001)
    pub id: u64,

    /// 任务标题
    #[serde(rename = "任务标题")]
    pub title: String,

    /// 任务描述
    #[serde(rename = "任务描述")]
    pub description: String,

    /// 执行者类型: human / agent
    #[serde(rename = "执行者类型")]
    pub executor_type: ExecutorType,

    /// 执行者标识 (human填人员标识, agent填daemon-id)
    #[serde(rename = "执行者")]
    pub executor: String,

    /// 任务状态
    #[serde(rename = "任务状态")]
    pub status: Status,

    /// 优先级: P0/P1/P2/P3
    #[serde(rename = "优先级")]
    pub priority: Priority,

    /// 开始时间
    #[serde(rename = "开始时间")]
    pub start_time: Option<NaiveDateTime>,

    /// 截止时间
    #[serde(rename = "截止时间")]
    pub deadline: Option<NaiveDateTime>,

    /// 完成时间
    #[serde(rename = "完成时间")]
    pub completed_at: Option<NaiveDateTime>,

    /// 最后催办时间
    #[serde(rename = "最后催办时间")]
    pub last_urge_time: Option<NaiveDateTime>,

    /// Agent CLI类型
    #[serde(rename = "Agent类型")]
    pub agent_type: Option<AgentType>,

    /// 工作目录
    #[serde(rename = "工作目录")]
    pub work_dir: String,

    /// 仓库地址 (Git URL)
    #[serde(rename = "仓库地址")]
    pub repo_url: String,

    /// 分支名称
    #[serde(rename = "分支名称")]
    pub branch: String,

    /// 审核人
    #[serde(rename = "审核人")]
    pub reviewer: Option<String>,

    /// 审核意见
    #[serde(rename = "审核意见")]
    pub review_comment: String,

    /// 审核驳回理由
    #[serde(rename = "审核驳回理由")]
    pub review_rejection_reason: String,

    /// 重试次数 (最大3次)
    #[serde(rename = "重试次数")]
    pub retry_count: u32,

    /// 催办次数
    #[serde(rename = "催办次数")]
    pub urge_count: u32,

    /// 预计工时 (小时, 1位小数)
    #[serde(rename = "预计工时")]
    pub estimated_hours: f64,

    /// 分配的运行时 (关联运行时表)
    #[serde(rename = "分配的运行时")]
    pub assigned_runtime: Vec<LinkRecord>,
}
