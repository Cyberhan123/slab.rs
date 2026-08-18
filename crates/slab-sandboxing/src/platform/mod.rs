use std::sync::Arc;

use crate::{SandboxDriver, SandboxEnvironment, SandboxError};

mod linux;
mod macos;
mod windows;

pub use linux::LinuxSandboxDriver;
pub use macos::MacosSandboxDriver;
pub use windows::WindowsSandboxDriver;

pub fn create_platform_driver(
    env: SandboxEnvironment,
) -> Result<Arc<dyn SandboxDriver>, SandboxError> {
    #[cfg(target_os = "windows")]
    {
        let driver = WindowsSandboxDriver::new(env);
        // Drive the one-time elevation + ACL provisioning BEFORE erasing to `dyn`. This is the
        // "enable triggers one UAC" point. On success `setup_status()` flips to ready (gate
        // unblocks); on failure (decline/timeout/ACL denied) the driver stays degraded and the
        // fail-closed gate blocks the shell. The daemon dies with slab-server (owner-PID
        // watchdog), so each server start pays one UAC.
        if let Err(error) = driver.prepare() {
            tracing::warn!(
                %error,
                "Windows sandbox prepare() failed; driver stays degraded (fail-closed)"
            );
        }
        Ok(Arc::new(driver))
    }

    #[cfg(target_os = "linux")]
    {
        Ok(Arc::new(LinuxSandboxDriver::new(env)))
    }

    #[cfg(target_os = "macos")]
    {
        Ok(Arc::new(MacosSandboxDriver::new(env)))
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        let _ = env;
        Err(SandboxError::UnsupportedPlatform)
    }
}
