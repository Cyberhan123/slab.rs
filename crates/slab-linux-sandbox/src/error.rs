//! Errors for the Linux sandbox crate. Decoupled from `slab_sandboxing::SandboxError` — the thin
//! shim in `slab_sandboxing::platform::linux` maps these at the boundary.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LinuxSandboxError {
    #[error("empty command")]
    EmptyCommand,
    #[error("spawn failed: {0}")]
    SpawnFailed(String),
    #[error("sandbox setup failed: {0}")]
    SetupFailed(String),
    #[error("sandbox not supported on this platform")]
    UnsupportedPlatform,
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("bwrap not available: {0}")]
    BwrapNotAvailable(String),
    #[error("seccomp filter compile failed: {0}")]
    SeccompCompile(String),
    #[error("landlock unavailable: {0}")]
    LandlockUnavailable(String),
}
