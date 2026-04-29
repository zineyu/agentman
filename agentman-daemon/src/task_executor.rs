use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use thiserror::Error;
use tokio::sync::Mutex;
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, instrument};

use crate::agent::factory::AgentFactory;
use crate::client::BaseClient;
use crate::config::DaemonConfig;
use std::path::PathBuf;
use crate::models::{
    dependency::DependencyCheckResult,
    execution::{AgentType as ExecutionAgentType, ExecutionLog, ExecutionStatus, LinkRecord, TriggerMode},
    task::{AgentType, Status, Task},
};

/// Task executor error types
#[derive(Error, Debug)]
pub enum TaskExecutorError {
    /// Invalid task (missing record_id)
    #[error("Invalid task: {0}")]
    InvalidTask(String),
    /// Client operation failed
    #[error("Client error: {0}")]
    ClientError(#[from] crate::client::BaseClientError),
    /// IO operation failed
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    /// Agent operation failed
    #[error("Agent error: {0}")]
    AgentError(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, TaskExecutorError>;

const FLUSH_INTERVAL_SECS: u64 = 10;
const MAX_RETRIES: u32 = 3;

pub struct TaskExecutor {
    client: Arc<BaseClient>,
    config: DaemonConfig,
    shutdown: AtomicBool,
    cancel_token: CancellationToken,
}

impl TaskExecutor {
    pub fn new(
        client: Arc<BaseClient>,
        config: &DaemonConfig,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            client,
            config: config.clone(),
            shutdown: AtomicBool::new(false),
            cancel_token,
        }
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.cancel_token.cancel();
    }

    #[instrument(skip(self))]
    pub async fn run_once(&self,
    ) -> Result<()> {
        info!("执行单次模式任务");
        self.process_tasks().await
    }

    #[instrument(skip(self))]
    pub async fn run_loop(&self,
    ) -> Result<()> {
        info!("启动任务执行循环");
        let mut ticker = interval(Duration::from_secs(self.config.poll_interval_secs));

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if self.shutdown.load(Ordering::Relaxed) {
                        info!("收到关闭请求，停止执行循环");
                        break;
                    }
                    if let Err(e) = self.process_tasks().await {
                        error!("处理任务时出错: {}", e);
                    }
                }
                _ = self.cancel_token.cancelled() => {
                    info!("收到关闭请求，停止执行循环");
                    break;
                }
            }
        }
        Ok(())
    }

    #[instrument(skip(self))]
    async fn process_tasks(&self,
    ) -> Result<()> {
        let tasks = self
            .client
            .get_pending_tasks(&self.config.runtime_id)
            .await?;

        // 依赖检查：过滤掉有未完成阻塞依赖的任务
        let ready_tasks = self.filter_tasks_by_dependencies(tasks).await?;

        info!("找到 {} 个待办任务", ready_tasks.len());

        for task in ready_tasks {
            if self.cancel_token.is_cancelled() {
                info!("收到关闭请求，停止执行循环");
                break;
            }
            if let Err(e) = self.process_single_task(task).await {
                error!("处理任务时出错: {}", e);
            }
        }

        Ok(())
    }

    /// 检查任务依赖状态，过滤掉有未完成阻塞依赖的任务
    #[instrument(skip(self, tasks))]
    async fn filter_tasks_by_dependencies(
        &self,
        tasks: Vec<Task>,
    ) -> Result<Vec<Task>> {
        // 收集所有任务的前置任务 ID
        let all_dep_ids: Vec<String> = tasks
            .iter()
            .flat_map(|t| t.dependencies.iter().map(|d| d.id.clone()))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        if all_dep_ids.is_empty() {
            // 没有任何依赖，全部返回
            return Ok(tasks);
        }

        // 批量查询前置任务状态
        let dep_statuses = match self
            .client
            .get_tasks_status(&all_dep_ids)
            .await
        {
            Ok(statuses) => statuses,
            Err(e) => {
                error!("查询依赖任务状态失败: {}", e);
                // 查询失败时保守处理：认为所有任务都有未完成的依赖
                return Ok(Vec::new());
            }
        };

        let mut ready_tasks = Vec::new();
        for task in tasks {
            if task.dependencies.is_empty() {
                ready_tasks.push(task);
                continue;
            }

            let check_result = check_single_task_dependencies(&task, &dep_statuses);
            match check_result {
                DependencyCheckResult::Ready => {
                    ready_tasks.push(task);
                }
                DependencyCheckResult::Blocked { unmet } => {
                    info!("任务 {} 被阻塞，等待前置依赖完成: {}", task.id, unmet.join(", "));
                }
                DependencyCheckResult::ReadyWithWarnings { warnings } => {
                    info!("任务 {} 可以执行，但前置依赖存在警告: {}", task.id, warnings.join(", "));
                    ready_tasks.push(task);
                }
            }
        }

        Ok(ready_tasks)
    }

    #[instrument(skip(self, task), fields(task_id = task.id))]
    async fn process_single_task(
        &self,
        mut task: Task,
    ) -> Result<()> {
        let task_id = task.record_id.clone();
        info!("正在处理任务 {}: {}", task.id, task.title);

        if task.record_id.is_empty() {
            error!("任务 {} 的 record_id 为空，跳过", task.id);
            return Err(TaskExecutorError::InvalidTask(
                format!("Task {} has empty record_id", task.id)
            ));
        }

        // 检测是否是审核驳回后的重试
        let is_rejection_retry = !task.review_rejection_reason.is_empty();
        if is_rejection_retry {
            info!("任务 {} 因驳回而重试: {}", task.id, task.review_rejection_reason);

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
                error!("清空驳回理由失败: {}", e);
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
            .update_task_status(&task_id, Status::InProgress, &status_message)
            .await
        {
            error!("更新任务状态为进行中失败: {}", e);
            return Err(e.into());
        }

        let workspace = PathBuf::from(&self.config.workspace_dir).join(&task_id);
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
            summary: String::new(),
            trigger_mode: TriggerMode::Auto,
        };

        let log_record_id = match self
            .client
            .create_execution_log(&execution_log)
            .await
        {
            Ok(id) => id,
            Err(e) => {
                error!("创建执行日志失败: {}", e);
                return Err(e.into());
            }
        };

        let adapter = match AgentFactory::create(
            task.agent_type.unwrap_or(AgentType::Other),
        ) {
            Ok(adapter) => adapter,
            Err(e) => {
                error!("创建 Agent 适配器失败: {}", e);
                self.handle_execution_failure(
                    &task,
                    &format!("Agent 适配器创建失败: {}", e),
                    &log_record_id,
                )
                .await?;
                return Err(TaskExecutorError::AgentError(e.into()));
            }
        };

        let output_buffer = Arc::new(Mutex::new(String::new()));
        let needs_flush = Arc::new(AtomicBool::new(false));
        let flush_buffer = output_buffer.clone();
        let flush_flag = needs_flush.clone();
        let flush_client = self.client.clone();
        let flush_log_id = log_record_id.clone();
        let flush_cancel = self.cancel_token.clone();

        let flush_handle = tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(FLUSH_INTERVAL_SECS));
            loop {
                tokio::select! {
                    _ = ticker.tick() => {
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
                                summary: String::new(),
                                trigger_mode: TriggerMode::Auto,
                            };
                            if let Err(e) = flush_client
                                .update_execution_log(&flush_log_id, &log
                                )
                                .await
                            {
                                error!("刷新执行日志失败: {}", e);
                            }
                        }
                    }
                    _ = flush_cancel.cancelled() => {
                        break;
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
                error!("Agent 执行错误: {}", e);
                self.finalize_execution(
                    &task,
                    &log_record_id,
                    &mut execution_log,
                    false,
                    &format!("Agent 执行错误: {}", e),
                    &output_buffer.lock().await,
                )
                .await?;
                return Err(TaskExecutorError::AgentError(e.into()));
            }
        };

        let final_output = output_buffer.lock().await.clone();
        let success = execution_result.success;

        self.finalize_execution(
            &task,
            &log_record_id,
            &mut execution_log,
            success,
            &execution_result.error_info.unwrap_or_default(),
            &final_output,
        )
        .await?;

        if success {
            info!("任务 {} 执行成功，状态设置为待审核", task.id);
        } else {
            info!("任务 {} 失败，重试次数: {}/{}", task.id, task.retry_count + 1, MAX_RETRIES);
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    #[instrument(skip(self, execution_log), fields(task_id = task.id, success))]
    async fn finalize_execution(
        &self,
        task: &Task,
        log_record_id: &str,
        execution_log: &mut ExecutionLog,
        success: bool,
        error_info: &str,
        output: &str,
    ) -> Result<()> {
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
            error!("更新执行日志失败: {}", e);
        }

        if success {
            if let Err(e) = self
                .client
                .update_task_status(
                    &task.record_id,
                    Status::PendingReview,
                    &format!("执行成功\n输出:\n{}", output),
                )
                .await
            {
                error!("更新任务状态为待审核失败: {}", e);
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

    #[instrument(skip(self), fields(task_id = task.id))]
    async fn handle_execution_failure(
        &self,
        task: &Task,
        error_info: &str,
        _log_record_id: &str,
    ) -> Result<()> {
        let new_retry_count = task.retry_count + 1;
        let (status, message) = if new_retry_count < MAX_RETRIES {
            (
                Status::Todo,
                format!(
                    "执行失败，准备第{}次重试\n错误: {}",
                    new_retry_count + 1,
                    error_info
                ),
            )
        } else {
            (
                Status::Cancelled,
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
            error!("更新失败状态失败: {}", e);
            return Err(e.into());
        }

        info!("任务 {} 失败后状态更新为 {}", task.id, status);
        Ok(())
    }
}

/// 检查单个任务的依赖状态
fn check_single_task_dependencies(
    task: &Task,
    dep_statuses: &std::collections::HashMap<String, Status>,
) -> DependencyCheckResult {
    let mut unmet = Vec::new();
    let warnings = Vec::new();

    for dep in &task.dependencies {
        let dep_status = dep_statuses.get(&dep.id);
        match dep_status {
            Some(Status::Completed) => {
                // 依赖已完成，正常
            }
            Some(other_status) => {
                // 前置任务未完成（默认所有依赖均为阻塞型）
                unmet.push(format!(
                    "{} (状态: {})",
                    dep.id,
                    other_status
                ));
            }
            None => {
                // 查询不到依赖任务状态（可能已被删除）
                unmet.push(format!(
                    "{} (任务不存在)",
                    dep.id
                ));
            }
        }
    }

    if !unmet.is_empty() {
        DependencyCheckResult::Blocked { unmet }
    } else if !warnings.is_empty() {
        DependencyCheckResult::ReadyWithWarnings { warnings }
    } else {
        DependencyCheckResult::Ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::task::LinkRecord;

    fn create_test_task(id: u64, record_id: &str, deps: &[&str]) -> Task {
        Task {
            record_id: record_id.to_string(),
            id,
            title: format!("任务{}", id),
            description: String::new(),
            executor_type: crate::models::task::ExecutorType::Agent,
            executor: "test".to_string(),
            status: Status::Todo,
            priority: crate::models::task::Priority::P2,
            start_time: None,
            deadline: None,
            completed_at: None,
            last_urge_time: None,
            agent_type: None,
            work_dir: String::new(),
            reviewer: None,
            review_comment: String::new(),
            review_rejection_reason: String::new(),
            retry_count: 0,
            urge_count: 0,
            estimated_hours: 0.0,
            assigned_runtime: Vec::new(),
            dependencies: deps.iter().map(|id| LinkRecord { id: id.to_string() }).collect(),
        }
    }

    #[test]
    fn test_check_deps_no_dependencies() {
        let task = create_test_task(1, "rec1", &[]);
        let statuses = std::collections::HashMap::new();
        let result = check_single_task_dependencies(&task, &statuses);
        assert_eq!(result, DependencyCheckResult::Ready);
    }

    #[test]
    fn test_check_deps_all_completed() {
        let task = create_test_task(1, "rec1", &["dep1", "dep2"]);
        let mut statuses = std::collections::HashMap::new();
        statuses.insert("dep1".to_string(), Status::Completed);
        statuses.insert("dep2".to_string(), Status::Completed);
        let result = check_single_task_dependencies(&task, &statuses);
        assert_eq!(result, DependencyCheckResult::Ready);
    }

    #[test]
    fn test_check_deps_one_unmet() {
        let task = create_test_task(1, "rec1", &["dep1", "dep2"]);
        let mut statuses = std::collections::HashMap::new();
        statuses.insert("dep1".to_string(), Status::Completed);
        statuses.insert("dep2".to_string(), Status::InProgress);
        let result = check_single_task_dependencies(&task, &statuses);
        match result {
            DependencyCheckResult::Blocked { unmet } => {
                assert_eq!(unmet.len(), 1);
                assert!(unmet[0].contains("dep2"));
                assert!(unmet[0].contains("进行中"));
            }
            _ => panic!("Expected Blocked, got {:?}", result),
        }
    }

    #[test]
    fn test_check_deps_missing_dep() {
        let task = create_test_task(1, "rec1", &["dep1"]);
        let statuses = std::collections::HashMap::new();
        let result = check_single_task_dependencies(&task, &statuses);
        match result {
            DependencyCheckResult::Blocked { unmet } => {
                assert_eq!(unmet.len(), 1);
                assert!(unmet[0].contains("任务不存在"));
            }
            _ => panic!("Expected Blocked, got {:?}", result),
        }
    }

    #[test]
    fn test_check_deps_multiple_unmet() {
        let task = create_test_task(1, "rec1", &["dep1", "dep2", "dep3"]);
        let mut statuses = std::collections::HashMap::new();
        statuses.insert("dep1".to_string(), Status::Completed);
        statuses.insert("dep2".to_string(), Status::Todo);
        statuses.insert("dep3".to_string(), Status::PendingReview);
        let result = check_single_task_dependencies(&task, &statuses);
        match result {
            DependencyCheckResult::Blocked { unmet } => {
                assert_eq!(unmet.len(), 2);
            }
            _ => panic!("Expected Blocked, got {:?}", result),
        }
    }

    #[test]
    fn test_check_deps_cancelled_dep() {
        let task = create_test_task(1, "rec1", &["dep1"]);
        let mut statuses = std::collections::HashMap::new();
        statuses.insert("dep1".to_string(), Status::Cancelled);
        let result = check_single_task_dependencies(&task, &statuses);
        match result {
            DependencyCheckResult::Blocked { unmet } => {
                assert!(unmet[0].contains("已取消"));
            }
            _ => panic!("Expected Blocked, got {:?}", result),
        }
    }
}
