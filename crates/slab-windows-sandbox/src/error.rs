//! Errors for the Windows sandbox crate. Decoupled from `slab_sandboxing::SandboxError` — the
//! thin shim in `slab_sandboxing::platform::windows` maps these at the boundary.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WindowsSandboxError {
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
    #[error("windows api call failed: {0}")]
    WindowsApi(String),
    #[error("ipc framing error: {0}")]
    Ipc(#[from] crate::ipc::FileFramingError),
    #[error("hmac verification failed")]
    HmacMismatch,
    #[error("failed to read/write the sandbox helper key: {0}")]
    KeyIo(String),
    #[error("failed to unseal the sandbox helper key (corrupt or different user)")]
    KeyUnsealFailed,
    #[error("elevation declined by user")]
    ElevationDeclined,
    #[error("elevation timed out")]
    ElevationTimeout,
    #[error("elevation failed: {0}")]
    ElevationFailed(String),
    #[error("helper returned non-zero exit ({0})")]
    HelperExit(i32),
    #[error("sandbox marker drifted; re-provisioning failed: {0}")]
    ProvisionDrift(String),
}

/// Wrap a `windows-sys` BOOL call: `0`/`false` ⇒ `Err(WindowsApi(...))` with a context label.
#[cfg(target_os = "windows")]
pub(crate) fn win32_ctx(result: i32, ctx: &str) -> Result<(), WindowsSandboxError> {
    if result == 0 {
        Err(WindowsSandboxError::WindowsApi(format!(
            "{ctx} failed: {}",
            std::io::Error::last_os_error()
        )))
    } else {
        Ok(())
    }
}
