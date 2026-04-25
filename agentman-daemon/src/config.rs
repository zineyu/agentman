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
    #[error("Invalid config value for '{field}': {reason}")]
    InvalidValue { field: String, reason: String },
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub runtime_id: String,
    pub runtime_name: String,
    pub base_url: String,
    pub base_token: String,
    pub app_id: String,
    pub app_secret: String,
    pub poll_interval_secs: u64,
    pub heartbeat_interval_secs: u64,
    pub max_concurrent_tasks: usize,
    pub workspace_dir: String,
    pub log_level: String,
    pub language: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            runtime_id: generate_stable_runtime_id(),
            runtime_name: "Agentman Daemon".to_string(),
            base_url: "https://open.feishu.cn".to_string(),
            base_token: String::new(),
            app_id: String::new(),
            app_secret: String::new(),
            poll_interval_secs: 30,
            heartbeat_interval_secs: 60,
            max_concurrent_tasks: 3,
            workspace_dir: "./workspace".to_string(),
            log_level: "info".to_string(),
            language: "en".to_string(),
        }
    }
}

impl std::fmt::Debug for DaemonConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DaemonConfig")
            .field("runtime_id", &self.runtime_id)
            .field("runtime_name", &self.runtime_name)
            .field("base_url", &self.base_url)
            .field("base_token", &"***REDACTED***")
            .field("app_id", &self.app_id)
            .field("app_secret", &"***REDACTED***")
            .field("poll_interval_secs", &self.poll_interval_secs)
            .field("heartbeat_interval_secs", &self.heartbeat_interval_secs)
            .field("max_concurrent_tasks", &self.max_concurrent_tasks)
            .field("workspace_dir", &self.workspace_dir)
            .field("log_level", &self.log_level)
            .field("language", &self.language)
            .finish()
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
        if self.runtime_id.is_empty() {
            return Err(ConfigError::MissingField("runtime_id".to_string()));
        }
        if self.runtime_name.is_empty() {
            return Err(ConfigError::MissingField("runtime_name".to_string()));
        }
        if self.base_token.is_empty() {
            return Err(ConfigError::MissingField("base_token".to_string()));
        }
        if self.app_id.is_empty() {
            return Err(ConfigError::MissingField("app_id".to_string()));
        }
        if self.app_secret.is_empty() {
            return Err(ConfigError::MissingField("app_secret".to_string()));
        }
        if self.workspace_dir.is_empty() {
            return Err(ConfigError::MissingField("workspace_dir".to_string()));
        }

        if self.poll_interval_secs == 0 {
            return Err(ConfigError::InvalidValue {
                field: "poll_interval_secs".to_string(),
                reason: "must be greater than 0".to_string(),
            });
        }
        if self.heartbeat_interval_secs == 0 {
            return Err(ConfigError::InvalidValue {
                field: "heartbeat_interval_secs".to_string(),
                reason: "must be greater than 0".to_string(),
            });
        }
        if self.max_concurrent_tasks == 0 {
            return Err(ConfigError::InvalidValue {
                field: "max_concurrent_tasks".to_string(),
                reason: "must be greater than 0".to_string(),
            });
        }

        if !self.base_url.starts_with("http://") && !self.base_url.starts_with("https://") {
            return Err(ConfigError::InvalidValue {
                field: "base_url".to_string(),
                reason: "must start with http:// or https://".to_string(),
            });
        }

        Ok(())
    }

    /// Set the locale based on the configured language
    pub fn set_locale(&self) {
        let locale = match self.language.as_str() {
            "zh" | "zh-CN" | "zh-Hans" => "zh",
            _ => "en",
        };
        rust_i18n::set_locale(locale);
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
        assert_eq!(config.language, "en");
    }

    #[test]
    fn test_config_validation_success() {
        let config = DaemonConfig {
            base_token: "token123".to_string(),
            app_id: "app123".to_string(),
            app_secret: "secret123".to_string(),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_missing_base_token() {
        let config = DaemonConfig {
            app_id: "app123".to_string(),
            app_secret: "secret123".to_string(),
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("base_token"));
    }

    #[test]
    fn test_config_validation_missing_app_id() {
        let config = DaemonConfig {
            base_token: "token123".to_string(),
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
            base_token: "token123".to_string(),
            app_id: "app123".to_string(),
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("app_secret"));
    }

    #[test]
    fn test_config_validation_invalid_poll_interval() {
        let config = DaemonConfig {
            base_token: "token123".to_string(),
            app_id: "app123".to_string(),
            app_secret: "secret123".to_string(),
            poll_interval_secs: 0,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("poll_interval_secs"));
    }

    #[test]
    fn test_config_validation_invalid_heartbeat_interval() {
        let config = DaemonConfig {
            base_token: "token123".to_string(),
            app_id: "app123".to_string(),
            app_secret: "secret123".to_string(),
            heartbeat_interval_secs: 0,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("heartbeat_interval_secs"));
    }

    #[test]
    fn test_config_validation_invalid_max_concurrent_tasks() {
        let config = DaemonConfig {
            base_token: "token123".to_string(),
            app_id: "app123".to_string(),
            app_secret: "secret123".to_string(),
            max_concurrent_tasks: 0,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("max_concurrent_tasks"));
    }

    #[test]
    fn test_config_validation_invalid_base_url() {
        let config = DaemonConfig {
            base_token: "token123".to_string(),
            app_id: "app123".to_string(),
            app_secret: "secret123".to_string(),
            base_url: "invalid-url".to_string(),
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("base_url"));
    }
}
