use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(String),
    #[error("Failed to parse config: {0}")]
    ParseError(String),
    #[error("Missing required field: {0}")]
    MissingField(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub runtime_id: String,
    pub runtime_name: String,
    pub base_url: String,
    pub app_id: String,
    pub app_secret: String,
    pub poll_interval_secs: u64,
    pub heartbeat_interval_secs: u64,
    pub max_concurrent_tasks: usize,
    pub workspace_dir: String,
    pub log_level: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            runtime_id: generate_stable_runtime_id(),
            runtime_name: "Agentman Daemon".to_string(),
            base_url: "https://open.feishu.cn".to_string(),
            app_id: String::new(),
            app_secret: String::new(),
            poll_interval_secs: 30,
            heartbeat_interval_secs: 60,
            max_concurrent_tasks: 3,
            workspace_dir: "./workspace".to_string(),
            log_level: "info".to_string(),
        }
    }
}

impl DaemonConfig {
    pub fn load(path: Option<&str>) -> Result<Self, ConfigError> {
        let config_path = path.unwrap_or("config.toml");

        if !Path::new(config_path).exists() {
            let config = Self::default();
            let toml = toml::to_string_pretty(&config)
                .map_err(|e| ConfigError::ParseError(e.to_string()))?;
            std::fs::write(config_path, toml)
                .map_err(|e| ConfigError::ReadError(e.to_string()))?;
            return Ok(config);
        }

        let content = std::fs::read_to_string(config_path)
            .map_err(|e| ConfigError::ReadError(e.to_string()))?;

        let config: DaemonConfig = toml::from_str(&content)
            .map_err(|e| ConfigError::ParseError(e.to_string()))?;

        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.app_id.is_empty() {
            return Err(ConfigError::MissingField("app_id".to_string()));
        }
        if self.app_secret.is_empty() {
            return Err(ConfigError::MissingField("app_secret".to_string()));
        }
        Ok(())
    }
}

/// Generate a stable runtime ID based on hostname
/// This ensures the same machine always gets the same runtime ID across restarts
fn generate_stable_runtime_id() -> String {
    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string());
    
    format!("agentman-{}", hostname)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DaemonConfig::default();
        assert!(config.runtime_id.starts_with("agentman-"));
        assert_eq!(config.runtime_name, "Agentman Daemon");
        assert_eq!(config.base_url, "https://open.feishu.cn");
        assert!(config.app_id.is_empty());
        assert!(config.app_secret.is_empty());
        assert_eq!(config.poll_interval_secs, 30);
        assert_eq!(config.heartbeat_interval_secs, 60);
        assert_eq!(config.max_concurrent_tasks, 3);
        assert_eq!(config.workspace_dir, "./workspace");
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn test_config_validation_success() {
        let config = DaemonConfig {
            app_id: "app123".to_string(),
            app_secret: "secret123".to_string(),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_missing_app_id() {
        let config = DaemonConfig {
            app_secret: "secret123".to_string(),
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("app_id"));
    }

    #[test]
    fn test_config_validation_missing_app_secret() {
        let config = DaemonConfig {
            app_id: "app123".to_string(),
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("app_secret"));
    }
}
