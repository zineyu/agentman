use serde::{Deserialize, Serialize};
use std::fmt;

/// 依赖关系类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DependencyType {
    /// 阻塞型：前置任务必须完成后才能执行
    #[default]
    #[serde(rename = "阻塞")]
    Blocking,
    /// 相关型：前置任务完成与否影响但不阻塞执行
    #[serde(rename = "相关")]
    Related,
    /// 可选型：前置任务仅作为参考，不阻塞执行
    #[serde(rename = "可选")]
    Optional,
}

impl DependencyType {
    /// 是否阻塞执行
    pub fn is_blocking(&self) -> bool {
        matches!(self, DependencyType::Blocking)
    }
}

impl fmt::Display for DependencyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DependencyType::Blocking => write!(f, "阻塞"),
            DependencyType::Related => write!(f, "相关"),
            DependencyType::Optional => write!(f, "可选"),
        }
    }
}



/// 任务依赖项（从任务主表的"前置任务"关联字段解析）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDependency {
    /// 前置任务的 record_id
    pub task_id: String,
    /// 依赖类型
    #[serde(default)]
    pub dep_type: DependencyType,
}

/// 依赖检查结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyCheckResult {
    /// 所有阻塞型依赖均已完成，可以执行
    Ready,
    /// 存在未完成的阻塞型依赖，不可执行
    Blocked { unmet: Vec<String> },
    /// 仅存在相关/可选型依赖未完成，可以执行但需记录
    ReadyWithWarnings { warnings: Vec<String> },
}

impl DependencyCheckResult {
    /// 是否可以执行
    pub fn can_execute(&self) -> bool {
        !matches!(self, DependencyCheckResult::Blocked { .. })
    }

    /// 是否需要记录警告
    pub fn has_warnings(&self) -> bool {
        matches!(self, DependencyCheckResult::ReadyWithWarnings { .. })
    }
}
