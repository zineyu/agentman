use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::config::DaemonConfig;

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

    #[error("{message}")]
    Other { message: String },
}

/// 缓存的token信息
#[derive(Clone)]
pub(crate) struct TokenInfo {
    pub(crate) token: String,
    pub(crate) expires_at: Instant,
}

/// 表ID缓存
#[derive(Clone, Default)]
pub(crate) struct TableIds {
    pub(crate) task_table_id: String,
    pub(crate) runtime_table_id: String,
    pub(crate) execution_log_table_id: String,
}

/// Lark Base API 客户端
pub struct BaseClient {
    pub(crate) http_client: Client,
    pub(crate) config: DaemonConfig,
    pub(crate) token_cache: Arc<RwLock<Option<TokenInfo>>>,
    pub(crate) table_ids: Arc<RwLock<TableIds>>,
    pub(crate) runtime_record_id: Arc<RwLock<Option<String>>>,
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
            runtime_record_id: Arc::new(RwLock::new(None)),
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

        debug!("{}", rust_i18n::t!("base_client.token_refreshed", seconds = expire_secs));

        Ok(token)
    }

    /// 清除token缓存
    pub(crate) async fn clear_token_cache(&self) {
        let mut cache = self.token_cache.write().await;
        *cache = None;
    }

    /// 初始化表ID，通过查询Base表列表获取
    pub async fn init_table_ids(&self) -> Result<(), BaseClientError> {
        let path = format!(
            "/open-apis/bitable/v1/apps/{}/tables",
            self.config.base_token
        );

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
                msg: "Tasks table (任务主表) not found in Base".to_string(),
            });
        }

        info!(
            "{}",
            rust_i18n::t!(
                "base_client.table_ids_initialized",
                task = table_ids.task_table_id,
                runtime = table_ids.runtime_table_id,
                log = table_ids.execution_log_table_id
            )
        );

        Ok(())
    }

    /// 获取任务表ID
    pub(crate) async fn task_table_id(&self) -> String {
        self.table_ids.read().await.task_table_id.clone()
    }

    /// 获取运行时表ID
    pub(crate) async fn runtime_table_id(&self) -> String {
        self.table_ids.read().await.runtime_table_id.clone()
    }

    /// 获取执行记录表ID
    pub(crate) async fn execution_log_table_id(&self) -> String {
        self.table_ids.read().await.execution_log_table_id.clone()
    }

    /// 发送API请求，带重试逻辑
    pub(crate) async fn api_request(
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
                "{}",
                rust_i18n::t!(
                    "base_client.api_request",
                    method = method.to_string(),
                    path = path,
                    attempt = attempt + 1
                )
            );

            match request.send().await {
                Ok(response) => {
                    let status = response.status();
                    let response_text = response.text().await?;
                    let response_body: Value = match serde_json::from_str(&response_text) {
                        Ok(v) => v,
                        Err(e) => {
                            warn!(
                                "{}",
                                rust_i18n::t!(
                                    "base_client.parse_response_error",
                                    status = status.as_u16(),
                                    error = e,
                                    raw = response_text
                                )
                            );
                            return Err(BaseClientError::SerializationError(e));
                        }
                    };

                    // HTTP 429 - 速率限制
                    if status == StatusCode::TOO_MANY_REQUESTS {
                        let delay = Duration::from_secs(2_u64.pow(attempt) + 1);
                        warn!("{}", rust_i18n::t!("base_client.rate_limited", delay = format!("{:?}", delay)));
                        tokio::time::sleep(delay).await;
                        continue;
                    }

                    // HTTP 401 - token过期，清除缓存并重试
                    if status == StatusCode::UNAUTHORIZED {
                        warn!("{}", rust_i18n::t!("base_client.unauthorized"));
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
                                "{}",
                                rust_i18n::t!(
                                    "base_client.retryable_error",
                                    status = code,
                                    message = response_body.get("msg").and_then(|v| v.as_str()).unwrap_or("unknown"),
                                    delay = format!("{:?}", delay)
                                )
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
                            "{}",
                            rust_i18n::t!(
                                "base_client.network_error",
                                attempt = attempt + 1,
                                error = e,
                                delay = format!("{:?}", delay)
                            )
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
}
