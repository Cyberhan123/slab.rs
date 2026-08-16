//! Errors for the rollout engine.

use std::io;

use thiserror::Error;

/// All rollout operations fall through this enum.
#[derive(Debug, Error)]
pub enum RolloutError {
    /// An OS-level I/O failure (open / write / fsync / rename).
    #[error("rollout io error: {0}")]
    Io(#[from] io::Error),

    /// A JSON serialization failure (a rollout line could not be encoded).
    #[error("rollout serialize error: {0}")]
    Serialize(#[from] serde_json::Error),

    /// The recorder actor is no longer accepting commands (dropped / shut down).
    #[error("rollout recorder closed")]
    RecorderClosed,

    /// A read was attempted for a thread that has no `SessionMeta` header yet
    /// (the file does not exist or has not been opened).
    #[error("rollout thread {0} has no session meta (file not created)")]
    NoSessionMeta(String),
}

/// Convenience alias used throughout the crate.
pub type Result<T, E = RolloutError> = std::result::Result<T, E>;
