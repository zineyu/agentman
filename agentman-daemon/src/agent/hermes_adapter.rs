use std::path::Path;
use std::pin::Pin;
use std::future::Future;

use anyhow::Result;

use crate::agent::{AgentAdapter, ExecutionResult};
use crate::models::task::Task;

pub struct HermesAdapter;

impl HermesAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HermesAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentAdapter for HermesAdapter {
    fn execute<'a>(
        &'a self,
        _task: &'a Task,
        _workspace: &'a Path,
    ) -> Pin<Box<dyn Future<Output = Result<ExecutionResult>> + Send + 'a>> {
        Box::pin(async move {
            todo!("Hermes API integration not yet implemented")
        })
    }

    fn name(&self) -> String {
        "HermesAdapter".to_string()
    }
}
