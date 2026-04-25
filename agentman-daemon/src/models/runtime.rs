use std::fmt;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::config::DaemonConfig;

/// 运行时状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeStatus {
    /// 在线
    #[serde(rename = "在线")]
    Online,
    /// 离线
    #[serde(rename = "离线")]
    Offline,
    /// 忙碌
    #[serde(rename = "忙碌")]
    Busy,
}

impl fmt::Display for RuntimeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeStatus::Online => write!(f, "在线"),
            RuntimeStatus::Offline => write!(f, "离线"),
            RuntimeStatus::Busy => write!(f, "忙碌"),
        }
    }
}

/// 关联记录结构（用于link字段）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkRecord {
    pub id: String,
}

/// 运行时表 (Runtimes)
/// 表ID: YOUR_RUNTIME_TABLE_ID
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeInfo {
    /// 自动编号
    pub id: u64,

    /// Daemon唯一标识UUID
    #[serde(rename = "运行时ID")]
    pub runtime_id: String,

    /// 运行时名称
    #[serde(rename = "运行时名称")]
    pub runtime_name: String,

    /// 主机名
    #[serde(rename = "主机名")]
    pub hostname: String,

    /// IP地址
    #[serde(rename = "IP地址")]
    pub ip_address: String,

    /// 可用Agent (逗号分隔的CLI列表, 如claude,codex,opencode)
    #[serde(rename = "可用Agent")]
    pub available_agents: String,

    /// 状态: 在线/离线/忙碌
    #[serde(rename = "状态")]
    pub status: RuntimeStatus,

    /// 最后心跳时间
    #[serde(rename = "最后心跳")]
    pub last_heartbeat: NaiveDateTime,

    /// 操作系统 (Linux/macOS/Windows)
    #[serde(rename = "操作系统")]
    pub os: String,

    /// Daemon版本号
    #[serde(rename = "版本号")]
    pub version: String,

    /// 关联任务 (反向关联任务主表)
    #[serde(rename = "关联任务")]
    pub linked_tasks: Vec<LinkRecord>,
}

/// 从配置创建运行时信息的 trait
pub trait FromConfig {
    fn from_config(config: &DaemonConfig) -> Self;
}

impl FromConfig for RuntimeInfo {
    fn from_config(config: &DaemonConfig) -> Self {
        Self {
            id: 0,
            runtime_id: config.runtime_id.clone(),
            runtime_name: config.runtime_name.clone(),
            hostname: get_hostname(),
            ip_address: get_local_ip(),
            available_agents: "claude,codex,opencode".to_string(),
            status: RuntimeStatus::Online,
            last_heartbeat: chrono::Local::now().naive_local(),
            os: std::env::consts::OS.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            linked_tasks: Vec::new(),
        }
    }
}

/// 获取本机主机名
fn get_hostname() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string())
}

/// 获取本地IP地址
fn get_local_ip() -> String {
    local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}
