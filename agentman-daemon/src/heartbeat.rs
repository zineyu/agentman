use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{debug, warn};

use crate::client::BaseClient;
use crate::config::DaemonConfig;
use crate::models::RuntimeInfo;

pub struct HeartbeatService {
    client: Arc<BaseClient>,
    runtime: Arc<RwLock<RuntimeInfo>>,
    interval_secs: u64,
}

impl HeartbeatService {
    pub fn new(
        client: Arc<BaseClient>,
        runtime: Arc<RwLock<RuntimeInfo>>,
        config: &DaemonConfig,
    ) -> Self {
        Self {
            client,
            runtime,
            interval_secs: config.heartbeat_interval_secs,
        }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let client = self.client.clone();
        let runtime = self.runtime.clone();
        let interval_secs = self.interval_secs;

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(interval_secs));

            loop {
                ticker.tick().await;

                let runtime_info = runtime.read().await;
                match client.update_heartbeat(&runtime_info).await {
                    Ok(_) => debug!("{}", rust_i18n::t!("heartbeat.sent_successfully")),
                    Err(e) => warn!("{}", rust_i18n::t!("heartbeat.failed", error = e)),
                }
            }
        });

        Ok(())
    }
}
