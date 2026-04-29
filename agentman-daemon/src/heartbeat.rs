use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::client::BaseClient;
use crate::config::DaemonConfig;
use crate::models::RuntimeInfo;

pub struct HeartbeatService {
    client: Arc<BaseClient>,
    runtime: Arc<RwLock<RuntimeInfo>>,
    interval_secs: u64,
    cancel_token: CancellationToken,
}

impl HeartbeatService {
    pub fn new(
        client: Arc<BaseClient>,
        runtime: Arc<RwLock<RuntimeInfo>>,
        config: &DaemonConfig,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            client,
            runtime,
            interval_secs: config.heartbeat_interval_secs,
            cancel_token,
        }
    }

    pub async fn start(&self) {
        let client = self.client.clone();
        let runtime = self.runtime.clone();
        let interval_secs = self.interval_secs;
        let cancel = self.cancel_token.clone();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(interval_secs));

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        let runtime_info = runtime.read().await;
                        match client.update_heartbeat(&runtime_info).await {
                            Ok(_) => debug!("心跳发送成功"),
                            Err(e) => warn!("发送心跳失败: {}", e),
                        }
                    }
                    _ = cancel.cancelled() => {
                        debug!("心跳服务正在关闭");
                        break;
                    }
                }
            }
        });
    }
}
