#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    #[error("empty command")]
    EmptyCommand,
    #[error("spawn failed: {0}")]
    SpawnFailed(String),
    #[error("command timed out")]
    Timeout,
    #[error("sandbox setup failed: {0}")]
    SetupFailed(String),
    #[error("bwrap not available: {0}")]
    BwrapNotAvailable(String),
    #[error("sandbox not supported on this platform")]
    UnsupportedPlatform,
    #[error("permission denied: {0}")]
    PermissionDenied(String),
}
