use clap::Parser;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

rust_i18n::i18n!("locales", fallback = "en");

use agentman_daemon::{
    client::BaseClient,
    config::DaemonConfig,
    heartbeat::HeartbeatService,
    models::{FromConfig, RuntimeInfo, RuntimeStatus},
    task_executor::TaskExecutor,
};

#[derive(Parser)]
#[command(name = "agentman-daemon")]
#[command(about = "Agent task management daemon")]
struct Cli {
    #[arg(short, long, help = "Configuration file path")]
    config: Option<String>,

    #[arg(short, long, help = "Run once and exit")]
    once: bool,

    #[arg(long, help = "Register this runtime")]
    register: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config = DaemonConfig::load(cli.config.as_deref())?;

    info!("{}", rust_i18n::t!("daemon.starting"));
    info!("{}", rust_i18n::t!("daemon.runtime_id", id = config.runtime_id));
    info!("{}", rust_i18n::t!("daemon.base_url", url = config.base_url));

    let client = Arc::new(BaseClient::new(&config)?);

    info!("{}", rust_i18n::t!("daemon.init_table_ids"));
    client.init_table_ids().await?;

    let mut runtime_info = RuntimeInfo::from_config(&config);

    if cli.register {
        info!("{}", rust_i18n::t!("daemon.check_existing_runtime"));
        let hostname = runtime_info.hostname.clone();

        match client.find_runtime_by_hostname(&hostname).await? {
            Some(existing_runtime) => {
                info!(
                    "{}",
                    rust_i18n::t!(
                        "daemon.found_existing_runtime",
                        id = existing_runtime.runtime_id,
                        hostname = hostname
                    )
                );
                runtime_info.runtime_id = existing_runtime.runtime_id;
                runtime_info.runtime_name = existing_runtime.runtime_name;
            }
            None => {
                info!("{}", rust_i18n::t!("daemon.no_existing_runtime"));
                client.register_runtime(&runtime_info).await?;
            }
        }
    }

    let runtime = Arc::new(RwLock::new(runtime_info));

    let heartbeat = HeartbeatService::new(client.clone(), runtime.clone(), &config);
    heartbeat.start().await?;

    let executor = Arc::new(TaskExecutor::new(client.clone(), &config));

    if cli.once {
        info!("{}", rust_i18n::t!("daemon.once_mode"));
        executor.run_once().await?;
    } else {
        info!("{}", rust_i18n::t!("daemon.start_main_loop"));

        let executor_for_task = executor.clone();
        let executor_handle = tokio::spawn(async move {
            executor_for_task.run_loop().await
        });

        tokio::signal::ctrl_c().await?;
        info!("{}", rust_i18n::t!("daemon.received_shutdown_signal"));
        executor.shutdown();
        if let Err(e) = executor_handle.await {
            warn!("{}", rust_i18n::t!("daemon.executor_shutdown_error", error = e));
        }
    }

    info!("{}", rust_i18n::t!("daemon.shutting_down"));
    {
        let mut runtime_info = runtime.write().await;
        runtime_info.status = RuntimeStatus::Offline;
        if let Err(e) = client.update_heartbeat(&*runtime_info).await {
            warn!("{}", rust_i18n::t!("daemon.failed_update_status", error = e));
        }
    }
    Ok(())
}
