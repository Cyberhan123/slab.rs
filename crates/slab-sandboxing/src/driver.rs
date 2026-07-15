use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::error::SandboxError;
use crate::policy::SandboxEnvironment;

/// Which process stream an output chunk came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputStream {
    Stdout,
    Stderr,
}

/// Receiver for incremental process output. While a command runs the driver
/// forwards each chunk (e.g. to the harness display); it still returns the fully
/// accumulated output when the process exits.
pub trait OutputSink: Send + Sync {
    fn on_output(&self, stream: OutputStream, delta: &str);
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SandboxedCommand {
    pub argv: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: Option<PathBuf>,
    pub timeout: Option<Duration>,
    /// Optional live-output receiver. `#[serde(skip)]` (closures aren't
    /// serializable); defaults to `None` when deserialized.
    #[serde(skip)]
    pub output_sink: Option<Arc<dyn OutputSink>>,
}

impl std::fmt::Debug for SandboxedCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SandboxedCommand")
            .field("argv", &self.argv)
            .field("env", &self.env)
            .field("cwd", &self.cwd)
            .field("timeout", &self.timeout)
            .field("output_sink", &self.output_sink.as_ref().map(|_| "<sink>"))
            .finish()
    }
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
        child.stdin(std::process::Stdio::null());
        child.stdout(std::process::Stdio::piped());
        child.stderr(std::process::Stdio::piped());

        let spawned = child.spawn().map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
        wait_for_child(spawned, cmd.timeout, cmd.output_sink.clone()).await
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
    sink: Option<Arc<dyn OutputSink>>,
) -> Result<SandboxedOutput, SandboxError> {
    let pid = child.id();
    tracing::info!(
        ?pid,
        ?timeout,
        has_sink = sink.is_some(),
        "wait_for_child: spawned, awaiting child.wait"
    );
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_sink = sink.clone();
    let stderr_sink = sink.clone();
    let stdout_task =
        tokio::spawn(async move { read_stream(stdout, stdout_sink, OutputStream::Stdout).await });
    let stderr_task =
        tokio::spawn(async move { read_stream(stderr, stderr_sink, OutputStream::Stderr).await });

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
    tracing::info!(?pid, exit_code, timed_out, "wait_for_child: child exited, awaiting read tasks");

    let stdout = stdout_task
        .await
        .map_err(|e| SandboxError::SpawnFailed(e.to_string()))?
        .map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
    tracing::info!(?pid, stdout_len = stdout.len(), "wait_for_child: stdout_task done");
    let mut stderr = stderr_task
        .await
        .map_err(|e| SandboxError::SpawnFailed(e.to_string()))?
        .map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
    tracing::info!(?pid, stderr_len = stderr.len(), "wait_for_child: stderr_task done, returning");
    if timed_out && stderr.is_empty() {
        stderr.extend_from_slice(b"command timed out");
    }

    Ok(SandboxedOutput { stdout, stderr, exit_code, timed_out })
}

/// Read a child pipe to EOF. When a sink is present, forward each chunk
/// incrementally (lossy UTF-8) AND accumulate the full buffer; otherwise just
/// `read_to_end`. The accumulated buffer is returned so the final output is
/// identical whether or not streaming was requested.
async fn read_stream<R: AsyncRead + Unpin>(
    reader: Option<R>,
    sink: Option<Arc<dyn OutputSink>>,
    stream: OutputStream,
) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let Some(mut reader) = reader else { return Ok(bytes) };
    match sink.as_ref() {
        Some(sink) => {
            let mut buf = [0u8; 8192];
            loop {
                let n = reader.read(&mut buf).await?;
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&buf[..n]);
                let delta = String::from_utf8_lossy(&buf[..n]);
                sink.on_output(stream, &delta);
            }
        }
        None => {
            reader.read_to_end(&mut bytes).await?;
        }
    }
    Ok(bytes)
}
