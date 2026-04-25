use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::config::DaemonConfig;
use crate::models::{
    execution::{AgentType as ExecutionAgentType, ExecutionLog, ExecutionStatus, TriggerMode},
    runtime::{RuntimeInfo, RuntimeStatus},
    task::{AgentType, ExecutorType, LinkRecord as TaskLinkRecord, Priority, Status, Task},
};

/// BaseClient 错误类型
#[derive(Debug, Error)]
pub enum BaseClientError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("API error {code}: {msg}")]
    ApiError { code: i32, msg: String },

    #[error("Token refresh failed: {0}")]
    TokenRefreshError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Max retries exceeded")]
    MaxRetriesExceeded,

    #[error("Record not found")]
    RecordNotFound,

    #[error("Base not found: no accessible Base named 'Agentman任务管理'")]
    BaseNotFound,
}

/// 缓存的token信息
#[derive(Clone)]
struct TokenInfo {
    token: String,
    expires_at: Instant,
}

/// 表ID缓存
#[derive(Clone, Default)]
struct TableIds {
    base_token: String,
    task_table_id: String,
    runtime_table_id: String,
    execution_log_table_id: String,
}

/// Lark Base API 客户端
pub struct BaseClient {
    http_client: Client,
    config: DaemonConfig,
    token_cache: Arc<RwLock<Option<TokenInfo>>>,
    table_ids: Arc<RwLock<TableIds>>,
}

impl BaseClient {
    /// 创建新的 BaseClient
    pub fn new(config: &DaemonConfig) -> Result<Self, BaseClientError> {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_idle_timeout(Duration::from_secs(60))
            .build()?;

        Ok(Self {
            http_client,
            config: config.clone(),
            token_cache: Arc::new(RwLock::new(None)),
            table_ids: Arc::new(RwLock::new(TableIds::default())),
        })
    }

    /// 获取 tenant_access_token，带缓存和自动刷新
    pub(crate) async fn get_access_token(&self) -> Result<String, BaseClientError> {
        // 快速路径：检查缓存token是否有效
        {
            let cache = self.token_cache.read().await;
            if let Some(token_info) = cache.as_ref() {
                // 如果token在5分钟内过期，则刷新
                if token_info.expires_at > Instant::now() + Duration::from_secs(300) {
                    return Ok(token_info.token.clone());
                }
            }
        }

        // 慢速路径：获取写锁并刷新
        let mut cache = self.token_cache.write().await;

        // 双重检查
        if let Some(token_info) = cache.as_ref() {
            if token_info.expires_at > Instant::now() + Duration::from_secs(300) {
                return Ok(token_info.token.clone());
            }
        }

        let url = format!(
            "{}/open-apis/auth/v3/tenant_access_token/internal",
            self.config.base_url
        );

        let body = json!({
            "app_id": self.config.app_id,
            "app_secret": self.config.app_secret,
        });

        let response = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json; charset=utf-8")
            .json(&body)
            .send()
            .await?;

        let response_body: Value = response.json().await?;
        let code = response_body
            .get("code")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1) as i32;

        if code != 0 {
            let msg = response_body
                .get("msg")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error")
                .to_string();
            return Err(BaseClientError::TokenRefreshError(format!(
                "API error {}: {}",
                code, msg
            )));
        }

        let token = response_body
            .get("tenant_access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                BaseClientError::TokenRefreshError(
                    "Missing tenant_access_token in response".to_string(),
                )
            })?
            .to_string();

        let expire_secs = response_body
            .get("expire")
            .and_then(|v| v.as_i64())
            .unwrap_or(7200);

        let expires_at = Instant::now() + Duration::from_secs(expire_secs.max(0) as u64);

        *cache = Some(TokenInfo {
            token: token.clone(),
            expires_at,
        });

        debug!("Access token refreshed, expires in {} seconds", expire_secs);

        Ok(token)
    }

    /// 清除token缓存
    async fn clear_token_cache(&self) {
        let mut cache = self.token_cache.write().await;
        *cache = None;
    }

    async fn discover_base(&self) -> Result<String, BaseClientError> {
        let path = "/open-apis/bitable/v1/apps";

        let response = self
            .api_request(reqwest::Method::GET, path, None, None)
            .await?;

        let apps = response
            .get("data")
            .and_then(|d| d.get("apps"))
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();

        for app in apps {
            if let (Some(name), Some(token)) = (
                app.get("name").and_then(|v| v.as_str()),
                app.get("app_token").and_then(|v| v.as_str()),
            ) {
                if name == "Agentman任务管理" {
                    info!("Discovered base token for '{}'", name);
                    return Ok(token.to_string());
                }
            }
        }

        Err(BaseClientError::BaseNotFound)
    }

    /// 初始化表ID，通过查询Base表列表获取
    pub async fn init_table_ids(&self) -> Result<(), BaseClientError> {
        let base_token = self.discover_base().await?;

        let path = format!("/open-apis/bitable/v1/apps/{}/tables", base_token);

        let response = self
            .api_request(reqwest::Method::GET, &path, None, None)
            .await?;

        let items = response
            .get("data")
            .and_then(|d| d.get("items"))
            .and_then(|i| i.as_array())
            .cloned()
            .unwrap_or_default();

        let mut table_ids = self.table_ids.write().await;

        for item in items {
            if let (Some(name), Some(id)) = (
                item.get("name").and_then(|v| v.as_str()),
                item.get("table_id").and_then(|v| v.as_str()),
            ) {
                match name {
                    "任务主表" => table_ids.task_table_id = id.to_string(),
                    "运行时表" => table_ids.runtime_table_id = id.to_string(),
                    "执行记录表" => table_ids.execution_log_table_id = id.to_string(),
                    _ => {}
                }
            }
        }

        if table_ids.task_table_id.is_empty() {
            return Err(BaseClientError::ApiError {
                code: -1,
                msg: "任务主表 not found in Base".to_string(),
            });
        }

        table_ids.base_token = base_token;

        info!(
            "Table IDs initialized: task={}, runtime={}, log={}",
            table_ids.task_table_id,
            table_ids.runtime_table_id,
            table_ids.execution_log_table_id
        );

        Ok(())
    }

    async fn base_token(&self) -> String {
        self.table_ids.read().await.base_token.clone()
    }

    /// 获取任务表ID
    async fn task_table_id(&self) -> String {
        self.table_ids.read().await.task_table_id.clone()
    }

    /// 获取运行时表ID
    async fn runtime_table_id(&self) -> String {
        self.table_ids.read().await.runtime_table_id.clone()
    }

    /// 获取执行记录表ID
    async fn execution_log_table_id(&self) -> String {
        self.table_ids.read().await.execution_log_table_id.clone()
    }

    /// 发送API请求，带重试逻辑
    async fn api_request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
        query: Option<Vec<(&str, String)>>,
    ) -> Result<Value, BaseClientError> {
        const MAX_RETRIES: u32 = 3;

        for attempt in 0..MAX_RETRIES {
            let token = self.get_access_token().await?;

            let url = format!("{}{}", self.config.base_url, path);
            let mut request = self
                .http_client
                .request(method.clone(), &url)
                .header("Authorization", format!("Bearer {}", token));

            if let Some(ref q) = query {
                request = request.query(q);
            }

            if let Some(ref b) = body {
                request = request
                    .header("Content-Type", "application/json; charset=utf-8")
                    .json(b);
            }

            debug!(
                "API request: {} {} (attempt {})",
                method,
                path,
                attempt + 1
            );

            match request.send().await {
                Ok(response) => {
                    let status = response.status();
                    let response_text = response.text().await?;
                    let response_body: Value = match serde_json::from_str(&response_text) {
                        Ok(v) => v,
                        Err(e) => {
                            warn!(
                                "Failed to parse response as JSON (status: {}): {}\nRaw response: {}",
                                status, e, response_text
                            );
                            return Err(BaseClientError::SerializationError(e));
                        }
                    };

                    // HTTP 429 - 速率限制
                    if status == StatusCode::TOO_MANY_REQUESTS {
                        let delay = Duration::from_secs(2_u64.pow(attempt) + 1);
                        warn!("HTTP 429 rate limited, retrying in {:?}", delay);
                        tokio::time::sleep(delay).await;
                        continue;
                    }

                    // HTTP 401 - token过期，清除缓存并重试
                    if status == StatusCode::UNAUTHORIZED {
                        warn!("HTTP 401 unauthorized, clearing token cache and retrying");
                        self.clear_token_cache().await;
                        continue;
                    }

                    if !status.is_success() {
                        return Err(BaseClientError::ApiError {
                            code: status.as_u16() as i32,
                            msg: format!("HTTP {}: {:?}", status, response_body),
                        });
                    }

                    // 检查API级别的错误码
                    let code = response_body
                        .get("code")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(-1) as i32;

                    match code {
                        // 可重试的错误：速率限制、写冲突、超时、数据未就绪
                        1254290 | 1254291 | 1255040 | 1254607 => {
                            let delay = Duration::from_secs(2_u64.pow(attempt) + 1);
                            warn!(
                                "Retryable API error {} ({}), retrying in {:?}",
                                code,
                                response_body
                                    .get("msg")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown"),
                                delay
                            );
                            tokio::time::sleep(delay).await;
                            continue;
                        }
                        0 => {
                            return Ok(response_body);
                        }
                        _ => {
                            let msg = response_body
                                .get("msg")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown error")
                                .to_string();
                            return Err(BaseClientError::ApiError { code, msg });
                        }
                    }
                }
                Err(e) => {
                    if e.is_timeout() || e.is_connect() {
                        let delay = Duration::from_secs(2_u64.pow(attempt) + 1);
                        warn!(
                            "Network error (attempt {}): {}, retrying in {:?}",
                            attempt + 1,
                            e,
                            delay
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    return Err(BaseClientError::HttpError(e));
                }
            }
        }

        Err(BaseClientError::MaxRetriesExceeded)
    }

    /// 获取待办的任务列表
    pub async fn get_pending_tasks(&self, runtime_id: &str) -> Result<Vec<Task>, BaseClientError> {
        // 使用API过滤任务状态和执行者
        let filter = format!(
            "AND(CurrentValue.[任务状态]=\"待办\",CurrentValue.[执行者]=\"{}\")",
            runtime_id
        );

        let path = format!(
            "/open-apis/bitable/v1/apps/{}/tables/{}/records",
            self.base_token().await, self.task_table_id().await
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
            "Fetched {} pending tasks for runtime {}",
            tasks.len(),
            runtime_id
        );

        Ok(tasks)
    }

    /// 更新任务状态并追加执行日志
    pub async fn update_task_status(
        &self,
        task_id: &str,
        status: &str,
        execution_log: &str,
    ) -> Result<(), BaseClientError> {
        // 先读取当前任务的执行日志
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

        let path = format!(
            "/open-apis/bitable/v1/apps/{}/tables/{}/records/{}",
            self.base_token().await, self.task_table_id().await, task_id
        );

        let body = json!({
            "fields": {
                "任务状态": status,
                "执行日志": new_log,
            }
        });

        self.api_request(reqwest::Method::PUT, &path, Some(body), None)
            .await?;

        info!("Updated task {} status to {}", task_id, status);

        Ok(())
    }

    /// 清空任务的审核驳回理由
    pub async fn clear_task_rejection_reason(
        &self,
        task_id: &str,
    ) -> Result<(), BaseClientError> {
        let path = format!(
            "/open-apis/bitable/v1/apps/{}/tables/{}/records/{}",
            self.base_token().await, self.task_table_id().await, task_id
        );

        let body = json!({
            "fields": {
                "审核驳回理由": "",
            }
        });

        self.api_request(reqwest::Method::PUT, &path, Some(body), None)
            .await?;

        info!("Cleared rejection reason for task {}", task_id);
        Ok(())
    }

    /// 获取任务的指定字段值
    async fn get_task_field(&self, task_id: &str, field_name: &str) -> Result<String, BaseClientError> {
        let path = format!(
            "/open-apis/bitable/v1/apps/{}/tables/{}/records/{}",
            self.base_token().await, self.task_table_id().await, task_id
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
            Some(v) if v.is_string() => Ok(v.as_str().unwrap_or("").to_string()),
            _ => Ok(String::new()),
        }
    }

    /// 根据主机名查找现有运行时
    pub async fn find_runtime_by_hostname(
        &self,
        hostname: &str,
    ) -> Result<Option<RuntimeInfo>, BaseClientError> {
        let filter = format!("CurrentValue.[主机名]=\"{}\"", hostname);

        let path = format!(
            "/open-apis/bitable/v1/apps/{}/tables/{}/records",
            self.base_token().await, self.runtime_table_id().await
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
            "Found existing runtime {} for hostname {}",
            runtime_info.runtime_id, hostname
        );

        Ok(Some(runtime_info))
    }

    /// 注册运行时
    pub async fn register_runtime(&self, runtime_info: &RuntimeInfo) -> Result<(), BaseClientError> {
        let path = format!(
            "/open-apis/bitable/v1/apps/{}/tables/{}/records",
            self.base_token().await, self.runtime_table_id().await
        );

        let body = json!({
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

        self.api_request(reqwest::Method::POST, &path, Some(body), None)
            .await?;

        info!("Registered runtime {}", runtime_info.runtime_id);

        Ok(())
    }

    /// 更新运行时心跳
    pub async fn update_heartbeat(&self, runtime_info: &RuntimeInfo) -> Result<(), BaseClientError> {
        // 先查找运行时记录
        let filter = format!("CurrentValue.[运行时ID]=\"{}\"", runtime_info.runtime_id);

        let path = format!(
            "/open-apis/bitable/v1/apps/{}/tables/{}/records",
            self.base_token().await, self.runtime_table_id().await
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
            // 运行时记录不存在，先注册
            warn!(
                "Runtime {} not found, registering instead",
                runtime_info.runtime_id
            );
            return self.register_runtime(runtime_info).await;
        }

        let record_id = items[0]
            .get("record_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let update_path = format!(
            "/open-apis/bitable/v1/apps/{}/tables/{}/records/{}",
            self.base_token().await, self.runtime_table_id().await, record_id
        );

        let body = json!({
            "fields": {
                "最后心跳": chrono::Utc::now().timestamp_millis(),
                "状态": runtime_status_to_string(&runtime_info.status),
            }
        });

        self.api_request(reqwest::Method::PUT, &update_path, Some(body), None)
            .await?;

        debug!("Updated heartbeat for runtime {}", runtime_info.runtime_id);

        Ok(())
    }

    /// 创建执行记录，返回记录ID
    pub async fn create_execution_log(&self, log: &ExecutionLog) -> Result<String, BaseClientError> {
        let path = format!(
            "/open-apis/bitable/v1/apps/{}/tables/{}/records",
            self.base_token().await, self.execution_log_table_id().await
        );

        let linked_task_ids: Vec<String> = log
            .linked_task
            .iter()
            .map(|link| link.id.clone())
            .collect();

        let body = json!({
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
            .unwrap_or("")
            .to_string();

        debug!(
            "Created execution log {} for task sequence {}",
            record_id, log.execution_sequence
        );

        Ok(record_id)
    }

    /// 更新执行记录
    pub async fn update_execution_log(
        &self,
        record_id: &str,
        log: &ExecutionLog,
    ) -> Result<(), BaseClientError> {
        let path = format!(
            "/open-apis/bitable/v1/apps/{}/tables/{}/records/{}",
            self.base_token().await, self.execution_log_table_id().await, record_id
        );

        let body = json!({
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

        debug!("Updated execution log {}", record_id);

        Ok(())
    }
}

/// 从API响应解析Task
fn parse_task_from_record(record: Value) -> Task {
    let fields = record.get("fields").cloned().unwrap_or(json!({}));
    let record_id = record
        .get("record_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
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

fn get_str_field(fields: &Value, name: &str) -> String {
    match fields.get(name) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Object(map)) => {
            map.get("link")
                .and_then(|v| v.as_str())
                .or_else(|| map.get("text").and_then(|v| v.as_str()))
                .unwrap_or("")
                .to_string()
        }
        Some(Value::Array(arr)) if !arr.is_empty() => {
            arr[0].as_str().unwrap_or("").to_string()
        }
        _ => String::new(),
    }
}

/// 获取可选字符串字段
fn get_opt_str_field(fields: &Value, name: &str) -> Option<String> {
    fields.get(name).and_then(|v| v.as_str()).map(String::from)
}

/// 获取u32字段
fn get_u32_field(fields: &Value, name: &str) -> u32 {
    fields
        .get(name)
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32
}

/// 获取f64字段
fn get_f64_field(fields: &Value, name: &str) -> f64 {
    fields
        .get(name)
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
}

/// 解析日期时间字段（毫秒时间戳）
fn parse_datetime_field(fields: &Value, name: &str) -> Option<chrono::NaiveDateTime> {
    fields
        .get(name)
        .and_then(|v| v.as_i64())
        .and_then(chrono::DateTime::from_timestamp_millis)
        .map(|dt| dt.naive_utc())
}

/// 解析任务状态
fn parse_task_status(status_str: &str) -> Status {
    match status_str {
        "待办" => Status::Todo,
        "进行中" => Status::InProgress,
        "待审核" => Status::PendingReview,
        "已完成" => Status::Completed,
        "已取消" => Status::Cancelled,
        _ => Status::Todo,
    }
}

/// 解析执行者类型
fn parse_executor_type(type_str: &str) -> ExecutorType {
    match type_str.to_lowercase().as_str() {
        "human" | "人工" => ExecutorType::Human,
        _ => ExecutorType::Agent,
    }
}

/// 解析优先级
fn parse_priority(priority_str: &str) -> Priority {
    match priority_str {
        "P0" => Priority::P0,
        "P1" => Priority::P1,
        "P2" => Priority::P2,
        "P3" => Priority::P3,
        _ => Priority::P2,
    }
}

/// 解析Agent类型字段
fn parse_agent_type_field(fields: &Value, name: &str) -> Option<AgentType> {
    let type_str = fields.get(name).and_then(|v| v.as_str())?;
    match type_str.to_lowercase().as_str() {
        "claude-code" | "claude" => Some(AgentType::ClaudeCode),
        "codex" => Some(AgentType::Codex),
        "opencode" => Some(AgentType::Opencode),
        "cursor" => Some(AgentType::Cursor),
        "其他" | "other" => Some(AgentType::Other),
        _ => None,
    }
}

/// 解析关联字段
fn parse_link_field(fields: &Value, name: &str) -> Vec<TaskLinkRecord> {
    let arr = match fields.get(name).and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return Vec::new(),
    };

    arr.iter()
        .filter_map(|item| {
            item.get("id")
                .and_then(|v| v.as_str())
                .map(|id| TaskLinkRecord {
                    id: id.to_string(),
                })
                .or_else(|| {
                    item.as_str().map(|id| TaskLinkRecord {
                        id: id.to_string(),
                    })
                })
        })
        .collect()
}

/// 解析运行时状态
fn parse_runtime_status(status_str: &str) -> RuntimeStatus {
    match status_str {
        "在线" => RuntimeStatus::Online,
        "离线" => RuntimeStatus::Offline,
        "忙碌" => RuntimeStatus::Busy,
        _ => RuntimeStatus::Offline,
    }
}

/// 运行时状态转字符串
fn runtime_status_to_string(status: &RuntimeStatus) -> String {
    match status {
        RuntimeStatus::Online => "在线".to_string(),
        RuntimeStatus::Offline => "离线".to_string(),
        RuntimeStatus::Busy => "忙碌".to_string(),
    }
}

/// Agent类型转字符串
fn agent_type_to_string(agent_type: &ExecutionAgentType) -> String {
    match agent_type {
        ExecutionAgentType::ClaudeCode => "claude-code".to_string(),
        ExecutionAgentType::Codex => "codex".to_string(),
        ExecutionAgentType::Opencode => "opencode".to_string(),
        ExecutionAgentType::Cursor => "cursor".to_string(),
        ExecutionAgentType::Other => "其他".to_string(),
    }
}

/// 执行状态转字符串
fn execution_status_to_string(status: &ExecutionStatus) -> String {
    match status {
        ExecutionStatus::Success => "成功".to_string(),
        ExecutionStatus::Failed => "失败".to_string(),
        ExecutionStatus::InProgress => "进行中".to_string(),
        ExecutionStatus::Timeout => "超时".to_string(),
    }
}

/// 触发方式转字符串
fn trigger_mode_to_string(mode: &TriggerMode) -> String {
    match mode {
        TriggerMode::Manual => "手动".to_string(),
        TriggerMode::Auto => "自动".to_string(),
        TriggerMode::Workflow => "工作流".to_string(),
    }
}
