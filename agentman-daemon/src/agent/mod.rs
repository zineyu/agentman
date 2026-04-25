use std::path::Path;
use std::pin::Pin;
use std::future::Future;

use anyhow::Result;

use crate::models::task::Task;

pub mod cli_adapter;
pub mod factory;
pub mod hermes_adapter;
pub mod openclaw_adapter;

/// 执行结果
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// 是否成功
    pub success: bool,
    /// 执行输出
    pub output: String,
    /// 错误信息
    pub error_info: Option<String>,
    /// Git commit hash
    pub commit_hash: Option<String>,
}

impl ExecutionResult {
    /// 创建成功结果
    pub fn success(output: String) -> Self {
        Self {
            success: true,
            output,
            error_info: None,
            commit_hash: None,
        }
    }

    /// 创建失败结果
    pub fn failed(output: String, error: String) -> Self {
        Self {
            success: false,
            output,
            error_info: Some(error),
            commit_hash: None,
        }
    }

    /// 创建超时结果
    pub fn timeout(output: String) -> Self {
        Self {
            success: false,
            output,
            error_info: Some("Execution timed out".to_string()),
            commit_hash: None,
        }
    }
}

pub trait AgentAdapter: Send + Sync {
    fn execute<'a>(
        &'a self,
        task: &'a Task,
        workspace: &'a Path,
    ) -> Pin<Box<dyn Future<Output = Result<ExecutionResult>> + Send + 'a>>;

    fn execute_with_stream<'a>(
        &'a self,
        task: &'a Task,
        workspace: &'a Path,
        mut on_output: Box<dyn FnMut(&str) + Send + 'a>,
    ) -> Pin<Box<dyn Future<Output = Result<ExecutionResult>> + Send + 'a>> {
        Box::pin(async move {
            let result = self.execute(task, workspace).await?;
            on_output(&result.output);
            Ok(result)
        })
    }

    fn name(&self) -> String;
}

#[cfg(test)]
mod tests;
