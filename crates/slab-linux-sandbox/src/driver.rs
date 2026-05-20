use async_trait::async_trait;
use tracing::debug;
use slab_sandboxing::{SandboxDriver, SandboxEnvironment, SandboxError, SandboxedCommand, SandboxedOutput};

use crate::bwrap::{build_bwrap_args, find_bwrap};

pub struct LinuxSandboxDriver {
    env: SandboxEnvironment,
}

impl LinuxSandboxDriver {
    pub fn new(env: SandboxEnvironment) -> Self {
        Self { env }
    }

    pub fn available() -> bool {
        find_bwrap().is_some()
    }
}

#[async_trait]
impl SandboxDriver for LinuxSandboxDriver {
    fn name(&self) -> &str {
        "linux-bwrap"
    }

    async fn run(&self, cmd: SandboxedCommand) -> Result<SandboxedOutput, SandboxError> {
        #[cfg(not(target_os = "linux"))]
        return Err(SandboxError::UnsupportedPlatform);

        #[cfg(target_os = "linux")]
        {
            let bwrap = find_bwrap().ok_or_else(|| {
                SandboxError::BwrapNotAvailable("bwrap not found on PATH".into())
            })?;

            // PR_SET_NO_NEW_PRIVS is a process-wide setting: once set it applies
            // to this process and all children spawned from it.  This is intentional
            // — we want the sandboxed child to be unable to gain new privileges.
            let ret = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
            if ret != 0 {
                return Err(SandboxError::SetupFailed(format!(
                    "PR_SET_NO_NEW_PRIVS failed: {}",
                    std::io::Error::last_os_error()
                )));
            }

            let bwrap_args = build_bwrap_args(&self.env, &cmd)?;

            debug!(bwrap = ?bwrap, args = ?bwrap_args, "spawning bwrap sandbox");

            let mut command = tokio::process::Command::new(&bwrap);
            command.args(&bwrap_args);
            command.args(&cmd.argv);
            for (k, v) in &cmd.env {
                command.env(k, v);
            }
            if let Some(ref cwd) = cmd.cwd {
                command.current_dir(cwd);
            }
            command.kill_on_drop(true);

            let spawned = command.spawn().map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;

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
