use async_trait::async_trait;
use slab_sandboxing::{
    SandboxDriver, SandboxEnvironment, SandboxError, SandboxedCommand, SandboxedOutput,
};
#[cfg(target_os = "windows")]
use tracing::{debug, warn};
#[cfg(target_os = "windows")]
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    },
};

pub struct WindowsSandboxDriver {
    #[allow(dead_code)]
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
            let program = cmd.argv.first().ok_or(SandboxError::EmptyCommand)?;
            let mut command = tokio::process::Command::new(program);
            command.args(&cmd.argv[1..]);
            for (k, v) in &cmd.env {
                command.env(k, v);
            }
            if let Some(ref cwd) = cmd.cwd {
                command.current_dir(cwd);
            }

            command.kill_on_drop(true);

            let spawned = command.spawn().map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
            let job = JobHandle::new()?;
            job.configure_kill_on_close()?;
            let process_handle = spawned.raw_handle().ok_or_else(|| {
                SandboxError::SetupFailed("spawned child has no process handle".to_string())
            })?;
            job.assign_process(process_handle as HANDLE)?;

            debug!(pid = spawned.id(), "spawned process in Windows Job Object");

            let result = if let Some(timeout) = cmd.timeout {
                tokio::time::timeout(timeout, spawned.wait_with_output())
                    .await
                    .map_err(|_| SandboxError::Timeout)?
                    .map_err(|e| SandboxError::SpawnFailed(e.to_string()))?
            } else {
                spawned
                    .wait_with_output()
                    .await
                    .map_err(|e| SandboxError::SpawnFailed(e.to_string()))?
            };

            Ok(SandboxedOutput {
                exit_code: result.status.code().unwrap_or(-1),
                stdout: result.stdout,
                stderr: result.stderr,
                timed_out: false,
            })
        }
    }
}

#[cfg(target_os = "windows")]
struct JobHandle(HANDLE);

#[cfg(target_os = "windows")]
unsafe impl Send for JobHandle {}

#[cfg(target_os = "windows")]
impl JobHandle {
    fn new() -> Result<Self, SandboxError> {
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

    fn assign_process(&self, process: HANDLE) -> Result<(), SandboxError> {
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
            CloseHandle(self.0);
        }
    }
}
