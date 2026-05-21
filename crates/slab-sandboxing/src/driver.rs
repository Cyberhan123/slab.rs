use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;

use crate::error::SandboxError;
use crate::policy::SandboxEnvironment;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxPlatform {
    Windows,
    Linux,
    Macos,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxIsolation {
    Full,
    Degraded,
    Passthrough,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxCapabilities {
    pub platform: SandboxPlatform,
    pub isolation: SandboxIsolation,
    pub filesystem: bool,
    pub network: bool,
    pub process_cleanup: bool,
    pub setup_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxSetupStatus {
    pub available: bool,
    pub prepared: bool,
    pub degraded: bool,
    pub details: String,
}

impl SandboxSetupStatus {
    pub fn ready(details: impl Into<String>) -> Self {
        Self { available: true, prepared: true, degraded: false, details: details.into() }
    }

    pub fn degraded(details: impl Into<String>) -> Self {
        Self { available: true, prepared: false, degraded: true, details: details.into() }
    }

    pub fn unavailable(details: impl Into<String>) -> Self {
        Self { available: false, prepared: false, degraded: false, details: details.into() }
    }
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

    async fn prepare(&self) -> Result<SandboxSetupStatus, SandboxError> {
        Ok(self.setup_status())
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            platform: SandboxPlatform::Unsupported,
            isolation: SandboxIsolation::Degraded,
            filesystem: false,
            network: false,
            process_cleanup: false,
            setup_required: false,
        }
    }

    fn setup_status(&self) -> SandboxSetupStatus {
        SandboxSetupStatus::ready(format!("{} is ready", self.name()))
    }
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
        child.stdout(std::process::Stdio::piped());
        child.stderr(std::process::Stdio::piped());

        let spawned = child.spawn().map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
        wait_for_child(spawned, cmd.timeout).await
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            platform: SandboxPlatform::Unsupported,
            isolation: SandboxIsolation::Passthrough,
            filesystem: false,
            network: false,
            process_cleanup: false,
            setup_required: false,
        }
    }
}

pub(crate) fn command_env(
    env: &SandboxEnvironment,
    cmd: &SandboxedCommand,
) -> HashMap<String, String> {
    let mut merged = cmd.env.clone();
    if let Some(proxy) = &env.permissions.managed_proxy {
        if let Some(http_proxy) = &proxy.http_proxy {
            merged.insert("HTTP_PROXY".to_string(), http_proxy.clone());
            merged.insert("http_proxy".to_string(), http_proxy.clone());
        }
        if let Some(https_proxy) = &proxy.https_proxy {
            merged.insert("HTTPS_PROXY".to_string(), https_proxy.clone());
            merged.insert("https_proxy".to_string(), https_proxy.clone());
        }
        if !proxy.no_proxy.is_empty() {
            let no_proxy = proxy.no_proxy.join(",");
            merged.insert("NO_PROXY".to_string(), no_proxy.clone());
            merged.insert("no_proxy".to_string(), no_proxy);
        }
    }
    merged
}

pub(crate) async fn wait_for_child(
    mut child: tokio::process::Child,
    timeout: Option<Duration>,
) -> Result<SandboxedOutput, SandboxError> {
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let stdout_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        if let Some(stdout) = stdout.as_mut() {
            stdout.read_to_end(&mut bytes).await?;
        }
        Ok::<_, std::io::Error>(bytes)
    });
    let stderr_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        if let Some(stderr) = stderr.as_mut() {
            stderr.read_to_end(&mut bytes).await?;
        }
        Ok::<_, std::io::Error>(bytes)
    });

    let (exit_code, timed_out) = if let Some(timeout) = timeout {
        match tokio::time::timeout(timeout, child.wait()).await {
            Ok(Ok(status)) => (status.code().unwrap_or(-1), false),
            Ok(Err(error)) => return Err(SandboxError::SpawnFailed(error.to_string())),
            Err(_) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                (1, true)
            }
        }
    } else {
        let status = child.wait().await.map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
        (status.code().unwrap_or(-1), false)
    };

    let stdout = stdout_task
        .await
        .map_err(|e| SandboxError::SpawnFailed(e.to_string()))?
        .map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
    let mut stderr = stderr_task
        .await
        .map_err(|e| SandboxError::SpawnFailed(e.to_string()))?
        .map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
    if timed_out && stderr.is_empty() {
        stderr.extend_from_slice(b"command timed out");
    }

    Ok(SandboxedOutput { stdout, stderr, exit_code, timed_out })
}
