use std::fmt;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

/// 执行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// 成功
    #[serde(rename = "成功")]
    Success,
    /// 失败
    #[serde(rename = "失败")]
    Failed,
    /// 进行中
    #[serde(rename = "进行中")]
    InProgress,
    /// 超时
    #[serde(rename = "超时")]
    Timeout,
}

impl fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionStatus::Success => write!(f, "成功"),
            ExecutionStatus::Failed => write!(f, "失败"),
            ExecutionStatus::InProgress => write!(f, "进行中"),
            ExecutionStatus::Timeout => write!(f, "超时"),
        }
    }
}

/// 触发方式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerMode {
    /// 手动触发
    #[serde(rename = "手动")]
    Manual,
    /// 自动触发
    #[serde(rename = "自动")]
    Auto,
    /// 工作流触发
    #[serde(rename = "工作流")]
    Workflow,
}

impl fmt::Display for TriggerMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TriggerMode::Manual => write!(f, "手动"),
            TriggerMode::Auto => write!(f, "自动"),
            TriggerMode::Workflow => write!(f, "工作流"),
        }
    }
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

impl fmt::Display for AgentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentType::ClaudeCode => write!(f, "claude-code"),
            AgentType::Codex => write!(f, "codex"),
            AgentType::Opencode => write!(f, "opencode"),
            AgentType::Cursor => write!(f, "cursor"),
            AgentType::Other => write!(f, "其他"),
        }
    }
}

/// 关联记录结构（用于link字段）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkRecord {
    pub id: String,
}

/// 执行记录表 (ExecutionLogs)
/// 表ID: YOUR_EXECUTION_LOG_TABLE_ID
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionLog {
    /// 自动编号
    pub id: u64,

    /// 关联任务 (指向任务主表)
    #[serde(rename = "关联任务")]
    pub linked_task: Vec<LinkRecord>,

    /// 执行序号 (第几次执行尝试)
    #[serde(rename = "执行序号")]
    pub execution_sequence: u32,

    /// 实际使用的Agent CLI
    #[serde(rename = "Agent类型")]
    pub agent_type: AgentType,

    /// 执行状态: 成功/失败/进行中/超时
    #[serde(rename = "执行状态")]
    pub execution_status: ExecutionStatus,

    /// 开始时间
    #[serde(rename = "开始时间")]
    pub start_time: NaiveDateTime,

    /// 结束时间
    #[serde(rename = "结束时间")]
    pub end_time: Option<NaiveDateTime>,

    /// 执行输出 (Agent标准输出日志)
    #[serde(rename = "执行输出")]
    pub execution_output: String,

    /// 错误信息
    #[serde(rename = "错误信息")]
    pub error_info: String,

    /// Git commit hash
    #[serde(rename = "提交记录")]
    pub commit_hash: String,

    /// 触发方式: 手动/自动/工作流
    #[serde(rename = "触发方式")]
    pub trigger_mode: TriggerMode,
}
