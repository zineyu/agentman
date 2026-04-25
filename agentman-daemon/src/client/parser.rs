use serde_json::{json, Value};

use crate::models::task::{Status, Task, LinkRecord};

/// 从API响应解析Task
pub fn parse_task_from_record(record: Value) -> Task {
    let fields = record.get("fields").cloned().unwrap_or(json!({}));
    let record_id = record
        .get("record_id")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    Task {
        record_id,
        id: 0,
        title: get_str_field(&fields, "任务标题"),
        description: get_str_field(&fields, "任务描述"),
        executor_type: parse_executor_type(&get_str_field(&fields, "执行者类型")),
        executor: get_str_field(&fields, "执行者"),
        status: parse_task_status(&get_str_field(&fields, "任务状态")),
        priority: parse_priority(&get_str_field(&fields, "优先级")),
        start_time: parse_datetime_field(&fields, "开始时间"),
        deadline: parse_datetime_field(&fields, "截止时间"),
        completed_at: parse_datetime_field(&fields, "完成时间"),
        last_urge_time: parse_datetime_field(&fields, "最后催办时间"),
        agent_type: parse_agent_type_field(&fields, "Agent类型"),
        work_dir: get_str_field(&fields, "工作目录"),
        reviewer: get_opt_str_field(&fields, "审核人"),
        review_comment: get_str_field(&fields, "审核意见"),
        review_rejection_reason: get_str_field(&fields, "审核驳回理由"),
        retry_count: get_u32_field(&fields, "重试次数"),
        urge_count: get_u32_field(&fields, "催办次数"),
        estimated_hours: get_f64_field(&fields, "预计工时"),
        assigned_runtime: parse_link_field(&fields, "分配的运行时"),
    }
}

pub fn get_str_field(fields: &Value, name: &str) -> String {
    match fields.get(name) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Object(map)) => {
            map.get("link")
                .and_then(|v| v.as_str())
                .or_else(|| map.get("text").and_then(|v| v.as_str()))
                .unwrap_or_default()
                .to_string()
        }
        Some(Value::Array(arr)) if !arr.is_empty() => {
            arr[0].as_str().unwrap_or_default().to_string()
        }
        _ => String::new(),
    }
}

pub fn get_opt_str_field(fields: &Value, name: &str) -> Option<String> {
    fields.get(name).and_then(|v| v.as_str()).map(String::from)
}

pub fn get_u32_field(fields: &Value, name: &str) -> u32 {
    fields
        .get(name)
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32
}

pub fn get_f64_field(fields: &Value, name: &str) -> f64 {
    fields
        .get(name)
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
}

pub fn parse_datetime_field(fields: &Value, name: &str) -> Option<chrono::NaiveDateTime> {
    fields
        .get(name)
        .and_then(|v| v.as_i64())
        .and_then(chrono::DateTime::from_timestamp_millis)
        .map(|dt| dt.naive_utc())
}

pub fn parse_task_status(status_str: &str) -> Status {
    match status_str {
        "待办" => Status::Todo,
        "进行中" => Status::InProgress,
        "待审核" => Status::PendingReview,
        "已完成" => Status::Completed,
        "已取消" => Status::Cancelled,
        _ => Status::Todo,
    }
}

pub fn parse_executor_type(type_str: &str) -> crate::models::task::ExecutorType {
    match type_str.to_lowercase().as_str() {
        "human" | "人工" => crate::models::task::ExecutorType::Human,
        _ => crate::models::task::ExecutorType::Agent,
    }
}

pub fn parse_priority(priority_str: &str) -> crate::models::task::Priority {
    match priority_str {
        "P0" => crate::models::task::Priority::P0,
        "P1" => crate::models::task::Priority::P1,
        "P2" => crate::models::task::Priority::P2,
        "P3" => crate::models::task::Priority::P3,
        _ => crate::models::task::Priority::P2,
    }
}

pub fn parse_agent_type_field(fields: &Value, name: &str) -> Option<crate::models::task::AgentType> {
    let type_str = fields.get(name).and_then(|v| v.as_str())?;
    match type_str.to_lowercase().as_str() {
        "claude-code" | "claude" => Some(crate::models::task::AgentType::ClaudeCode),
        "codex" => Some(crate::models::task::AgentType::Codex),
        "opencode" => Some(crate::models::task::AgentType::Opencode),
        "cursor" => Some(crate::models::task::AgentType::Cursor),
        "其他" | "other" => Some(crate::models::task::AgentType::Other),
        _ => None,
    }
}

pub fn parse_link_field(fields: &Value, name: &str) -> Vec<crate::models::task::LinkRecord> {
    let arr = match fields.get(name).and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return Vec::new(),
    };

    arr.iter()
        .filter_map(|item| {
            item.get("id")
                .and_then(|v| v.as_str())
                .map(|id| crate::models::task::LinkRecord {
                    id: id.to_string(),
                })
                .or_else(|| {
                    item.as_str().map(|id| LinkRecord {
                        id: id.to_string(),
                    })
                })
        })
        .collect()
}

pub fn parse_runtime_status(status_str: &str) -> crate::models::runtime::RuntimeStatus {
    match status_str {
        "在线" => crate::models::runtime::RuntimeStatus::Online,
        "离线" => crate::models::runtime::RuntimeStatus::Offline,
        "忙碌" => crate::models::runtime::RuntimeStatus::Busy,
        _ => crate::models::runtime::RuntimeStatus::Offline,
    }
}

pub fn runtime_status_to_string(status: &crate::models::runtime::RuntimeStatus) -> String {
    match status {
        crate::models::runtime::RuntimeStatus::Online => "在线".to_string(),
        crate::models::runtime::RuntimeStatus::Offline => "离线".to_string(),
        crate::models::runtime::RuntimeStatus::Busy => "忙碌".to_string(),
    }
}

pub fn agent_type_to_string(agent_type: &crate::models::execution::AgentType) -> String {
    match agent_type {
        crate::models::execution::AgentType::ClaudeCode => "claude-code".to_string(),
        crate::models::execution::AgentType::Codex => "codex".to_string(),
        crate::models::execution::AgentType::Opencode => "opencode".to_string(),
        crate::models::execution::AgentType::Cursor => "cursor".to_string(),
        crate::models::execution::AgentType::Other => "其他".to_string(),
    }
}

pub fn execution_status_to_string(status: &crate::models::execution::ExecutionStatus) -> String {
    match status {
        crate::models::execution::ExecutionStatus::Success => "成功".to_string(),
        crate::models::execution::ExecutionStatus::Failed => "失败".to_string(),
        crate::models::execution::ExecutionStatus::InProgress => "进行中".to_string(),
        crate::models::execution::ExecutionStatus::Timeout => "超时".to_string(),
    }
}

pub fn trigger_mode_to_string(mode: &crate::models::execution::TriggerMode) -> String {
    match mode {
        crate::models::execution::TriggerMode::Manual => "手动".to_string(),
        crate::models::execution::TriggerMode::Auto => "自动".to_string(),
        crate::models::execution::TriggerMode::Workflow => "工作流".to_string(),
    }
}
