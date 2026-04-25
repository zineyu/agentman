use crate::agent::{AgentAdapter, Result, cli_adapter::CommandLineAdapter, openclaw_adapter::OpenclawAdapter, hermes_adapter::HermesAdapter};
use crate::models::task::AgentType;

pub struct AgentFactory;

impl AgentFactory {
    pub fn create(agent_type: AgentType) -> Result<Box<dyn AgentAdapter>> {
        match agent_type {
            AgentType::ClaudeCode
            | AgentType::Codex
            | AgentType::Opencode
            | AgentType::Cursor
            | AgentType::Other => {
                let adapter = CommandLineAdapter::new(agent_type)?;
                Ok(Box::new(adapter))
            }
        }
    }

    pub fn create_openclaw() -> Box<dyn AgentAdapter> {
        Box::new(OpenclawAdapter::new())
    }

    pub fn create_hermes() -> Box<dyn AgentAdapter> {
        Box::new(HermesAdapter::new())
    }
}
