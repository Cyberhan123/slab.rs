use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::SandboxError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxedCommand {
    pub argv: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: Option<PathBuf>,
    pub timeout: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct SandboxedOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
    pub timed_out: bool,
}

impl SandboxedOutput {
    pub fn stdout_str(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
    pub fn stderr_str(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

#[async_trait]
pub trait SandboxDriver: Send + Sync {
    async fn run(&self, cmd: SandboxedCommand) -> Result<SandboxedOutput, SandboxError>;
    fn name(&self) -> &str;
}

/// A pass-through sandbox driver that executes commands directly without isolation.
/// Use only in development/test environments or when DangerFullAccess policy is set.
pub struct PassThroughDriver;

#[async_trait]
impl SandboxDriver for PassThroughDriver {
    fn name(&self) -> &str {
        "passthrough"
    }

    async fn run(&self, cmd: SandboxedCommand) -> Result<SandboxedOutput, SandboxError> {
        use tokio::process::Command;
        let program = cmd.argv.first().ok_or(SandboxError::EmptyCommand)?;
        let mut child = Command::new(program);
        child.args(&cmd.argv[1..]);
        for (k, v) in &cmd.env {
            child.env(k, v);
        }
        if let Some(ref cwd) = cmd.cwd {
            child.current_dir(cwd);
        }
        child.kill_on_drop(true);

        let spawned = child.spawn().map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;

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
