use serde::Deserialize;
use std::{fs, net::SocketAddr, path::Path};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub bind_address: SocketAddr,
    pub database_url: String,
    pub api_token: String,
}

impl Config {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.display().to_string(),
            source,
        })?;

        let mut config: Self = toml::from_str(&raw).map_err(|source| ConfigError::Parse {
            path: path.display().to_string(),
            source,
        })?;

        if config.api_token.trim().is_empty() {
            return Err(ConfigError::Validation(
                "api_token must not be empty".to_owned(),
            ));
        }
        config.api_token = config.api_token.trim().to_owned();
        Ok(config)
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Read {
        path: String,
        source: std::io::Error,
    },
    Parse {
        path: String,
        source: toml::de::Error,
    },
    Validation(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(f, "failed to read config file '{path}': {source}")
            }
            Self::Parse { path, source } => {
                write!(f, "failed to parse config file '{path}': {source}")
            }
            Self::Validation(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ConfigError {}
