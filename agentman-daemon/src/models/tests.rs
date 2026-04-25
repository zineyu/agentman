use super::*;
use crate::config::DaemonConfig;
use crate::models::execution::{AgentType as ExecutionAgentType, LinkRecord as ExecutionLinkRecord};
use chrono::NaiveDate;

#[test]
fn test_status_serialization() {
    let status = Status::Todo;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"待办\"");

    let status = Status::InProgress;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"进行中\"");

    let status = Status::PendingReview;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"待审核\"");

    let status = Status::Completed;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"已完成\"");

    let status = Status::Cancelled;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"已取消\"");
}

#[test]
fn test_executor_type_serialization() {
    let exec = ExecutorType::Agent;
    let json = serde_json::to_string(&exec).unwrap();
    assert_eq!(json, "\"agent\"");

    let exec = ExecutorType::Human;
    let json = serde_json::to_string(&exec).unwrap();
    assert_eq!(json, "\"human\"");
}

#[test]
fn test_task_deserialization() {
    let json = r#"{
        "record_id": "rec123",
        "id": 1,
        "任务标题": "Test Task",
        "任务描述": "This is a test task",
        "执行者类型": "agent",
        "执行者": "daemon-001",
        "任务状态": "待办",
        "优先级": "P0",
        "开始时间": null,
        "截止时间": null,
        "完成时间": null,
        "最后催办时间": null,
        "Agent类型": "claude-code",
        "工作目录": "./workspace/test",
        "仓库地址": "https://github.com/example/repo.git",
        "分支名称": "main",
        "审核人": null,
        "审核意见": "",
        "审核驳回理由": "",
        "重试次数": 0,
        "催办次数": 0,
        "预计工时": 2.5,
        "分配的运行时": [{"id": "runtime001"}]
    }"#;

    let task: Task = serde_json::from_str(json).expect("Failed to deserialize task");
    assert_eq!(task.record_id, "rec123");
    assert_eq!(task.id, 1);
    assert_eq!(task.title, "Test Task");
    assert_eq!(task.description, "This is a test task");
    assert_eq!(task.executor_type, ExecutorType::Agent);
    assert_eq!(task.executor, "daemon-001");
    assert_eq!(task.status, Status::Todo);
    assert_eq!(task.priority, Priority::P0);
    assert_eq!(task.agent_type, Some(AgentType::ClaudeCode));
    assert_eq!(task.work_dir, "./workspace/test");
    assert_eq!(task.repo_url, "https://github.com/example/repo.git");
    assert_eq!(task.branch, "main");
    assert_eq!(task.retry_count, 0);
    assert_eq!(task.urge_count, 0);
    assert_eq!(task.estimated_hours, 2.5);
    assert_eq!(task.assigned_runtime.len(), 1);
    assert_eq!(task.assigned_runtime[0].id, "runtime001");
}

#[test]
fn test_agent_type_from_str() {
    let agent: AgentType = serde_json::from_str("\"claude-code\"").unwrap();
    assert_eq!(agent, AgentType::ClaudeCode);

    let agent: AgentType = serde_json::from_str("\"codex\"").unwrap();
    assert_eq!(agent, AgentType::Codex);

    let agent: AgentType = serde_json::from_str("\"opencode\"").unwrap();
    assert_eq!(agent, AgentType::Opencode);

    let agent: AgentType = serde_json::from_str("\"cursor\"").unwrap();
    assert_eq!(agent, AgentType::Cursor);

    let agent: AgentType = serde_json::from_str("\"其他\"").unwrap();
    assert_eq!(agent, AgentType::Other);
}

// ============== execution.rs tests ==============

#[test]
fn test_execution_status_serialization() {
    let status = ExecutionStatus::Success;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"成功\"");

    let status = ExecutionStatus::Failed;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"失败\"");

    let status = ExecutionStatus::InProgress;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"进行中\"");

    let status = ExecutionStatus::Timeout;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"超时\"");
}

#[test]
fn test_trigger_mode_serialization() {
    let mode = TriggerMode::Manual;
    let json = serde_json::to_string(&mode).unwrap();
    assert_eq!(json, "\"手动\"");

    let mode = TriggerMode::Auto;
    let json = serde_json::to_string(&mode).unwrap();
    assert_eq!(json, "\"自动\"");

    let mode = TriggerMode::Workflow;
    let json = serde_json::to_string(&mode).unwrap();
    assert_eq!(json, "\"工作流\"");
}

#[test]
fn test_execution_log_serialization() {
    let log = ExecutionLog {
        id: 1,
        linked_task: vec![ExecutionLinkRecord { id: "task001".to_string() }],
        execution_sequence: 1,
        agent_type: ExecutionAgentType::ClaudeCode,
        execution_status: ExecutionStatus::Success,
        start_time: NaiveDate::from_ymd_opt(2024, 1, 1)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap(),
        end_time: Some(
            NaiveDate::from_ymd_opt(2024, 1, 1)
                .unwrap()
                .and_hms_opt(10, 30, 0)
                .unwrap(),
        ),
        execution_output: "Task completed successfully".to_string(),
        error_info: "".to_string(),
        commit_hash: "abc123".to_string(),
        trigger_mode: TriggerMode::Auto,
    };

    let json = serde_json::to_string(&log).unwrap();
    assert!(json.contains("\"成功\""));
    assert!(json.contains("\"自动\""));
    assert!(json.contains("\"claude-code\""));
    assert!(json.contains("Task completed successfully"));
    assert!(json.contains("abc123"));

    let deserialized: ExecutionLog = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, 1);
    assert_eq!(deserialized.execution_sequence, 1);
    assert_eq!(deserialized.agent_type, ExecutionAgentType::ClaudeCode);
    assert_eq!(deserialized.execution_status, ExecutionStatus::Success);
    assert_eq!(deserialized.execution_output, "Task completed successfully");
    assert_eq!(deserialized.commit_hash, "abc123");
    assert_eq!(deserialized.trigger_mode, TriggerMode::Auto);
}

#[test]
fn test_runtime_status_serialization() {
    let status = RuntimeStatus::Online;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"在线\"");

    let status = RuntimeStatus::Offline;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"离线\"");

    let status = RuntimeStatus::Busy;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"忙碌\"");
}

#[test]
fn test_runtime_info_from_config() {
    let config = DaemonConfig {
        runtime_id: "test-runtime-001".to_string(),
        runtime_name: "Test Runtime".to_string(),
        ..Default::default()
    };

    let runtime = RuntimeInfo::from_config(&config);
    assert_eq!(runtime.runtime_id, "test-runtime-001");
    assert_eq!(runtime.runtime_name, "Test Runtime");
    assert_eq!(runtime.status, RuntimeStatus::Online);
    assert_eq!(runtime.id, 0);
    assert_eq!(runtime.available_agents, "claude,codex,opencode");
    assert_eq!(runtime.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(runtime.os, std::env::consts::OS);
    assert!(runtime.linked_tasks.is_empty());
}
