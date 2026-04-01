use std::path::PathBuf;

use slab_types::RuntimeBackendId;
use tracing::{info, warn};

use crate::context::worker_state::OperationContext;
use crate::context::{ModelState, SubmitOperation, WorkerState};
use crate::domain::models::{
    AcceptedOperation, CompleteSetupCommand, ComponentStatus, EnvironmentStatus,
};
use crate::error::AppCoreError;

#[derive(Clone)]
pub struct SetupService {
    model_state: ModelState,
    worker_state: WorkerState,
}

impl SetupService {
    pub fn new(model_state: ModelState, worker_state: WorkerState) -> Self {
        Self { model_state, worker_state }
    }

    /// Return the current environment status: FFmpeg presence, backend
    /// availability, and whether the setup wizard has been completed.
    pub async fn environment_status(&self) -> Result<EnvironmentStatus, AppCoreError> {
        let initialized = self.model_state.pmid().config().setup.initialized;

        // `ffmpeg_is_installed` is a blocking check — run it off the async executor.
        let ffmpeg_installed =
            tokio::task::spawn_blocking(ffmpeg_sidecar::command::ffmpeg_is_installed)
                .await
                .unwrap_or(false);

        let ffmpeg_version = if ffmpeg_installed {
            tokio::task::spawn_blocking(|| ffmpeg_sidecar::version::ffmpeg_version().ok())
                .await
                .unwrap_or(None)
        } else {
            None
        };

        let backends: Vec<ComponentStatus> = RuntimeBackendId::ALL
            .into_iter()
            .map(|b| {
                let name = b.to_string();
                let available = self.model_state.grpc().has_backend(b);
                ComponentStatus { name, installed: available, version: None }
            })
            .collect();

        Ok(EnvironmentStatus {
            initialized,
            ffmpeg: ComponentStatus {
                name: "ffmpeg".to_owned(),
                installed: ffmpeg_installed,
                version: ffmpeg_version,
            },
            backends,
        })
    }

    /// Kick off an async FFmpeg download task and return immediately with an
    /// accepted-operation handle.  The caller should poll the task via the
    /// tasks API to track progress.
    pub async fn download_ffmpeg(&self) -> Result<AcceptedOperation, AppCoreError> {
        let ffmpeg_dir = self.model_state.pmid().config().setup.ffmpeg.dir;

        let operation_id = self
            .worker_state
            .submit_operation(
                SubmitOperation::pending("ffmpeg_download", None, None),
                move |operation| async move {
                    run_ffmpeg_download(operation, ffmpeg_dir).await;
                },
            )
            .await?;

        Ok(AcceptedOperation { operation_id })
    }

    /// Persist the `setup.initialized` flag (or reset it) in `settings.json`.
    pub async fn complete_setup(
        &self,
        cmd: CompleteSetupCommand,
    ) -> Result<EnvironmentStatus, AppCoreError> {
        self.model_state.pmid().set_setup_initialized(cmd.initialized).await?;

        info!(initialized = cmd.initialized, "setup state persisted");
        self.environment_status().await
    }
}

// ── FFmpeg download task ──────────────────────────────────────────────────────

async fn run_ffmpeg_download(operation: OperationContext, ffmpeg_dir: Option<String>) {
    let operation_id = operation.id().to_owned();

    if let Err(error) = operation.mark_running().await {
        warn!(task_id = %operation_id, error = %error, "failed to mark ffmpeg_download running");
        return;
    }

    // Resolve destination: use settings-configured dir or fall back to the
    // sidecar directory (next to the server executable).
    let destination: Option<PathBuf> =
        ffmpeg_dir.as_deref().filter(|s| !s.is_empty()).map(PathBuf::from);

    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<PathBuf> {
        // Skip if already installed (idempotent).
        if ffmpeg_sidecar::command::ffmpeg_is_installed() {
            return Ok(ffmpeg_sidecar::paths::ffmpeg_path());
        }

        let download_url = ffmpeg_sidecar::download::ffmpeg_download_url()?;

        let dest = match destination {
            Some(dir) => {
                std::fs::create_dir_all(&dir)?;
                dir
            }
            None => ffmpeg_sidecar::paths::sidecar_dir()?,
        };

        let archive = ffmpeg_sidecar::download::download_ffmpeg_package(download_url, &dest)?;
        ffmpeg_sidecar::download::unpack_ffmpeg(&archive, &dest)?;

        // Clean up the archive after unpacking.
        let _ = std::fs::remove_file(&archive);

        Ok(ffmpeg_sidecar::paths::ffmpeg_path())
    })
    .await;

    match result {
        Ok(Ok(path)) => {
            let result_json = serde_json::json!({ "path": path }).to_string();
            if let Err(db_err) = operation.mark_succeeded(&result_json).await {
                warn!(task_id = %operation_id, error = %db_err, "failed to persist ffmpeg_download success");
            }
            info!(task_id = %operation_id, "ffmpeg downloaded successfully");
        }
        Ok(Err(error)) => {
            let msg = error.to_string();
            warn!(task_id = %operation_id, error = %msg, "ffmpeg download failed");
            if let Err(db_err) = operation.mark_failed(&msg).await {
                warn!(task_id = %operation_id, error = %db_err, "failed to persist ffmpeg_download failure");
            }
        }
        Err(join_error) => {
            // JoinError can represent either a task panic or a task cancellation
            // (e.g., runtime shutdown). Distinguish between them for clearer diagnostics.
            let msg = if join_error.is_panic() {
                format!("ffmpeg download worker panicked: {join_error}")
            } else {
                format!("ffmpeg download worker was cancelled: {join_error}")
            };
            warn!(task_id = %operation_id, "{msg}");
            if let Err(db_err) = operation.mark_failed(&msg).await {
                warn!(task_id = %operation_id, error = %db_err, "failed to persist ffmpeg_download task error");
            }
        }
    }
}
