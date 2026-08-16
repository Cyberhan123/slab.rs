//! Server-driven model loading with coarse progress streaming.
//!
//! [`ModelService::ensure_model_loaded_with_progress`] composes the existing
//! `download_model` + `load_model` service methods into a single ensure-loaded
//! operation that reports coarse progress over an `mpsc` channel. It backs the
//! `model/load/delta` + `model/load/completed` harness notifications emitted
//! from the `turn/start` handler.
//!
//! Progress fidelity (by design):
//! - **Download phase** — real byte-level progress, sourced by polling the task
//!   `result_data` that [`super::download_progress::ModelDownloadProgressReporter`]
//!   already writes to the store (throttled at ~500 ms / 512 KB). No changes to
//!   the download pipeline.
//! - **Load phase** — coarse only (`"loading"`). The runtime `LoadModel` gRPC is
//!   unary and the engine FFI exposes no progress callback, so true load % is
//!   not available.

use std::time::Duration;

use tokio::sync::mpsc;

use crate::domain::models::{
    DownloadModelCommand, ModelLoadCommand, ModelStatus, TaskStatus, UnifiedModel, UnifiedModelKind,
};
use crate::error::AppCoreError;
use crate::infra::db::{ModelStore, TaskStore};

use super::ModelService;

/// Coarse progress events emitted while ensuring a model is loaded. The
/// terminal outcome is carried by the `Result` of
/// [`ModelService::ensure_model_loaded_with_progress`]; these events are the
/// non-terminal deltas only.
#[derive(Debug, Clone)]
pub enum ModelLoadProgress {
    /// Weights are being downloaded into the local cache.
    Download { downloaded_bytes: u64, total_bytes: Option<u64> },
    /// Weights are local; the engine is loading them (coarse, no %).
    LoadPhase,
}

/// Poll interval for the download task status. The reporter itself throttles
/// writes to ~500 ms / 512 KB, so polling slightly finer than that keeps display
/// latency low without generating redundant deltas.
const DOWNLOAD_POLL_INTERVAL: Duration = Duration::from_millis(200);

impl ModelService {
    /// Ensure the model referenced by `command` is downloaded (if local) and
    /// loaded into the runtime, streaming coarse progress to `progress`.
    ///
    /// Composes `download_model` (idempotent — reuses an in-flight task) +
    /// `load_model`. Fast paths emit NO progress and return immediately:
    /// - **cloud** models (no local runtime load);
    /// - **local** models already loaded into the runtime (avoids per-turn
    ///   indicator flicker on repeated turns).
    ///
    /// Progress channel send errors are ignored — the caller may stop draining
    /// (e.g. turn interrupted); the load still completes for the runtime.
    pub async fn ensure_model_loaded_with_progress(
        &self,
        command: ModelLoadCommand,
        progress: mpsc::Sender<ModelLoadProgress>,
    ) -> Result<ModelStatus, AppCoreError> {
        let model_id = match command.model_id.as_deref().map(str::trim) {
            Some(id) if !id.is_empty() => id.to_owned(),
            // No model id to resolve (e.g. explicit backend+path); fall back to a
            // plain load with a single coarse phase.
            _ => {
                let _ = progress.send(ModelLoadProgress::LoadPhase).await;
                return self.load_model(command).await;
            }
        };

        let model = self.resolve_model_for_load(&model_id).await?;

        // Cloud models have no local runtime load — the agent calls the provider
        // directly. Short-circuit with no progress.
        if model.kind == UnifiedModelKind::Cloud {
            return Ok(ModelStatus {
                backend: "cloud".to_owned(),
                status: "ready".to_owned(),
                context_length: None,
                training_context_length: None,
                chat_template: None,
            });
        }

        // Already loaded into the runtime — nothing to do; no progress, no delay.
        if let Some(state) = self.runtime_state_for_model(&model).await
            && state.loaded
        {
            return Ok(ModelStatus {
                backend: state.backend_id.to_string(),
                status: "loaded".to_owned(),
                context_length: None,
                training_context_length: None,
                chat_template: None,
            });
        }

        // If the weights aren't local yet, download first (streaming byte progress).
        let already_downloaded =
            model.spec.local_path.as_deref().map(str::trim).is_some_and(|value| !value.is_empty());
        if !already_downloaded {
            self.run_download_with_progress(&model_id, &progress).await?;
        }

        // Coarse "loading" phase, then the actual engine load (idempotent if a
        // concurrent load already completed).
        let _ = progress.send(ModelLoadProgress::LoadPhase).await;
        self.load_model(command).await
    }

    async fn resolve_model_for_load(&self, model_id: &str) -> Result<UnifiedModel, AppCoreError> {
        let record = self
            .model_state
            .store()
            .get_model(model_id)
            .await?
            .ok_or_else(|| AppCoreError::NotFound(format!("model {model_id} not found")))?;
        record.try_into().map_err(|error: String| AppCoreError::Internal(error))
    }

    /// Trigger the (idempotent) download and poll its task status, forwarding
    /// byte-level progress until it terminates. Returns an error if the download
    /// ends without success.
    async fn run_download_with_progress(
        &self,
        model_id: &str,
        progress: &mpsc::Sender<ModelLoadProgress>,
    ) -> Result<(), AppCoreError> {
        let accepted =
            self.download_model(DownloadModelCommand { model_id: model_id.to_owned() }).await?;
        let task_id = accepted.operation_id;
        let store = self.model_state.store();

        loop {
            let task = store.get_task(&task_id).await?.ok_or_else(|| {
                AppCoreError::Internal(format!("download task {task_id} not found"))
            })?;

            match task.status {
                TaskStatus::Succeeded => break,
                TaskStatus::Failed | TaskStatus::Cancelled | TaskStatus::Interrupted => {
                    let detail = task
                        .error_msg
                        .as_deref()
                        .map(|message| format!(": {message}"))
                        .unwrap_or_default();
                    return Err(AppCoreError::Internal(format!(
                        "model download for {model_id} ended with status {}{detail}",
                        task.status.as_str()
                    )));
                }
                TaskStatus::Pending | TaskStatus::Running => {}
            }

            if let Some((downloaded, total)) =
                task.result_data.as_deref().and_then(parse_download_progress)
            {
                let _ = progress
                    .send(ModelLoadProgress::Download {
                        downloaded_bytes: downloaded,
                        total_bytes: total,
                    })
                    .await;
            }

            tokio::time::sleep(DOWNLOAD_POLL_INTERVAL).await;
        }

        Ok(())
    }
}

/// Parse the `{"progress":{"current":..,"total":..}}` payload the download
/// progress reporter writes into the task `result_data`. Returns `None` for any
/// shape that isn't a progress payload (e.g. the terminal `{"local_path":..}`
/// written on success, which we never read because we break on `Succeeded`).
fn parse_download_progress(payload: &str) -> Option<(u64, Option<u64>)> {
    #[derive(serde::Deserialize)]
    struct Wrapper {
        progress: Progress,
    }
    #[derive(serde::Deserialize, Default)]
    struct Progress {
        #[serde(default)]
        current: u64,
        #[serde(default)]
        total: Option<u64>,
    }
    let wrapper: Wrapper = serde_json::from_str(payload).ok()?;
    Some((wrapper.progress.current, wrapper.progress.total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_download_progress_reads_current_and_total() {
        let payload =
            r#"{"progress":{"label":"model.gguf","current":1024,"total":4096,"unit":"bytes"}}"#;
        assert_eq!(parse_download_progress(payload), Some((1024, Some(4096))));
    }

    #[test]
    fn parse_download_progress_handles_missing_total() {
        let payload = r#"{"progress":{"current":1024}}"#;
        assert_eq!(parse_download_progress(payload), Some((1024, None)));
    }

    #[test]
    fn parse_download_progress_rejects_non_progress_payload() {
        // Terminal success payload shape — not a progress document.
        assert_eq!(parse_download_progress(r#"{"local_path":"/x/model.gguf"}"#), None);
        assert_eq!(parse_download_progress("not json"), None);
    }
}
