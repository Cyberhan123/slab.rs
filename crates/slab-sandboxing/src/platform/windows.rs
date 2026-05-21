use async_trait::async_trait;
use tracing::{debug, warn};

use crate::{
    SandboxCapabilities, SandboxDriver, SandboxEnvironment, SandboxError, SandboxIsolation,
    SandboxPlatform, SandboxSetupStatus, SandboxedCommand, SandboxedOutput,
    guard::validate_command,
};

pub struct WindowsSandboxDriver {
    env: SandboxEnvironment,
}

impl WindowsSandboxDriver {
    pub fn new(env: SandboxEnvironment) -> Self {
        Self { env }
    }
}

#[async_trait]
impl SandboxDriver for WindowsSandboxDriver {
    fn name(&self) -> &str {
        "windows-job-object"
    }

    async fn run(&self, cmd: SandboxedCommand) -> Result<SandboxedOutput, SandboxError> {
        #[cfg(not(target_os = "windows"))]
        {
            let _ = cmd;
            return Err(SandboxError::UnsupportedPlatform);
        }

        #[cfg(target_os = "windows")]
        {
            use std::process::Stdio;

            use crate::driver::{command_env, wait_for_child};

            validate_command(&self.env, &cmd)?;

            let program = cmd.argv.first().ok_or(SandboxError::EmptyCommand)?;
            let mut command = tokio::process::Command::new(program);
            command.args(&cmd.argv[1..]);
            for (key, value) in command_env(&self.env, &cmd) {
                command.env(key, value);
            }
            if let Some(ref cwd) = cmd.cwd {
                command.current_dir(cwd);
            }
            command.kill_on_drop(true);
            command.stdout(Stdio::piped());
            command.stderr(Stdio::piped());

            let spawned = command.spawn().map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
            let job = JobHandle::new()?;
            job.configure_kill_on_close()?;
            let process_handle = spawned.raw_handle().ok_or_else(|| {
                SandboxError::SetupFailed("spawned child has no process handle".to_string())
            })?;
            job.assign_process(process_handle as windows_sys::Win32::Foundation::HANDLE)?;

            debug!(pid = spawned.id(), "spawned process in Windows Job Object");
            let output = wait_for_child(spawned, cmd.timeout).await?;
            drop(job);
            Ok(output)
        }
    }

    async fn prepare(&self) -> Result<SandboxSetupStatus, SandboxError> {
        Ok(self.setup_status())
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            platform: SandboxPlatform::Windows,
            isolation: SandboxIsolation::Degraded,
            filesystem: true,
            network: true,
            process_cleanup: true,
            setup_required: self.env.permissions.platform.windows_setup_required,
        }
    }

    fn setup_status(&self) -> SandboxSetupStatus {
        #[cfg(target_os = "windows")]
        {
            if self.env.permissions.platform.windows_setup_required {
                SandboxSetupStatus::degraded(
                    "Windows elevated sandbox setup is required before full token, ACL, and firewall isolation.",
                )
            } else {
                SandboxSetupStatus::degraded(
                    "Windows Job Object cleanup and Slab policy guard are active; elevated token, ACL, and firewall setup has not been requested.",
                )
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            SandboxSetupStatus::unavailable("Windows sandbox is only available on Windows")
        }
    }
}

#[cfg(target_os = "windows")]
struct JobHandle(windows_sys::Win32::Foundation::HANDLE);

#[cfg(target_os = "windows")]
unsafe impl Send for JobHandle {}

#[cfg(target_os = "windows")]
impl JobHandle {
    fn new() -> Result<Self, SandboxError> {
        use windows_sys::Win32::System::JobObjects::CreateJobObjectW;

        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(SandboxError::SetupFailed(format!(
                "CreateJobObjectW failed: {}",
                std::io::Error::last_os_error()
            )));
        }
        Ok(Self(handle))
    }

    fn configure_kill_on_close(&self) -> Result<(), SandboxError> {
        use windows_sys::Win32::System::JobObjects::{
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JobObjectExtendedLimitInformation, SetInformationJobObject,
        };

        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = unsafe {
            SetInformationJobObject(
                self.0,
                JobObjectExtendedLimitInformation,
                &mut info as *mut _ as *mut _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if ok == 0 {
            return Err(SandboxError::SetupFailed(format!(
                "SetInformationJobObject failed: {}",
                std::io::Error::last_os_error()
            )));
        }
        Ok(())
    }

    fn assign_process(
        &self,
        process: windows_sys::Win32::Foundation::HANDLE,
    ) -> Result<(), SandboxError> {
        use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;

        let ok = unsafe { AssignProcessToJobObject(self.0, process) };
        if ok == 0 {
            let error = std::io::Error::last_os_error();
            warn!(%error, "failed to assign process to Windows Job Object");
            return Err(SandboxError::SetupFailed(format!(
                "AssignProcessToJobObject failed: {error}"
            )));
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
impl Drop for JobHandle {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
