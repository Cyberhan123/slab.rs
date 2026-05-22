use thiserror::Error;

/// Error type for settings, host config, and launch resolution.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("not implemented: {0}")]
    NotImplemented(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for ConfigError {
    fn from(error: anyhow::Error) -> Self {
        Self::Internal(error.to_string())
    }
}
