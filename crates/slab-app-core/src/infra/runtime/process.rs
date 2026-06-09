use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::process::{Child, ChildStdin, Command as TokioCommand};
use tracing::{info, warn};

use crate::error::AppCoreError;
use crate::infra::endpoint::{http_probe_authority, normalize_ipc_endpoint_path};
use crate::infra::process_supervisor::resolve_sibling_sidecar_exe;
use crate::launch::ResolvedRuntimeChildSpec;

use super::supervisor::{RuntimeChildExit, RuntimeChildHandle, RuntimeChildSpawner};

// Backend startup can spend several seconds in GPU runtime discovery and shared library init
// before the gRPC socket is reachable.
const RUNTIME_CHILD_READY_TIMEOUT: Duration = Duration::from_secs(15);
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
    resolve_sibling_sidecar_exe(server_exe, "slab-runtime")
}

async fn runtime_endpoint_ready(
    child_spec: &ResolvedRuntimeChildSpec,
) -> Result<bool, AppCoreError> {
    match child_spec.transport {
        slab_config::RuntimeTransportMode::Http => {
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
        slab_config::RuntimeTransportMode::Ipc => {
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
    http_probe_authority(bind_address)
        .map_err(|error| invalid_runtime_bind_address("HTTP", bind_address, error))
}

fn normalize_ipc_probe_target(bind_address: &str) -> Result<String, AppCoreError> {
    normalize_ipc_endpoint_path(bind_address)
        .map_err(|error| invalid_runtime_bind_address("IPC", bind_address, error))
}

fn invalid_runtime_bind_address(
    transport: &str,
    bind_address: &str,
    error: anyhow::Error,
) -> AppCoreError {
    AppCoreError::Internal(format!(
        "runtime child {transport} bind address '{bind_address}' is invalid: {error}"
    ))
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

        if let Some(status) = child.try_wait().map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to inspect runtime child '{}' startup state: {error}",
                child_spec.backend.canonical_id()
            ))
        })? {
            return Err(AppCoreError::Internal(format!(
                "runtime child '{}' exited before gRPC endpoint '{}' became ready: {status}",
                child_spec.backend.canonical_id(),
                child_spec.grpc_bind_address
            )));
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

#[cfg(test)]
mod tests {
    use super::{runtime_endpoint_ready, wait_for_runtime_child_ready};
    use crate::launch::ResolvedRuntimeChildSpec;
    use slab_config::RuntimeTransportMode;
    use slab_types::RuntimeBackendId;
    use std::path::PathBuf;
    use std::process::Stdio;
    use tokio::process::Command as TokioCommand;

    fn test_child_spec(bind_address: String) -> ResolvedRuntimeChildSpec {
        ResolvedRuntimeChildSpec {
            backend: RuntimeBackendId::GgmlLlama,
            service_backends: Vec::new(),
            grpc_bind_address: bind_address,
            transport: RuntimeTransportMode::Http,
            queue_capacity: 64,
            backend_capacity: 4,
            lib_dir: None,
            log_level: None,
            log_json: Some(false),
            log_file: PathBuf::from("C:/runtime/logs/slab-runtime-test.log"),
            shutdown_on_stdin_close: true,
        }
    }

    #[tokio::test]
    async fn runtime_endpoint_ready_reports_listening_http_socket() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let spec = test_child_spec(listener.local_addr().unwrap().to_string());

        assert!(runtime_endpoint_ready(&spec).await.unwrap());
    }

    #[tokio::test]
    async fn wait_for_runtime_child_ready_fails_fast_when_child_exits() {
        let spec = test_child_spec("127.0.0.1:9".to_owned());
        let mut cmd = if cfg!(windows) {
            let mut cmd = TokioCommand::new("cmd");
            cmd.args(["/C", "exit 7"]);
            cmd
        } else {
            let mut cmd = TokioCommand::new("sh");
            cmd.args(["-lc", "exit 7"]);
            cmd
        };
        let mut child =
            cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();

        let error = wait_for_runtime_child_ready(&spec, &mut child).await.unwrap_err();
        let message = error.to_string();

        assert!(message.contains("exited before gRPC endpoint"));
    }
}
