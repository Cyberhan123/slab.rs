use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command as TokioCommand;
use tokio::sync::mpsc;
use tokio::task::AbortHandle;

use crate::error::AppCoreError;

type LineHandler = Arc<
    dyn Fn(String, SupervisedStdioProcess) -> Pin<Box<dyn Future<Output = ()> + Send>>
        + Send
        + Sync,
>;
type ExitHandler =
    Arc<dyn Fn(SupervisedProcessExit) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

#[derive(Debug, Clone)]
pub(crate) struct SupervisedProcessExit {
    pub status: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct SupervisedStdioProcessConfig {
    pub label: String,
    pub executable: PathBuf,
    pub arguments: Vec<String>,
}

#[derive(Clone)]
pub(crate) struct SupervisedStdioProcess {
    inner: Arc<SupervisedStdioProcessInner>,
}

struct SupervisedStdioProcessInner {
    label: Arc<str>,
    stdin: mpsc::UnboundedSender<String>,
    alive: Arc<AtomicBool>,
    tasks: Mutex<Vec<AbortHandle>>,
}

impl Drop for SupervisedStdioProcessInner {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::SeqCst);
        for task in self.tasks.lock().expect("lock process task handles").drain(..) {
            task.abort();
        }
        tracing::debug!(process = %self.label, "supervised process handles dropped");
    }
}

impl SupervisedStdioProcess {
    pub async fn spawn(
        config: SupervisedStdioProcessConfig,
        stdout_handler: LineHandler,
        exit_handler: ExitHandler,
    ) -> Result<Self, AppCoreError> {
        let mut command = TokioCommand::new(&config.executable);
        command.args(&config.arguments);
        command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        command.kill_on_drop(true);
        let mut process = command.spawn().map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to spawn {} from {}: {error}",
                config.label,
                config.executable.display()
            ))
        })?;

        let stdin = process.stdin.take().ok_or_else(|| {
            AppCoreError::Internal(format!("failed to capture {} stdin", config.label))
        })?;
        let stdout = process.stdout.take().ok_or_else(|| {
            AppCoreError::Internal(format!("failed to capture {} stdout", config.label))
        })?;
        let stderr = process.stderr.take();
        let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<String>();
        let alive = Arc::new(AtomicBool::new(true));
        let inner = Arc::new(SupervisedStdioProcessInner {
            label: Arc::from(config.label.as_str()),
            stdin: stdin_tx,
            alive: Arc::clone(&alive),
            tasks: Mutex::new(Vec::new()),
        });
        let handle = Self { inner: Arc::clone(&inner) };
        let mut task_handles = Vec::new();

        let writer_alive = Arc::clone(&alive);
        let writer_label = config.label.clone();
        let writer_task = tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(line) = stdin_rx.recv().await {
                if stdin.write_all(line.as_bytes()).await.is_err() {
                    writer_alive.store(false, Ordering::SeqCst);
                    break;
                }
                if stdin.write_all(b"\n").await.is_err() {
                    writer_alive.store(false, Ordering::SeqCst);
                    break;
                }
                if stdin.flush().await.is_err() {
                    writer_alive.store(false, Ordering::SeqCst);
                    break;
                }
            }
            tracing::debug!(process = %writer_label, "supervised process stdin writer ended");
        });
        task_handles.push(writer_task.abort_handle());

        if let Some(stderr) = stderr {
            let stderr_label = config.label.clone();
            let stderr_task = tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::warn!(process = %stderr_label, "{line}");
                }
            });
            task_handles.push(stderr_task.abort_handle());
        }

        let stdout_alive = Arc::clone(&alive);
        let stdout_process: Weak<SupervisedStdioProcessInner> = Arc::downgrade(&inner);
        let stdout_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let Some(inner) = stdout_process.upgrade() else {
                    break;
                };
                stdout_handler(line, SupervisedStdioProcess { inner }).await;
            }
            stdout_alive.store(false, Ordering::SeqCst);
        });
        task_handles.push(stdout_task.abort_handle());

        let wait_alive = Arc::clone(&alive);
        let wait_label = config.label;
        let wait_task = tokio::spawn(async move {
            let exit = match process.wait().await {
                Ok(status) => {
                    tracing::warn!(process = %wait_label, %status, "supervised process exited");
                    SupervisedProcessExit { status: Some(status.to_string()), error: None }
                }
                Err(error) => {
                    tracing::warn!(
                        process = %wait_label,
                        %error,
                        "failed to wait for supervised process"
                    );
                    SupervisedProcessExit { status: None, error: Some(error.to_string()) }
                }
            };
            wait_alive.store(false, Ordering::SeqCst);
            exit_handler(exit).await;
        });
        task_handles.push(wait_task.abort_handle());

        inner.tasks.lock().expect("lock process task handles").extend(task_handles);

        Ok(handle)
    }

    pub fn is_alive(&self) -> bool {
        self.inner.alive.load(Ordering::SeqCst)
    }

    pub fn send_line(&self, line: String) -> Result<(), AppCoreError> {
        if !self.is_alive() {
            return Err(AppCoreError::Internal(format!("{} is not running", self.inner.label)));
        }
        self.inner
            .stdin
            .send(line)
            .map_err(|_| AppCoreError::Internal(format!("{} stdin is closed", self.inner.label)))
    }
}

pub(crate) fn resolve_sibling_sidecar_exe(
    server_exe: &Path,
    binary_name: &str,
) -> Result<PathBuf, AppCoreError> {
    let parent = server_exe.parent().ok_or_else(|| {
        AppCoreError::Internal("failed to resolve server executable parent directory".to_owned())
    })?;
    let server_name = server_exe.file_name().and_then(|name| name.to_str()).ok_or_else(|| {
        AppCoreError::Internal("server executable name is not valid UTF-8".to_owned())
    })?;
    let ext = if cfg!(windows) { ".exe" } else { "" };

    let mut candidates = Vec::new();
    if let Some(rest) = server_name.strip_prefix("slab-server-") {
        candidates.push(parent.join(format!("{binary_name}-{rest}")));
    }
    candidates.push(parent.join(format!("{binary_name}{ext}")));

    if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if cfg!(windows) {
                if name.starts_with(&format!("{binary_name}-")) && name.ends_with(".exe") {
                    candidates.push(path);
                }
            } else if name.starts_with(&format!("{binary_name}-")) {
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
        "{binary_name} executable not found near {}. Build and bundle {binary_name} sidecar first.",
        server_exe.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::resolve_sibling_sidecar_exe;

    #[test]
    fn resolves_plain_sibling_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let ext = if cfg!(windows) { ".exe" } else { "" };
        let server = dir.path().join(format!("slab-server{ext}"));
        let sidecar = dir.path().join(format!("slab-js-runtime{ext}"));
        std::fs::write(&server, "").unwrap();
        std::fs::write(&sidecar, "").unwrap();

        let resolved = resolve_sibling_sidecar_exe(&server, "slab-js-runtime").unwrap();

        assert_eq!(resolved, sidecar);
    }

    #[test]
    fn resolves_hashed_sibling_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let ext = if cfg!(windows) { ".exe" } else { "" };
        let server = dir.path().join(format!("slab-server-aarch64{ext}"));
        let sidecar = dir.path().join(format!("slab-runtime-aarch64{ext}"));
        std::fs::write(&server, "").unwrap();
        std::fs::write(&sidecar, "").unwrap();

        let resolved = resolve_sibling_sidecar_exe(&server, "slab-runtime").unwrap();

        assert_eq!(resolved, sidecar);
    }
}
