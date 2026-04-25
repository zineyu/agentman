use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::interval;
use tracing::{error, info};

use crate::agent::factory::AgentFactory;
use crate::client::BaseClient;
use crate::config::DaemonConfig;
use crate::git::workspace::WorkspaceManager;
use crate::models::{
    execution::{AgentType as ExecutionAgentType, ExecutionLog, ExecutionStatus, LinkRecord, TriggerMode},
    task::{AgentType, Task},
};

const FLUSH_INTERVAL_SECS: u64 = 10;
const MAX_RETRIES: u32 = 3;

pub struct TaskExecutor {
    client: Arc<BaseClient>,
    config: DaemonConfig,
    workspace_manager: WorkspaceManager,
}

impl TaskExecutor {
    pub fn new(client: Arc<BaseClient>, config: &DaemonConfig) -> Self {
        let workspace_manager =
            WorkspaceManager::new(config.workspace_dir.clone());
        Self {
            client,
            config: config.clone(),
            workspace_manager,
        }
    }

    pub async fn run_once(&self,
    ) -> anyhow::Result<()> {
        info!("Executing tasks (once mode)");
        self.process_tasks().await
    }

    pub async fn run_loop(&self,
    ) -> anyhow::Result<()> {
        info!("Starting task execution loop");
        let mut ticker = interval(Duration::from_secs(self.config.poll_interval_secs));

        loop {
            ticker.tick().await;
            if let Err(e) = self.process_tasks().await {
                error!("Error processing tasks: {}", e);
            }
        }
    }

    async fn process_tasks(&self,
    ) -> anyhow::Result<()> {
        let tasks = self
            .client
            .get_pending_tasks(&self.config.runtime_id)
            .await?;

        info!("Found {} pending tasks", tasks.len());

        for task in tasks {
            if let Err(e) = self.process_single_task(task).await {
                error!("Failed to process task: {}", e);
            }
        }

        Ok(())
    }

    async fn process_single_task(
        &self,
        mut task: Task,
    ) -> anyhow::Result<()> {
        let task_id = task.record_id.clone();
        info!("Processing task {}: {}", task.id, task.title);

        if task.record_id.is_empty() {
            error!("Task {} has empty record_id, skipping", task.id);
            return Err(anyhow::anyhow!("Task has empty record_id"));
        }

        // 检测是否是审核驳回后的重试
        let is_rejection_retry = !task.review_rejection_reason.is_empty();
        if is_rejection_retry {
            info!(
                "Task {} is retrying after rejection: {}",
                task.id, task.review_rejection_reason
            );

            // 将驳回理由追加到任务描述中，作为额外上下文
            task.description = format!(
                "[审核驳回 - 第{}次重试]\n驳回理由: {}\n\n原始任务描述:\n{}",
                task.retry_count + 1,
                task.review_rejection_reason,
                task.description
            );

            // 清空审核驳回理由，避免重复处理
            if let Err(e) = self
                .client
                .clear_task_rejection_reason(&task_id)
                .await
            {
                error!("Failed to clear rejection reason: {}", e);
            }
        }

        let status_message = if is_rejection_retry {
            format!(
                "第{}次重试（审核驳回: {}）",
                task.retry_count + 1,
                task.review_rejection_reason
            )
        } else {
            "任务开始执行".to_string()
        };

        if let Err(e) = self
            .client
            .update_task_status(&task_id, "进行中", &status_message)
            .await
        {
            error!("Failed to update task status to 进行中: {}", e);
            return Err(e.into());
        }

        let workspace = self
            .workspace_manager
            .prepare_workspace(&task_id);
        let work_dir = workspace.join("work");
        std::fs::create_dir_all(&work_dir)?;

        let mut execution_log = ExecutionLog {
            id: 0,
            linked_task: vec![LinkRecord {
                id: task_id.clone(),
            }],
            execution_sequence: task.retry_count + 1,
            agent_type: match task.agent_type.unwrap_or(AgentType::Other) {
                AgentType::ClaudeCode => ExecutionAgentType::ClaudeCode,
                AgentType::Codex => ExecutionAgentType::Codex,
                AgentType::Opencode => ExecutionAgentType::Opencode,
                AgentType::Cursor => ExecutionAgentType::Cursor,
                AgentType::Other => ExecutionAgentType::Other,
            },
            execution_status: ExecutionStatus::InProgress,
            start_time: chrono::Local::now().naive_local(),
            end_time: None,
            execution_output: String::new(),
            error_info: String::new(),
            commit_hash: String::new(),
            trigger_mode: TriggerMode::Auto,
        };

        let log_record_id = match self
            .client
            .create_execution_log(&execution_log)
            .await
        {
            Ok(id) => id,
            Err(e) => {
                error!("Failed to create execution log: {}", e);
                return Err(e.into());
            }
        };

        let adapter = match AgentFactory::create(
            task.agent_type.unwrap_or(AgentType::Other),
        ) {
            Ok(adapter) => adapter,
            Err(e) => {
                error!("Failed to create agent adapter: {}", e);
                self.handle_execution_failure(
                    &task,
                    &format!("Agent adapter creation failed: {}", e),
                    &log_record_id,
                )
                .await?;
                return Err(e);
            }
        };

        let output_buffer = Arc::new(Mutex::new(String::new()));
        let needs_flush = Arc::new(AtomicBool::new(false));
        let flush_buffer = output_buffer.clone();
        let flush_flag = needs_flush.clone();
        let flush_client = self.client.clone();
        let flush_log_id = log_record_id.clone();

        let flush_handle = tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(FLUSH_INTERVAL_SECS));
            loop {
                ticker.tick().await;
                if flush_flag.swap(false, Ordering::Relaxed) {
                    let buf = flush_buffer.lock().await;
                    let log = ExecutionLog {
                        id: 0,
                        linked_task: vec![LinkRecord {
                            id: String::new(),
                        }],
                        execution_sequence: 0,
                        agent_type: ExecutionAgentType::Other,
                        execution_status: ExecutionStatus::InProgress,
                        start_time: chrono::Local::now().naive_local(),
                        end_time: None,
                        execution_output: buf.clone(),
                        error_info: String::new(),
                        commit_hash: String::new(),
                        trigger_mode: TriggerMode::Auto,
                    };
                    if let Err(e) = flush_client
                        .update_execution_log(&flush_log_id, &log
                        )
                        .await
                    {
                        error!("Failed to flush execution log: {}", e);
                    }
                }
            }
        });

        let stream_buffer = output_buffer.clone();
        let stream_flag = needs_flush.clone();
        let callback = Box::new(move |chunk: &str| {
            if let Ok(mut buf) = stream_buffer.try_lock() {
                buf.push_str(chunk);
                buf.push('\n');
                stream_flag.store(true, Ordering::Relaxed);
            }
        });

        let result = adapter
            .execute_with_stream(&task, &work_dir, callback
            )
            .await;

        flush_handle.abort();

        let execution_result = match result {
            Ok(result) => result,
            Err(e) => {
                error!("Agent execution error: {}", e);
                self.finalize_execution(
                    &task,
                    &work_dir,
                    &log_record_id,
                    &mut execution_log,
                    false,
                    &format!("Agent execution error: {}", e),
                    &output_buffer.lock().await,
                )
                .await?;
                return Err(e);
            }
        };

        let final_output = output_buffer.lock().await.clone();
        let success = execution_result.success;

        self.finalize_execution(
            &task,
            &work_dir,
            &log_record_id,
            &mut execution_log,
            success,
            &execution_result.error_info.unwrap_or_default(),
            &final_output,
        )
        .await?;

        if success {
            info!(
                "Task {} completed successfully, status set to 待审核",
                task.id
            );
        } else {
            info!(
                "Task {} failed, retry count: {}/{}",
                task.id,
                task.retry_count + 1,
                MAX_RETRIES
            );
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn finalize_execution(
        &self,
        task: &Task,
        _work_dir: &std::path::Path,
        log_record_id: &str,
        execution_log: &mut ExecutionLog,
        success: bool,
        error_info: &str,
        output: &str,
    ) -> anyhow::Result<()> {
        execution_log.end_time =
            Some(chrono::Local::now().naive_local());
        execution_log.execution_output = output.to_string();
        execution_log.error_info = error_info.to_string();
        execution_log.execution_status = if success {
            ExecutionStatus::Success
        } else {
            ExecutionStatus::Failed
        };

        if let Err(e) = self
            .client
            .update_execution_log(log_record_id, execution_log)
            .await
        {
            error!("Failed to update execution log: {}", e);
        }

        if success {
            if let Err(e) = self
                .client
                .update_task_status(
                    &task.record_id,
                    "待审核",
                    &format!("执行成功\n输出:\n{}", output),
                )
                .await
            {
                error!("Failed to update task status to 待审核: {}", e);
                return Err(e.into());
            }
        } else {
            self.handle_execution_failure(
                task, error_info, log_record_id,
            )
            .await?;
        }

        Ok(())
    }

    async fn handle_execution_failure(
        &self,
        task: &Task,
        error_info: &str,
        _log_record_id: &str,
    ) -> anyhow::Result<()> {
        let new_retry_count = task.retry_count + 1;
        let (status, message) = if new_retry_count < MAX_RETRIES {
            (
                "待办",
                format!(
                    "执行失败，准备第{}次重试\n错误: {}",
                    new_retry_count + 1,
                    error_info
                ),
            )
        } else {
            (
                "已取消",
                format!(
                    "执行失败超过最大重试次数({}/{})\n错误: {}",
                    MAX_RETRIES, MAX_RETRIES, error_info
                ),
            )
        };

        if let Err(e) = self
            .client
            .update_task_status(&task.record_id, status, &message
            )
            .await
        {
            error!("Failed to update task status on failure: {}", e);
            return Err(e.into());
        }

        info!(
            "Task {} status updated to {} after failure",
            task.id, status
        );
        Ok(())
    }
}
