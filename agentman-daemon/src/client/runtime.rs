use tracing::{info, debug, warn};

use crate::client::core::{BaseClient, BaseClientError};
use crate::client::parser::{get_str_field, parse_datetime_field, parse_runtime_status, runtime_status_to_string};
use crate::models::runtime::RuntimeInfo;

impl BaseClient {
    pub async fn find_runtime_by_hostname(
        &self,
        hostname: &str,
    ) -> Result<Option<RuntimeInfo>, BaseClientError> {
        let filter = format!("CurrentValue.[主机名]=\"{}\"", hostname);

        let path = format!(
            "/open-apis/bitable/v1/apps/{}/tables/{}/records",
            self.config.base_token, self.runtime_table_id().await
        );

        let query = vec![("filter", filter), ("page_size", "1".to_string())];

        let response = self
            .api_request(reqwest::Method::GET, &path, None, Some(query))
            .await?;

        let items = response
            .get("data")
            .and_then(|d| d.get("items"))
            .and_then(|i| i.as_array())
            .cloned()
            .unwrap_or_default();

        if items.is_empty() {
            return Ok(None);
        }

        let record = &items[0];

        if let Some(record_id) = record.get("record_id").and_then(|v| v.as_str()) {
            let mut cached_id = self.runtime_record_id.write().await;
            *cached_id = Some(record_id.to_string());
        }

        let fields = record.get("fields").cloned().unwrap_or_default();

        let runtime_info = RuntimeInfo {
            id: 0,
            runtime_id: get_str_field(&fields, "运行时ID"),
            runtime_name: get_str_field(&fields, "运行时名称"),
            hostname: get_str_field(&fields, "主机名"),
            ip_address: get_str_field(&fields, "IP地址"),
            available_agents: get_str_field(&fields, "可用Agent"),
            status: parse_runtime_status(&get_str_field(&fields, "状态")),
            last_heartbeat: parse_datetime_field(&fields, "最后心跳")
                .unwrap_or_else(|| chrono::Local::now().naive_local()),
            os: get_str_field(&fields, "操作系统"),
            version: get_str_field(&fields, "版本号"),
            linked_tasks: Vec::new(),
        };

        info!(
            "{}",
            rust_i18n::t!(
                "base_client.found_existing_runtime",
                id = runtime_info.runtime_id,
                hostname = hostname
            )
        );

        Ok(Some(runtime_info))
    }

    pub async fn register_runtime(
        &self,
        runtime_info: &RuntimeInfo,
    ) -> Result<(), BaseClientError> {
        let path = format!(
            "/open-apis/bitable/v1/apps/{}/tables/{}/records",
            self.config.base_token, self.runtime_table_id().await
        );

        let body = serde_json::json!({
            "fields": {
                "运行时ID": runtime_info.runtime_id,
                "主机名": runtime_info.hostname,
                "IP地址": runtime_info.ip_address,
                "状态": runtime_status_to_string(&runtime_info.status),
                "最后心跳": chrono::Utc::now().timestamp_millis(),
                "操作系统": runtime_info.os,
                "版本号": runtime_info.version,
            }
        });

        let response = self
            .api_request(reqwest::Method::POST, &path, Some(body), None)
            .await?;

        if let Some(record_id) = response
            .get("data")
            .and_then(|d| d.get("record"))
            .and_then(|r| r.get("record_id"))
            .and_then(|v| v.as_str())
        {
            let mut cached_id = self.runtime_record_id.write().await;
            *cached_id = Some(record_id.to_string());
        }

        info!(
            "{}",
            rust_i18n::t!(
                "base_client.registered_runtime",
                id = runtime_info.runtime_id
            )
        );

        Ok(())
    }

    pub async fn update_heartbeat(
        &self,
        runtime_info: &RuntimeInfo,
    ) -> Result<(), BaseClientError> {
        let record_id = {
            let cached_id = self.runtime_record_id.read().await;
            cached_id.clone()
        };

        let record_id = match record_id {
            Some(id) => id,
            None => {
                let filter = format!("CurrentValue.[运行时ID]=\"{}\"", runtime_info.runtime_id);

                let path = format!(
                    "/open-apis/bitable/v1/apps/{}/tables/{}/records",
                    self.config.base_token, self.runtime_table_id().await
                );

                let query = vec![("filter", filter), ("page_size", "1".to_string())];

                let response = self
                    .api_request(reqwest::Method::GET, &path, None, Some(query))
                    .await?;

                let items = response
                    .get("data")
                    .and_then(|d| d.get("items"))
                    .and_then(|i| i.as_array())
                    .cloned()
                    .unwrap_or_default();

                if items.is_empty() {
                    warn!(
                        "{}",
                        rust_i18n::t!(
                            "base_client.runtime_not_found_registering",
                            id = runtime_info.runtime_id
                        )
                    );
                    return self.register_runtime(runtime_info).await;
                }

                let id = items[0]
                    .get("record_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let mut cached_id = self.runtime_record_id.write().await;
                *cached_id = Some(id.clone());

                id
            }
        };

        let update_path = format!(
            "/open-apis/bitable/v1/apps/{}/tables/{}/records/{}",
            self.config.base_token, self.runtime_table_id().await, record_id
        );

        let body = serde_json::json!({
            "fields": {
                "最后心跳": chrono::Utc::now().timestamp_millis(),
                "状态": runtime_status_to_string(&runtime_info.status),
            }
        });

        self.api_request(reqwest::Method::PUT, &update_path, Some(body), None)
            .await?;

        debug!(
            "{}",
            rust_i18n::t!(
                "base_client.updated_heartbeat",
                id = runtime_info.runtime_id
            )
        );

        Ok(())
    }
}
