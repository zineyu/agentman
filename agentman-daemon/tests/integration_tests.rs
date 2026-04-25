use serde_json::json;

use agentman_daemon::client::BaseClient;
use agentman_daemon::config::DaemonConfig;
use agentman_daemon::models::{
    execution::{AgentType as ExecutionAgentType, ExecutionLog, ExecutionStatus, LinkRecord, TriggerMode},
    runtime::{FromConfig, RuntimeInfo},
    task::{ExecutorType, Status},
};

fn create_test_config(base_url: &str) -> DaemonConfig {
    DaemonConfig {
        runtime_id: "test-runtime-001".to_string(),
        runtime_name: "Test Runtime".to_string(),
        base_url: base_url.to_string(),
        base_token: "BascnXXXXXXXXXXXXXXXX".to_string(),
        app_id: "cli_xxxxxxxxxxxxxxxx".to_string(),
        app_secret: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx".to_string(),
        poll_interval_secs: 30,
        heartbeat_interval_secs: 60,
        max_concurrent_tasks: 3,
        workspace_dir: "./workspace".to_string(),
        log_level: "info".to_string(),
        language: "en".to_string(),
    }
}

fn mock_token_response() -> serde_json::Value {
    json!({
        "code": 0,
        "msg": "ok",
        "tenant_access_token": "test-token-12345",
        "expire": 7200,
    })
}



#[tokio::test]
async fn test_base_client_get_pending_tasks() {
    let mut server = mockito::Server::new_async().await;
    let config = create_test_config(&server.url());

    let _token = server
        .mock("POST", "/open-apis/auth/v3/tenant_access_token/internal")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_token_response().to_string())
        .create_async()
        .await;

    let task_response = json!({
        "code": 0,
        "msg": "success",
        "data": {
            "items": [
                {
                    "record_id": "rec123",
                    "fields": {
                        "任务标题": "测试任务",
                        "任务描述": "这是一个测试任务",
                        "执行者类型": "agent",
                        "执行者": "test-runtime-001",
                        "任务状态": "待办",
                        "优先级": "P1",
                        "仓库地址": "https://github.com/test/repo.git",
                        "分支名称": "main",
                        "重试次数": 0,
                        "催办次数": 0,
                        "预计工时": 2.5,
                        "审核驳回理由": "",
                    }
                }
            ],
            "total": 1
        }
    });

    let _tasks = server
        .mock("GET", mockito::Matcher::Regex(r"/open-apis/bitable/v1/apps/.*/tables/.*/records.*".to_string()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(task_response.to_string())
        .create_async()
        .await;

    let client = BaseClient::new(&config).unwrap();
    let tasks = client.get_pending_tasks("test-runtime-001").await.unwrap();

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].record_id, "rec123");
    assert_eq!(tasks[0].title, "测试任务");
    assert_eq!(tasks[0].status, Status::Todo);
    assert_eq!(tasks[0].executor_type, ExecutorType::Agent);
}

#[tokio::test]
async fn test_base_client_update_task_status() {
    let mut server = mockito::Server::new_async().await;
    let config = create_test_config(&server.url());

    let _token = server
        .mock("POST", "/open-apis/auth/v3/tenant_access_token/internal")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_token_response().to_string())
        .create_async()
        .await;

    let get_response = json!({
        "code": 0,
        "msg": "success",
        "data": {
            "record": {
                "record_id": "rec123",
                "fields": {
                    "执行日志": ""
                }
            }
        }
    });

    let _get = server
        .mock("GET", mockito::Matcher::Regex(r"/open-apis/bitable/v1/apps/.*/tables/.*/records/rec123".to_string()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(get_response.to_string())
        .create_async()
        .await;

    let _update = server
        .mock("PUT", mockito::Matcher::Regex(r"/open-apis/bitable/v1/apps/.*/tables/.*/records/rec123".to_string()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(json!({"code": 0, "msg": "success"}).to_string())
        .create_async()
        .await;

    let client = BaseClient::new(&config).unwrap();
    let result = client.update_task_status("rec123", Status::InProgress, "任务开始执行").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_base_client_register_runtime() {
    let mut server = mockito::Server::new_async().await;
    let config = create_test_config(&server.url());

    let _token = server
        .mock("POST", "/open-apis/auth/v3/tenant_access_token/internal")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_token_response().to_string())
        .create_async()
        .await;

    let _register = server
        .mock("POST", mockito::Matcher::Regex(r"/open-apis/bitable/v1/apps/.*/tables/.*/records".to_string()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(json!({"code": 0, "msg": "success"}).to_string())
        .create_async()
        .await;

    let client = BaseClient::new(&config).unwrap();
    let runtime = RuntimeInfo::from_config(&config);
    let result = client.register_runtime(&runtime).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_base_client_create_execution_log() {
    let mut server = mockito::Server::new_async().await;
    let config = create_test_config(&server.url());

    let _token = server
        .mock("POST", "/open-apis/auth/v3/tenant_access_token/internal")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_token_response().to_string())
        .create_async()
        .await;

    let create_response = json!({
        "code": 0,
        "msg": "success",
        "data": {
            "record": {
                "record_id": "rec_log_001"
            }
        }
    });

    let _create = server
        .mock("POST", mockito::Matcher::Regex(r"/open-apis/bitable/v1/apps/.*/tables/.*/records".to_string()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(create_response.to_string())
        .create_async()
        .await;

    let client = BaseClient::new(&config).unwrap();
    let log = ExecutionLog {
        id: 0,
        linked_task: vec![LinkRecord { id: "rec123".to_string() }],
        execution_sequence: 1,
        agent_type: ExecutionAgentType::ClaudeCode,
        execution_status: ExecutionStatus::InProgress,
        start_time: chrono::Local::now().naive_local(),
        end_time: None,
        execution_output: String::new(),
        error_info: String::new(),
        commit_hash: String::new(),
        trigger_mode: TriggerMode::Auto,
    };

    let record_id = client.create_execution_log(&log).await.unwrap();
    assert_eq!(record_id, "rec_log_001");
}

#[tokio::test]
async fn test_base_client_retry_on_rate_limit() {
    let mut server = mockito::Server::new_async().await;
    let config = create_test_config(&server.url());

    let _token = server
        .mock("POST", "/open-apis/auth/v3/tenant_access_token/internal")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_token_response().to_string())
        .create_async()
        .await;

    let _rate_limit = server
        .mock("GET", mockito::Matcher::Regex(r"/open-apis/bitable/v1/apps/.*/tables/.*/records.*".to_string()))
        .with_status(429)
        .with_header("content-type", "application/json")
        .with_body(json!({"code": 1254290, "msg": "rate limited"}).to_string())
        .expect_at_least(1)
        .create_async()
        .await;

    let client = BaseClient::new(&config).unwrap();
    let result = client.get_pending_tasks("test-runtime-001").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_base_client_clear_rejection_reason() {
    let mut server = mockito::Server::new_async().await;
    let config = create_test_config(&server.url());

    let _token = server
        .mock("POST", "/open-apis/auth/v3/tenant_access_token/internal")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_token_response().to_string())
        .create_async()
        .await;

    let _update = server
        .mock("PUT", mockito::Matcher::Regex(r"/open-apis/bitable/v1/apps/.*/tables/.*/records/rec123".to_string()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(json!({"code": 0, "msg": "success"}).to_string())
        .create_async()
        .await;

    let client = BaseClient::new(&config).unwrap();
    let result = client.clear_task_rejection_reason("rec123").await;
    assert!(result.is_ok());
}
