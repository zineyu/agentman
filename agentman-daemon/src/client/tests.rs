#[cfg(test)]
mod parser_tests {
    use serde_json::json;

    use crate::client::parser::*;
    use crate::models::execution::{AgentType as ExecutionAgentType, ExecutionStatus, TriggerMode};
    use crate::models::runtime::RuntimeStatus;
    use crate::models::task::{AgentType, ExecutorType, Priority, Status};

    #[test]
    fn test_parse_task_status() {
        assert_eq!(parse_task_status("待办"), Status::Todo);
        assert_eq!(parse_task_status("进行中"), Status::InProgress);
        assert_eq!(parse_task_status("待审核"), Status::PendingReview);
        assert_eq!(parse_task_status("已完成"), Status::Completed);
        assert_eq!(parse_task_status("已取消"), Status::Cancelled);
        assert_eq!(parse_task_status("未知状态"), Status::Todo);
        assert_eq!(parse_task_status(""), Status::Todo);
    }

    #[test]
    fn test_parse_executor_type() {
        assert_eq!(parse_executor_type("agent"), ExecutorType::Agent);
        assert_eq!(parse_executor_type("Agent"), ExecutorType::Agent);
        assert_eq!(parse_executor_type("人工"), ExecutorType::Human);
        assert_eq!(parse_executor_type("human"), ExecutorType::Human);
        assert_eq!(parse_executor_type("HUMAN"), ExecutorType::Human);
        assert_eq!(parse_executor_type(""), ExecutorType::Agent);
    }

    #[test]
    fn test_parse_priority() {
        assert_eq!(parse_priority("P0"), Priority::P0);
        assert_eq!(parse_priority("P1"), Priority::P1);
        assert_eq!(parse_priority("P2"), Priority::P2);
        assert_eq!(parse_priority("P3"), Priority::P3);
        assert_eq!(parse_priority("P4"), Priority::P2);
        assert_eq!(parse_priority(""), Priority::P2);
    }

    #[test]
    fn test_parse_runtime_status() {
        assert_eq!(parse_runtime_status("在线"), RuntimeStatus::Online);
        assert_eq!(parse_runtime_status("离线"), RuntimeStatus::Offline);
        assert_eq!(parse_runtime_status("忙碌"), RuntimeStatus::Busy);
        assert_eq!(parse_runtime_status("未知"), RuntimeStatus::Offline);
        assert_eq!(parse_runtime_status(""), RuntimeStatus::Offline);
    }

    #[test]
    fn test_agent_type_to_string() {
        assert_eq!(
            agent_type_to_string(&ExecutionAgentType::ClaudeCode),
            "claude-code"
        );
        assert_eq!(agent_type_to_string(&ExecutionAgentType::Codex), "codex");
        assert_eq!(
            agent_type_to_string(&ExecutionAgentType::Opencode),
            "opencode"
        );
        assert_eq!(
            agent_type_to_string(&ExecutionAgentType::Cursor),
            "cursor"
        );
        assert_eq!(
            agent_type_to_string(&ExecutionAgentType::Other),
            "其他"
        );
    }

    #[test]
    fn test_execution_status_to_string() {
        assert_eq!(
            execution_status_to_string(&ExecutionStatus::Success),
            "成功"
        );
        assert_eq!(
            execution_status_to_string(&ExecutionStatus::Failed),
            "失败"
        );
        assert_eq!(
            execution_status_to_string(&ExecutionStatus::InProgress),
            "进行中"
        );
        assert_eq!(
            execution_status_to_string(&ExecutionStatus::Timeout),
            "超时"
        );
    }

    #[test]
    fn test_trigger_mode_to_string() {
        assert_eq!(trigger_mode_to_string(&TriggerMode::Manual), "手动");
        assert_eq!(trigger_mode_to_string(&TriggerMode::Auto), "自动");
        assert_eq!(trigger_mode_to_string(&TriggerMode::Workflow), "工作流");
    }

    #[test]
    fn test_get_str_field() {
        let fields = json!({
            "字符串字段": "test_value",
            "空字段": null,
            "数字字段": 123,
            "数组字段": ["item1", "item2"],
            "对象字段": {
                "link": "linked_value",
                "text": "text_value"
            }
        });

        assert_eq!(get_str_field(&fields, "字符串字段"), "test_value");
        assert_eq!(get_str_field(&fields, "空字段"), "");
        assert_eq!(get_str_field(&fields, "不存在的字段"), "");
        assert_eq!(get_str_field(&fields, "数字字段"), "");
        assert_eq!(get_str_field(&fields, "数组字段"), "item1");
        assert_eq!(get_str_field(&fields, "对象字段"), "linked_value");
    }

    #[test]
    fn test_get_str_field_with_text_fallback() {
        let fields = json!({
            "仅有text": {
                "text": "text_only_value"
            }
        });

        assert_eq!(get_str_field(&fields, "仅有text"), "text_only_value");
    }

    #[test]
    fn test_get_opt_str_field() {
        let fields = json!({
            "存在的字段": "value",
            "null字段": null,
            "数字字段": 123
        });

        assert_eq!(
            get_opt_str_field(&fields, "存在的字段"),
            Some("value".to_string())
        );
        assert_eq!(get_opt_str_field(&fields, "null字段"), None);
        assert_eq!(get_opt_str_field(&fields, "数字字段"), None);
        assert_eq!(get_opt_str_field(&fields, "不存在的字段"), None);
    }

    #[test]
    fn test_get_u32_field() {
        let fields = json!({
            "数字": 42,
            "大数字": 999999,
            "null字段": null,
            "字符串数字": "123"
        });

        assert_eq!(get_u32_field(&fields, "数字"), 42);
        assert_eq!(get_u32_field(&fields, "大数字"), 999999);
        assert_eq!(get_u32_field(&fields, "null字段"), 0);
        assert_eq!(get_u32_field(&fields, "字符串数字"), 0);
        assert_eq!(get_u32_field(&fields, "不存在的字段"), 0);
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn test_get_f64_field() {
        let fields = json!({
            "浮点数": 3.14,
            "整数": 42,
            "null字段": null,
            "字符串数字": "1.5"
        });

        assert_eq!(get_f64_field(&fields, "浮点数"), 3.14);
        assert_eq!(get_f64_field(&fields, "整数"), 42.0);
        assert_eq!(get_f64_field(&fields, "null字段"), 0.0);
        assert_eq!(get_f64_field(&fields, "字符串数字"), 0.0);
        assert_eq!(get_f64_field(&fields, "不存在的字段"), 0.0);
    }

    #[test]
    fn test_parse_link_field() {
        let fields = json!({
            "有效链接": [
                {"id": "rec001"},
                {"id": "rec002"}
            ],
            "字符串数组": ["rec003", "rec004"],
            "空数组": [],
            "null字段": null,
            "混合数组": [
                {"id": "rec005"},
                "rec006",
                {"no_id": "value"}
            ]
        });

        let result = parse_link_field(&fields, "有效链接");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "rec001");
        assert_eq!(result[1].id, "rec002");

        let result = parse_link_field(&fields, "字符串数组");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "rec003");
        assert_eq!(result[1].id, "rec004");

        let result = parse_link_field(&fields, "空数组");
        assert!(result.is_empty());

        let result = parse_link_field(&fields, "null字段");
        assert!(result.is_empty());

        let result = parse_link_field(&fields, "混合数组");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "rec005");
        assert_eq!(result[1].id, "rec006");
    }

    #[test]
    fn test_parse_agent_type_field() {
        let fields = json!({
            "claude": "claude-code",
            "codex": "codex",
            "opencode": "opencode",
            "cursor": "cursor",
            "other": "其他",
            "other_en": "other",
            "unknown": "unknown_tool"
        });

        assert_eq!(
            parse_agent_type_field(&fields, "claude"),
            Some(AgentType::ClaudeCode)
        );
        assert_eq!(
            parse_agent_type_field(&fields, "codex"),
            Some(AgentType::Codex)
        );
        assert_eq!(
            parse_agent_type_field(&fields, "opencode"),
            Some(AgentType::Opencode)
        );
        assert_eq!(
            parse_agent_type_field(&fields, "cursor"),
            Some(AgentType::Cursor)
        );
        assert_eq!(
            parse_agent_type_field(&fields, "other"),
            Some(AgentType::Other)
        );
        assert_eq!(
            parse_agent_type_field(&fields, "other_en"),
            Some(AgentType::Other)
        );
        assert_eq!(parse_agent_type_field(&fields, "unknown"), None);
        assert_eq!(parse_agent_type_field(&fields, "不存在的"), None);
    }

    #[test]
    fn test_parse_task_from_record() {
        let record = json!({
            "record_id": "rec123",
            "fields": {
                "任务标题": "测试任务",
                "任务描述": "这是一个测试任务",
                "执行者类型": "agent",
                "执行者": "daemon-001",
                "任务状态": "待办",
                "优先级": "P1",
                "仓库地址": "https://github.com/test/repo.git",
                "分支名称": "main",
                "重试次数": 2,
                "催办次数": 1,
                "预计工时": 3.5,
                "审核驳回理由": "需要修改",
                "分配的运行时": [{"id": "runtime001"}]
            }
        });

        let task = parse_task_from_record(record);

        assert_eq!(task.record_id, "rec123");
        assert_eq!(task.title, "测试任务");
        assert_eq!(task.description, "这是一个测试任务");
        assert_eq!(task.executor_type, ExecutorType::Agent);
        assert_eq!(task.executor, "daemon-001");
        assert_eq!(task.status, Status::Todo);
        assert_eq!(task.priority, Priority::P1);
        assert_eq!(task.retry_count, 2);
        assert_eq!(task.urge_count, 1);
        assert_eq!(task.estimated_hours, 3.5);
        assert_eq!(task.review_rejection_reason, "需要修改");
        assert_eq!(task.assigned_runtime.len(), 1);
        assert_eq!(task.assigned_runtime[0].id, "runtime001");
    }

    #[test]
    fn test_parse_task_from_record_with_empty_fields() {
        let record = json!({
            "record_id": "rec456"
        });

        let task = parse_task_from_record(record);

        assert_eq!(task.record_id, "rec456");
        assert!(task.title.is_empty());
        assert!(task.description.is_empty());
        assert_eq!(task.status, Status::Todo);
        assert_eq!(task.priority, Priority::P2);
        assert_eq!(task.retry_count, 0);
        assert!(task.assigned_runtime.is_empty());
    }

    #[test]
    fn test_parse_task_from_record_without_record_id() {
        let record = json!({
            "fields": {
                "任务标题": "无ID任务"
            }
        });

        let task = parse_task_from_record(record);

        assert!(task.record_id.is_empty());
        assert_eq!(task.title, "无ID任务");
    }

    #[test]
    fn test_parse_datetime_field() {
        use chrono::NaiveDate;

        // 2024-01-01 10:00:00 UTC in milliseconds
        let timestamp_ms = 1704103200000i64;

        let fields = json!({
            "时间戳": timestamp_ms,
            "null字段": null,
            "字符串": "not a timestamp"
        });

        let result = parse_datetime_field(&fields, "时间戳");
        assert!(result.is_some());
        let datetime = result.unwrap();
        assert_eq!(datetime.date(), NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());

        assert!(parse_datetime_field(&fields, "null字段").is_none());
        assert!(parse_datetime_field(&fields, "字符串").is_none());
        assert!(parse_datetime_field(&fields, "不存在的字段").is_none());
    }
}
