use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("context template error: {0}")]
    Template(String),
    #[error("context io error at {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("context json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, ContextError>;

pub(crate) fn io_error(path: impl Into<PathBuf>, source: std::io::Error) -> ContextError {
    ContextError::Io { path: path.into(), source }
}
