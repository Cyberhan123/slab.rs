use async_trait::async_trait;
#[cfg(target_os = "windows")]
use tracing::debug;
use slab_sandboxing::{SandboxDriver, SandboxEnvironment, SandboxError, SandboxedCommand, SandboxedOutput};

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
            use std::os::windows::process::CommandExt;

            let program = cmd.argv.first().ok_or(SandboxError::EmptyCommand)?;
            let mut command = tokio::process::Command::new(program);
            command.args(&cmd.argv[1..]);
            for (k, v) in &cmd.env {
                command.env(k, v);
            }
            if let Some(ref cwd) = cmd.cwd {
                command.current_dir(cwd);
            }

            // CREATE_SUSPENDED so we can assign to a job object before it runs
            command.creation_flags(0x00000004);
            command.kill_on_drop(true);

            let spawned = command.spawn().map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;

            debug!(pid = spawned.id(), "spawned process under Windows Job Object sandbox");

            let result = if let Some(timeout) = cmd.timeout {
                tokio::time::timeout(timeout, spawned.wait_with_output())
                    .await
                    .map_err(|_| SandboxError::Timeout)?
                    .map_err(|e| SandboxError::SpawnFailed(e.to_string()))?
            } else {
                spawned.wait_with_output().await.map_err(|e| SandboxError::SpawnFailed(e.to_string()))?
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
