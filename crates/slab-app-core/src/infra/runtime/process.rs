use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::process::{Child, ChildStdin, Command as TokioCommand};
use tracing::{info, warn};

use crate::error::AppCoreError;
use crate::launch::ResolvedRuntimeChildSpec;

use super::supervisor::{RuntimeChildExit, RuntimeChildHandle, RuntimeChildSpawner};

const RUNTIME_CHILD_READY_TIMEOUT: Duration = Duration::from_secs(5);
const RUNTIME_CHILD_READY_POLL_INTERVAL: Duration = Duration::from_millis(100);
const RUNTIME_CHILD_READY_CONNECT_TIMEOUT: Duration = Duration::from_millis(250);
const RUNTIME_CHILD_FAILED_START_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);

pub struct TokioRuntimeSpawner {
    runtime_exe: PathBuf,
    log_level: Option<String>,
    log_json: bool,
}

impl TokioRuntimeSpawner {
    pub fn new(runtime_exe: PathBuf, log_level: Option<String>, log_json: bool) -> Self {
        Self { runtime_exe, log_level, log_json }
    }
}

pub fn resolve_runtime_exe(server_exe: &Path) -> Result<PathBuf, AppCoreError> {
    let parent = server_exe.parent().ok_or_else(|| {
        AppCoreError::Internal("failed to resolve server executable parent directory".to_owned())
    })?;
    let server_name = server_exe.file_name().and_then(|name| name.to_str()).ok_or_else(|| {
        AppCoreError::Internal("server executable name is not valid UTF-8".to_owned())
    })?;
    let ext = if cfg!(windows) { ".exe" } else { "" };

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(rest) = server_name.strip_prefix("slab-server-") {
        candidates.push(parent.join(format!("slab-runtime-{rest}")));
    }
    candidates.push(parent.join(format!("slab-runtime{ext}")));

    if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if cfg!(windows) {
                if name.starts_with("slab-runtime-") && name.ends_with(".exe") {
                    candidates.push(path);
                }
            } else if name.starts_with("slab-runtime-") {
                candidates.push(path);
            }
        }
    }

    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(AppCoreError::Internal(format!(
        "slab-runtime executable not found near {}. Build and bundle slab-runtime sidecar first.",
        server_exe.display()
    )))
}

async fn runtime_endpoint_ready(
    child_spec: &ResolvedRuntimeChildSpec,
) -> Result<bool, AppCoreError> {
    match child_spec.transport {
        slab_types::settings::RuntimeTransportMode::Http => {
            let target = normalize_http_probe_target(&child_spec.grpc_bind_address)?;
            match tokio::time::timeout(
                RUNTIME_CHILD_READY_CONNECT_TIMEOUT,
                tokio::net::TcpStream::connect(target.as_str()),
            )
            .await
            {
                Ok(Ok(stream)) => {
                    drop(stream);
                    Ok(true)
                }
                Ok(Err(_)) | Err(_) => Ok(false),
            }
        }
        slab_types::settings::RuntimeTransportMode::Ipc => {
            let target = normalize_ipc_probe_target(&child_spec.grpc_bind_address)?;
            match tokio::time::timeout(
                RUNTIME_CHILD_READY_CONNECT_TIMEOUT,
                parity_tokio_ipc::Endpoint::connect(target),
            )
            .await
            {
                Ok(Ok(stream)) => {
                    drop(stream);
                    Ok(true)
                }
                Ok(Err(_)) | Err(_) => Ok(false),
            }
        }
    }
}

fn normalize_http_probe_target(bind_address: &str) -> Result<String, AppCoreError> {
    let trimmed = bind_address.trim();
    if trimmed.is_empty() {
        return Err(AppCoreError::Internal("runtime child HTTP bind address is empty".to_owned()));
    }

    let without_scheme = trimmed
        .strip_prefix("http://")
        .or_else(|| trimmed.strip_prefix("https://"))
        .unwrap_or(trimmed);
    let authority = without_scheme.split('/').next().unwrap_or("").trim();
    if authority.is_empty() {
        return Err(AppCoreError::Internal(format!(
            "runtime child HTTP bind address '{}' is invalid",
            bind_address
        )));
    }

    Ok(authority.to_owned())
}

fn normalize_ipc_probe_target(bind_address: &str) -> Result<String, AppCoreError> {
    let trimmed = bind_address.trim();
    if trimmed.is_empty() {
        return Err(AppCoreError::Internal("runtime child IPC bind address is empty".to_owned()));
    }

    let path = trimmed.strip_prefix("ipc://").unwrap_or(trimmed).trim();
    if path.is_empty() {
        return Err(AppCoreError::Internal(format!(
            "runtime child IPC bind address '{}' is invalid",
            bind_address
        )));
    }

    Ok(path.to_owned())
}

async fn terminate_failed_start_child(child_spec: &ResolvedRuntimeChildSpec, child: &mut Child) {
    let backend = child_spec.backend.canonical_id();
    let bind_address = &child_spec.grpc_bind_address;

    match child.try_wait() {
        Ok(Some(status)) => {
            warn!(
                backend,
                bind_address = %bind_address,
                log_file = %child_spec.log_file.display(),
                exit = %status,
                "runtime child already exited during startup failure cleanup"
            );
            return;
        }
        Ok(None) => {}
        Err(error) => {
            warn!(
                backend,
                bind_address = %bind_address,
                log_file = %child_spec.log_file.display(),
                error = %error,
                "failed to inspect runtime child during startup failure cleanup"
            );
            return;
        }
    }

    if let Err(error) = child.start_kill() {
        warn!(
            backend,
            bind_address = %bind_address,
            log_file = %child_spec.log_file.display(),
            error = %error,
            "failed to kill runtime child after startup failure"
        );
        return;
    }

    match tokio::time::timeout(RUNTIME_CHILD_FAILED_START_SHUTDOWN_TIMEOUT, child.wait()).await {
        Ok(Ok(status)) => {
            warn!(
                backend,
                bind_address = %bind_address,
                log_file = %child_spec.log_file.display(),
                exit = %status,
                "runtime child exited after startup failure cleanup"
            );
        }
        Ok(Err(error)) => {
            warn!(
                backend,
                bind_address = %bind_address,
                log_file = %child_spec.log_file.display(),
                error = %error,
                "failed while waiting for runtime child cleanup after startup failure"
            );
        }
        Err(_) => {
            warn!(
                backend,
                bind_address = %bind_address,
                log_file = %child_spec.log_file.display(),
                timeout_ms = RUNTIME_CHILD_FAILED_START_SHUTDOWN_TIMEOUT.as_millis(),
                "timed out waiting for runtime child cleanup after startup failure"
            );
        }
    }
}

async fn wait_for_runtime_child_ready(
    child_spec: &ResolvedRuntimeChildSpec,
    child: &mut Child,
) -> Result<(), AppCoreError> {
    let started_at = tokio::time::Instant::now();

    loop {
        let endpoint_ready = match runtime_endpoint_ready(child_spec).await {
            Ok(ready) => ready,
            Err(error) => {
                terminate_failed_start_child(child_spec, child).await;
                return Err(error);
            }
        };
        if endpoint_ready {
            return Ok(());
        }

        match child.try_wait().map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to inspect runtime child '{}' startup state: {error}",
                child_spec.backend.canonical_id()
            ))
        })? {
            Some(status) => {
                return Err(AppCoreError::Internal(format!(
                    "runtime child '{}' exited before gRPC endpoint '{}' became ready: {status}",
                    child_spec.backend.canonical_id(),
                    child_spec.grpc_bind_address
                )));
            }
            None => {}
        }

        if started_at.elapsed() >= RUNTIME_CHILD_READY_TIMEOUT {
            terminate_failed_start_child(child_spec, child).await;
            return Err(AppCoreError::Internal(format!(
                "runtime child '{}' did not expose gRPC endpoint '{}' within {} ms",
                child_spec.backend.canonical_id(),
                child_spec.grpc_bind_address,
                RUNTIME_CHILD_READY_TIMEOUT.as_millis()
            )));
        }

        tokio::time::sleep(RUNTIME_CHILD_READY_POLL_INTERVAL).await;
    }
}

struct TokioRuntimeChildHandle {
    backend: String,
    bind_address: String,
    child: Child,
    stdin: Option<ChildStdin>,
}

#[async_trait]
impl RuntimeChildHandle for TokioRuntimeChildHandle {
    async fn wait_for_exit(&mut self) -> Result<RuntimeChildExit, AppCoreError> {
        let status = self.child.wait().await.map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to wait for runtime child '{}': {error}",
                self.backend
            ))
        })?;
        Ok(RuntimeChildExit {
            code: status.code(),
            signal: None,
            message: (!status.success()).then(|| format!("process exited with status {status}")),
        })
    }

    async fn request_graceful_shutdown(&mut self) -> Result<(), AppCoreError> {
        if self.stdin.take().is_some() {
            info!(
                backend = %self.backend,
                bind_address = %self.bind_address,
                "requested child graceful shutdown via stdin close"
            );
        } else {
            warn!(
                backend = %self.backend,
                bind_address = %self.bind_address,
                "child stdin handle missing; graceful shutdown may already be in progress"
            );
        }
        Ok(())
    }

    async fn force_kill(&mut self) -> Result<(), AppCoreError> {
        self.child.start_kill().map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to signal child force kill '{}': {error}",
                self.backend
            ))
        })
    }
}

#[async_trait]
impl RuntimeChildSpawner for TokioRuntimeSpawner {
    async fn spawn_child(
        &self,
        child_spec: &ResolvedRuntimeChildSpec,
    ) -> Result<Box<dyn RuntimeChildHandle>, AppCoreError> {
        let mut cmd = TokioCommand::new(&self.runtime_exe);
        cmd.args(child_spec.command_args(self.log_level.as_deref(), self.log_json))
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .stdin(Stdio::piped());

        let mut child = cmd.spawn().map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to spawn slab-runtime child '{}' from {}: {error}",
                child_spec.backend.canonical_id(),
                self.runtime_exe.display()
            ))
        })?;
        let stdin = child.stdin.take();
        info!(
            backend = child_spec.backend.canonical_id(),
            bind_address = %child_spec.grpc_bind_address,
            pid = ?child.id(),
            log_file = %child_spec.log_file.display(),
            "spawned backend child process"
        );
        wait_for_runtime_child_ready(child_spec, &mut child).await?;
        info!(
            backend = child_spec.backend.canonical_id(),
            bind_address = %child_spec.grpc_bind_address,
            pid = ?child.id(),
            log_file = %child_spec.log_file.display(),
            timeout_ms = RUNTIME_CHILD_READY_TIMEOUT.as_millis(),
            "runtime child gRPC endpoint became ready"
        );

        Ok(Box::new(TokioRuntimeChildHandle {
            backend: child_spec.backend.canonical_id().to_owned(),
            bind_address: child_spec.grpc_bind_address.clone(),
            child,
            stdin,
        }))
    }
}
