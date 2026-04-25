use tracing::debug;

use crate::client::core::{BaseClient, BaseClientError};
use crate::client::parser::{agent_type_to_string, execution_status_to_string, trigger_mode_to_string};
use crate::models::execution::ExecutionLog;

impl BaseClient {
    pub async fn create_execution_log(
        &self,
        log: &ExecutionLog,
    ) -> Result<String, BaseClientError> {
        let path = format!(
            "/open-apis/bitable/v1/apps/{}/tables/{}/records",
            self.config.base_token, self.execution_log_table_id().await
        );

        let linked_task_ids: Vec<String> = log
            .linked_task
            .iter()
            .map(|link| link.id.clone())
            .collect();

        let body = serde_json::json!({
            "fields": {
                "关联任务": linked_task_ids,
                "执行序号": log.execution_sequence,
                "Agent类型": agent_type_to_string(&log.agent_type),
                "执行状态": execution_status_to_string(&log.execution_status),
                "开始时间": log.start_time.and_utc().timestamp_millis(),
                "结束时间": log.end_time.map(|t| t.and_utc().timestamp_millis()),
                "执行输出": log.execution_output,
                "错误信息": log.error_info,
                "提交记录": log.commit_hash,
                "触发方式": trigger_mode_to_string(&log.trigger_mode),
            }
        });

        let response = self
            .api_request(reqwest::Method::POST, &path, Some(body), None)
            .await?;

        let record_id = response
            .get("data")
            .and_then(|d| d.get("record"))
            .and_then(|r| r.get("record_id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| BaseClientError::Other {
                message: "创建执行记录失败：未返回 record_id".to_string(),
            })?
            .to_string();

        debug!(
            "{}",
            rust_i18n::t!(
                "base_client.created_execution_log",
                id = record_id,
                seq = log.execution_sequence
            )
        );

        Ok(record_id)
    }

    pub async fn update_execution_log(
        &self,
        record_id: &str,
        log: &ExecutionLog,
    ) -> Result<(), BaseClientError> {
        let path = format!(
            "/open-apis/bitable/v1/apps/{}/tables/{}/records/{}",
            self.config.base_token, self.execution_log_table_id().await, record_id
        );

        let body = serde_json::json!({
            "fields": {
                "执行状态": execution_status_to_string(&log.execution_status),
                "结束时间": log.end_time.map(|t| t.and_utc().timestamp_millis()),
                "执行输出": log.execution_output,
                "错误信息": log.error_info,
                "提交记录": log.commit_hash,
            }
        });

        self.api_request(reqwest::Method::PUT, &path, Some(body), None)
            .await?;

        debug!(
            "{}",
            rust_i18n::t!("base_client.updated_execution_log", id = record_id)
        );

        Ok(())
    }
}
