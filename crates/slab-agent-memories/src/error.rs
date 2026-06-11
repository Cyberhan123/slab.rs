use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("failed to render memory template: {0}")]
    Template(String),
    #[error("invalid memory model output: {0}")]
    InvalidModelOutput(String),
    #[error("memory filesystem error at {path}: {source}")]
    Fs { path: PathBuf, source: std::io::Error },
    #[error("memory git command failed: {0}")]
    Git(String),
    #[error("memory json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, MemoryError>;

pub(crate) fn fs_error(path: impl Into<PathBuf>, source: std::io::Error) -> MemoryError {
    MemoryError::Fs { path: path.into(), source }
}
