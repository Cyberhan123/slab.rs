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
        Ok(Arc::new(WindowsSandboxDriver::new(env)))
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
