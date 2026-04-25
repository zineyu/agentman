use clap::Parser;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use agentman_daemon::{
    client::BaseClient,
    config::DaemonConfig,
    heartbeat::HeartbeatService,
    models::{FromConfig, RuntimeInfo},
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

    info!("Agentman Daemon starting...");
    info!("Runtime ID: {}", config.runtime_id);
    info!("Base URL: {}", config.base_url);

    let client = Arc::new(BaseClient::new(&config)?);

    info!("Initializing table IDs...");
    client.init_table_ids().await?;

    let mut runtime_info = RuntimeInfo::from_config(&config);

    if cli.register {
        info!("Checking for existing runtime...");
        let hostname = runtime_info.hostname.clone();

        match client.find_runtime_by_hostname(&hostname).await? {
            Some(existing_runtime) => {
                info!(
                    "Found existing runtime {} for hostname {}, reusing",
                    existing_runtime.runtime_id, hostname
                );
                runtime_info.runtime_id = existing_runtime.runtime_id;
                runtime_info.runtime_name = existing_runtime.runtime_name;
            }
            None => {
                info!("No existing runtime found, registering new runtime...");
                client.register_runtime(&runtime_info).await?;
            }
        }
    }

    let runtime = Arc::new(RwLock::new(runtime_info));

    let heartbeat = HeartbeatService::new(client.clone(), runtime.clone(), &config);
    heartbeat.start().await?;

    let executor = TaskExecutor::new(client.clone(), &config);

    if cli.once {
        info!("Running in once mode");
        executor.run_once().await?;
    } else {
        info!("Starting main loop");
        executor.run_loop().await?;
    }

    info!("Agentman Daemon shutting down...");
    Ok(())
}
