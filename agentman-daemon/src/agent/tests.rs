use super::*;
use crate::agent::factory::AgentFactory;
use crate::models::task::AgentType;

#[test]
fn test_execution_result_success() {
    let result = ExecutionResult::success("Task output".to_string());
    assert!(result.success);
    assert_eq!(result.output, "Task output");
    assert!(result.error_info.is_none());
    assert!(result.commit_hash.is_none());
}

#[test]
fn test_execution_result_failed() {
    let result = ExecutionResult::failed(
        "Partial output".to_string(),
        "Something went wrong".to_string(),
    );
    assert!(!result.success);
    assert_eq!(result.output, "Partial output");
    assert_eq!(result.error_info, Some("Something went wrong".to_string()));
    assert!(result.commit_hash.is_none());
}

#[test]
fn test_agent_factory_claude_code() {
    let result = AgentFactory::create(AgentType::ClaudeCode);
    match result {
        Ok(adapter) => {
            assert!(adapter.name().contains("ClaudeCode"));
        }
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("ClaudeCode") || msg.contains("claude") || msg.contains("cli_adapter.no_cli_found"),
                "Unexpected error: {}",
                msg
            );
        }
    }
}

#[test]
fn test_agent_factory_cursor() {
    let result = AgentFactory::create(AgentType::Cursor);
    match result {
        Ok(adapter) => {
            assert!(adapter.name().contains("Cursor"));
        }
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("Cursor") || msg.contains("cursor") || msg.contains("cli_adapter.no_cli_found"),
                "Unexpected error: {}",
                msg
            );
        }
    }
}
