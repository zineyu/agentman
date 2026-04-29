use std::path::Path;
use std::pin::Pin;
use std::future::Future;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use crate::agent::{AgentAdapter, AgentError, ExecutionResult, Result};
use crate::models::task::{AgentType, Task};
use crate::utils::strip_ansi_codes;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// 命令行Agent适配器
pub struct CommandLineAdapter {
    agent_type: AgentType,
    cli_name: String,
}

impl CommandLineAdapter {
    /// 创建新的CLI适配器
    pub fn new(agent_type: AgentType) -> Result<Self> {
        let cli_name = Self::detect_cli(agent_type)?;
        Ok(Self {
            agent_type,
            cli_name,
        })
    }

    /// 检测指定类型的CLI是否在PATH中可用
    fn detect_cli(agent_type: AgentType) -> Result<String> {
        let cli_names = match agent_type {
            AgentType::ClaudeCode => vec!["claude", "claude-code"],
            AgentType::Codex => vec!["codex"],
            AgentType::Opencode => vec!["opencode"],
            AgentType::Cursor => vec!["cursor"],
            AgentType::Other => {
                return Err(AgentError::CannotAutoDetect(
                    format!("{:?}", agent_type)
                ));
            }
        };

        for name in &cli_names {
            if Self::is_in_path(name) {
                return Ok(name.to_string());
            }
        }

        Err(AgentError::CliNotFound {
            agent_type: format!("{:?}", agent_type),
            reason: format!("Tried: {:?}", cli_names),
        })
    }

    fn is_in_path(cmd: &str) -> bool {
        let path_var = std::env::var_os("PATH");
        if path_var.is_none() {
            return false;
        }

        let binding = path_var.unwrap();
        let paths = std::env::split_paths(&binding);
        let executable = if cfg!(windows) {
            format!("{}.exe", cmd)
        } else {
            cmd.to_string()
        };

        for path in paths {
            let full_path = path.join(&executable);
            if full_path.is_file() {
                return true;
            }
        }

        false
    }

    /// 构建命令行调用参数
    fn build_command(&self,
        task: &Task,
        workspace: &Path,
    ) -> Command {
        let mut cmd = Command::new(&self.cli_name);
        cmd.current_dir(workspace)
            .stdin(Stdio::null());

        let prompt = format!(
            "Task #{}: {}\n\n{}",
            task.id, task.title, task.description
        );

        match self.agent_type {
            AgentType::ClaudeCode => {
                cmd.arg("-p").arg(&prompt);
            }
            AgentType::Codex => {
                cmd.arg(&prompt);
            }
            AgentType::Opencode => {
                cmd.arg("run").arg(&prompt);
            }
            AgentType::Cursor => {
                cmd.arg("--prompt").arg(&prompt);
            }
            AgentType::Other => {
                cmd.arg(&prompt);
            }
        }

        cmd
    }
}

impl AgentAdapter for CommandLineAdapter {
    fn execute<'a>(
        &'a self,
        task: &'a Task,
        workspace: &'a Path,
    ) -> Pin<Box<dyn Future<Output = Result<ExecutionResult>> + Send + 'a>> {
        Box::pin(async move {
            info!(
                "{}",
                rust_i18n::t!(
                    "cli_adapter.executing_task",
                    id = task.id,
                    cli = self.name(),
                    workspace = workspace.display()
                )
            );

            let mut cmd = self.build_command(task, workspace);

            debug!("{}", rust_i18n::t!("cli_adapter.running_command", command = format!("{:?}", cmd)));

            let output = match timeout(DEFAULT_TIMEOUT, cmd.output()).await {
                Ok(Ok(output)) => output,
                Ok(Err(e)) => {
                    error!("{}", rust_i18n::t!("cli_adapter.failed_execute", cli = self.cli_name, error = e));
                    return Ok(ExecutionResult::failed(
                        String::new(),
                        format!("{}", rust_i18n::t!("cli_adapter.process_execution_failed", error = e)),
                    ));
                }
                Err(_) => {
                    warn!("{}", rust_i18n::t!("cli_adapter.task_timeout", id = task.id));
                    return Ok(ExecutionResult::timeout(String::new()));
                }
            };

            let stdout = strip_ansi_codes(&String::from_utf8_lossy(&output.stdout));
            let stderr = strip_ansi_codes(&String::from_utf8_lossy(&output.stderr));
            let combined_output = format!("{}{}", stdout, stderr);

            if output.status.success() {
                info!("{}", rust_i18n::t!("cli_adapter.task_executed_success", id = task.id, cli = self.name()));
                Ok(ExecutionResult::success(combined_output))
            } else {
                let exit_code = output.status.code().unwrap_or(-1);
                warn!(
                    "{}",
                    rust_i18n::t!("cli_adapter.task_failed_exit_code", id = task.id, code = exit_code, cli = self.name())
                );
                Ok(ExecutionResult::failed(
                    combined_output,
                    format!("{}", rust_i18n::t!("cli_adapter.process_exited", code = exit_code)),
                ))
            }
        })
    }

    fn execute_with_stream<'a>(
        &'a self,
        task: &'a Task,
        workspace: &'a Path,
        on_output: Box<dyn FnMut(&str) + Send + 'a>,
    ) -> Pin<Box<dyn Future<Output = Result<ExecutionResult>> + Send + 'a>> {
        Box::pin(async move {
            info!(
                "{}",
                rust_i18n::t!("cli_adapter.executing_task_streaming", id = task.id, cli = self.name(), workspace = workspace.display())
            );

            let mut cmd = self.build_command(task, workspace);
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            debug!("{}", rust_i18n::t!("cli_adapter.running_command", command = format!("{:?}", cmd)));

            let mut child = match cmd.spawn() {
                Ok(child) => child,
                Err(e) => {
                    error!("{}", rust_i18n::t!("cli_adapter.failed_spawn", cli = self.cli_name, error = e));
                    return Ok(ExecutionResult::failed(
                        String::new(),
                        format!("{}", rust_i18n::t!("cli_adapter.process_spawn_failed", error = e)),
                    ));
                }
            };

            let stdout = child.stdout.take().expect("stdout pipe");
            let stderr = child.stderr.take().expect("stderr pipe");

            let mut stdout_reader = BufReader::new(stdout).lines();
            let mut stderr_reader = BufReader::new(stderr).lines();

            let combined_output = Arc::new(Mutex::new(String::new()));
            let stdout_output = combined_output.clone();
            let stderr_output = combined_output.clone();

            let on_output = Arc::new(Mutex::new(on_output));
            let stdout_callback = on_output.clone();
            let stderr_callback = on_output.clone();

            let read_stdout = async move {
                while let Ok(Some(line)) = stdout_reader.next_line().await {
                    let clean = strip_ansi_codes(&line);
                    let line_with_nl = format!("{}\n", clean);
                    stdout_output.lock().await.push_str(&line_with_nl);
                    stdout_callback.lock().await(&clean);
                }
            };

            let read_stderr = async move {
                while let Ok(Some(line)) = stderr_reader.next_line().await {
                    let clean = strip_ansi_codes(&line);
                    let line_with_nl = format!("{}\n", clean);
                    stderr_output.lock().await.push_str(&line_with_nl);
                    stderr_callback.lock().await(&clean);
                }
            };

            let result = match timeout(DEFAULT_TIMEOUT, async {
                tokio::join!(read_stdout, read_stderr);
                child.wait().await
            })
            .await
            {
                Ok(Ok(status)) => status,
                Ok(Err(e)) => {
                    error!("{}", rust_i18n::t!("cli_adapter.failed_wait", cli = self.cli_name, error = e));
                    let output = combined_output.lock().await.clone();
                    return Ok(ExecutionResult::failed(
                        output,
                        format!("{}", rust_i18n::t!("cli_adapter.process_wait_failed", error = e)),
                    ));
                }
                Err(_) => {
                    warn!("{}", rust_i18n::t!("cli_adapter.task_timeout", id = task.id));
                    let _ = child.kill().await;
                    let output = combined_output.lock().await.clone();
                    return Ok(ExecutionResult::timeout(output));
                }
            };

            let output = combined_output.lock().await.clone();

            if result.success() {
                info!("{}", rust_i18n::t!("cli_adapter.task_executed_success", id = task.id, cli = self.name()));
                Ok(ExecutionResult::success(output))
            } else {
                let exit_code = result.code().unwrap_or(-1);
                warn!(
                    "{}",
                    rust_i18n::t!("cli_adapter.task_failed_exit_code", id = task.id, code = exit_code, cli = self.name())
                );
                Ok(ExecutionResult::failed(
                    output,
                    format!("{}", rust_i18n::t!("cli_adapter.process_exited", code = exit_code)),
                ))
            }
        })
    }

    fn name(&self) -> String {
        format!("CommandLineAdapter({:?})", self.agent_type)
    }
}
