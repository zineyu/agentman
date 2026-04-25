use tracing::info;

use crate::client::core::{BaseClient, BaseClientError};
use crate::client::parser::parse_task_from_record;
use crate::models::task::{Status, Task};

impl BaseClient {
    pub async fn get_pending_tasks(
        &self,
        runtime_id: &str,
    ) -> Result<Vec<Task>, BaseClientError> {
        let filter = format!(
            "AND(CurrentValue.[任务状态]=\"待办\",CurrentValue.[执行者]=\"{}\")",
            runtime_id
        );

        let path = format!(
            "/open-apis/bitable/v1/apps/{}/tables/{}/records",
            self.config.base_token, self.task_table_id().await
        );

        let query = vec![
            ("filter", filter),
            ("page_size", "500".to_string()),
            ("automatic_fields", "true".to_string()),
        ];

        let response = self
            .api_request(reqwest::Method::GET, &path, None, Some(query))
            .await?;

        let items = response
            .get("data")
            .and_then(|d| d.get("items"))
            .and_then(|i| i.as_array())
            .cloned()
            .unwrap_or_default();

        let tasks: Vec<Task> = items.into_iter().map(parse_task_from_record).collect();

        info!(
            "{}",
            rust_i18n::t!(
                "base_client.fetched_pending_tasks",
                count = tasks.len(),
                runtime = runtime_id
            )
        );

        Ok(tasks)
    }

    pub async fn update_task_status(
        &self,
        task_id: &str,
        status: Status,
        execution_log: &str,
    ) -> Result<(), BaseClientError> {
        let current_log = self.get_task_field(task_id, "执行日志").await?;

        let new_log = if current_log.is_empty() {
            execution_log.to_string()
        } else {
            format!(
                "{}\n[{}] {}",
                current_log,
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                execution_log
            )
        };

        let status_str = status.to_string();

        let path = format!(
            "/open-apis/bitable/v1/apps/{}/tables/{}/records/{}",
            self.config.base_token, self.task_table_id().await, task_id
        );

        let body = serde_json::json!({
            "fields": {
                "任务状态": status_str,
                "执行日志": new_log,
            }
        });

        self.api_request(reqwest::Method::PUT, &path, Some(body), None)
            .await?;

        info!(
            "{}",
            rust_i18n::t!(
                "base_client.updated_task_status",
                id = task_id,
                status = status_str
            )
        );

        Ok(())
    }

    pub async fn clear_task_rejection_reason(
        &self,
        task_id: &str,
    ) -> Result<(), BaseClientError> {
        let path = format!(
            "/open-apis/bitable/v1/apps/{}/tables/{}/records/{}",
            self.config.base_token, self.task_table_id().await, task_id
        );

        let body = serde_json::json!({
            "fields": {
                "审核驳回理由": "",
            }
        });

        self.api_request(reqwest::Method::PUT, &path, Some(body), None)
            .await?;

        info!(
            "{}",
            rust_i18n::t!("base_client.cleared_rejection_reason", id = task_id)
        );
        Ok(())
    }

    async fn get_task_field(
        &self,
        task_id: &str,
        field_name: &str,
    ) -> Result<String, BaseClientError> {
        let path = format!(
            "/open-apis/bitable/v1/apps/{}/tables/{}/records/{}",
            self.config.base_token, self.task_table_id().await, task_id
        );

        let response = self
            .api_request(reqwest::Method::GET, &path, None, None)
            .await?;

        let value = response
            .get("data")
            .and_then(|d| d.get("record"))
            .and_then(|r| r.get("fields"))
            .and_then(|f| f.get(field_name));

        match value {
            Some(v) if v.is_string() => Ok(v.as_str().unwrap_or_default().to_string()),
            _ => Ok(String::new()),
        }
    }
}
