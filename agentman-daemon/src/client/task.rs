use tracing::{info, debug};

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

        info!("已获取 {} 个待办任务（运行时: {}）", tasks.len(), runtime_id);

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
            format!("已更新任务 {} 状态为 {}", task_id, status_str)
        );

        Ok(())
    }

    /// 批量查询任务状态（用于依赖检查）
    /// 返回 (record_id, status) 的映射
    pub async fn get_tasks_status(
        &self,
        record_ids: &[String],
    ) -> Result<std::collections::HashMap<String, Status>, BaseClientError> {
        if record_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let path = format!(
            "/open-apis/bitable/v1/apps/{}/tables/{}/records",
            self.config.base_token, self.task_table_id().await
        );

        // 构建 filter: OR(CurrentValue.[record_id]="id1", CurrentValue.[record_id]="id2", ...)
        let filter_parts: Vec<String> = record_ids
            .iter()
            .map(|id| format!("CurrentValue.[record_id]=\"{}\"", id))
            .collect();

        let filter = if filter_parts.len() == 1 {
            filter_parts.into_iter().next().unwrap()
        } else {
            let inner = filter_parts.join(",");
            format!("OR({})", inner)
        };

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

        let mut result = std::collections::HashMap::with_capacity(record_ids.len());
        for item in items {
            let record_id = item
                .get("record_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let fields = item.get("fields").cloned().unwrap_or(serde_json::json!({}));
            let status_str = fields.get("任务状态").and_then(|v| v.as_str()).unwrap_or("待办");
            let status = crate::client::parser::parse_task_status(status_str);
            result.insert(record_id, status);
        }

        debug!("已获取 {} 个任务的状态", result.len());

        Ok(result)
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
            format!("已清空任务 {} 的驳回理由", task_id)
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
