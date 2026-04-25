use std::path::Path;
use std::pin::Pin;
use std::future::Future;

use crate::agent::{AgentAdapter, ExecutionResult, Result};
use crate::models::task::Task;

pub struct OpenclawAdapter;

impl OpenclawAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OpenclawAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentAdapter for OpenclawAdapter {
    fn execute<'a>(
        &'a self,
        _task: &'a Task,
        _workspace: &'a Path,
    ) -> Pin<Box<dyn Future<Output = Result<ExecutionResult>> + Send + 'a>> {
        Box::pin(async move {
            todo!("Openclaw API integration not yet implemented")
        })
    }

    fn name(&self) -> String {
        "OpenclawAdapter".to_string()
    }
}
