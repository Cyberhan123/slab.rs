use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use slab_hub::{DownloadProgress, DownloadProgressUpdate};
use tracing::warn;

use crate::domain::models::{TaskProgress, TaskStatus};
use crate::infra::db::TaskStore;

const MODEL_DOWNLOAD_PROGRESS_MIN_INTERVAL: Duration = Duration::from_millis(500);
const MODEL_DOWNLOAD_PROGRESS_MIN_BYTES_DELTA: u64 = 512 * 1024;

#[derive(Serialize)]
struct ModelDownloadProgressPayload {
    progress: TaskProgress,
}

#[derive(Debug, Default)]
struct ModelDownloadProgressState {
    last_progress: Option<TaskProgress>,
    last_published_at: Option<Instant>,
}

#[derive(Debug)]
pub(super) struct ModelDownloadProgressReporter {
    task_id: String,
    store: Arc<crate::infra::db::AnyStore>,
    artifact_index_by_filename: HashMap<String, u32>,
    artifact_count: u32,
    state: Mutex<ModelDownloadProgressState>,
}

impl ModelDownloadProgressReporter {
    pub(super) fn new(
        task_id: impl Into<String>,
        store: Arc<crate::infra::db::AnyStore>,
        artifacts: &BTreeMap<String, String>,
    ) -> Self {
        let artifact_index_by_filename = artifacts
            .values()
            .enumerate()
            .map(|(index, filename)| (filename.clone(), index as u32))
            .collect();

        Self {
            task_id: task_id.into(),
            store,
            artifact_index_by_filename,
            artifact_count: artifacts.len() as u32,
            state: Mutex::new(ModelDownloadProgressState::default()),
        }
    }

    fn to_task_progress(&self, update: &DownloadProgressUpdate) -> TaskProgress {
        let step =
            self.artifact_index_by_filename.get(&update.filename).copied().map(|index| index + 1);

        TaskProgress {
            label: Some(update.filename.clone()),
            message: None,
            current: update.downloaded_bytes,
            total: update.total_bytes,
            unit: Some("bytes".to_owned()),
            step,
            step_count: (self.artifact_count > 1).then_some(self.artifact_count),
            logs: None,
        }
    }

    fn publish(&self, update: &DownloadProgressUpdate, force: bool) {
        let progress = self.to_task_progress(update);
        let should_publish = {
            let mut state = self.state.lock().expect("model download progress state");
            if !should_publish_model_download_progress(&state, &progress, force) {
                false
            } else {
                state.last_progress = Some(progress.clone());
                state.last_published_at = Some(Instant::now());
                true
            }
        };

        if !should_publish {
            return;
        }

        let payload =
            serde_json::to_string(&ModelDownloadProgressPayload { progress }).unwrap_or_default();
        let task_id = self.task_id.clone();
        let store = Arc::clone(&self.store);

        tokio::spawn(async move {
            if let Err(error) = store
                .update_task_status_if_active(&task_id, TaskStatus::Running, Some(&payload), None)
                .await
            {
                warn!(task_id = %task_id, error = %error, "failed to persist model download progress");
            }
        });
    }
}

impl DownloadProgress for ModelDownloadProgressReporter {
    fn on_start(&self, update: &DownloadProgressUpdate) {
        self.publish(update, true);
    }

    fn on_progress(&self, update: &DownloadProgressUpdate) {
        self.publish(update, false);
    }

    fn on_finish(&self, update: &DownloadProgressUpdate) {
        self.publish(update, true);
    }
}

fn should_publish_model_download_progress(
    state: &ModelDownloadProgressState,
    progress: &TaskProgress,
    force: bool,
) -> bool {
    if force {
        return true;
    }

    let Some(previous) = state.last_progress.as_ref() else {
        return true;
    };

    if previous == progress {
        return false;
    }

    if previous.label != progress.label
        || previous.total != progress.total
        || previous.step != progress.step
        || previous.step_count != progress.step_count
        || progress.current < previous.current
    {
        return true;
    }

    if progress.total.is_some() && Some(progress.current) == progress.total {
        return true;
    }

    if progress.current.saturating_sub(previous.current) >= MODEL_DOWNLOAD_PROGRESS_MIN_BYTES_DELTA
    {
        return true;
    }

    state
        .last_published_at
        .is_none_or(|published_at| published_at.elapsed() >= MODEL_DOWNLOAD_PROGRESS_MIN_INTERVAL)
}

#[cfg(test)]
mod tests {
    use super::{
        MODEL_DOWNLOAD_PROGRESS_MIN_BYTES_DELTA, MODEL_DOWNLOAD_PROGRESS_MIN_INTERVAL,
        ModelDownloadProgressState, should_publish_model_download_progress,
    };
    use crate::domain::models::TaskProgress;
    use std::time::{Duration, Instant};

    #[test]
    fn progress_publish_rules_allow_initial_forced_and_terminal_updates() {
        let initial = task_progress("model.gguf", 0, Some(100), Some(1), Some(2));

        assert!(should_publish_model_download_progress(
            &ModelDownloadProgressState::default(),
            &initial,
            false
        ));

        let state = state_with(initial.clone(), Duration::ZERO);
        assert!(should_publish_model_download_progress(&state, &initial, true));

        let finished = task_progress("model.gguf", 100, Some(100), Some(1), Some(2));
        assert!(should_publish_model_download_progress(&state, &finished, false));
    }

    #[test]
    fn progress_publish_rules_throttle_duplicate_and_small_byte_updates() {
        let previous = task_progress("model.gguf", 1_000, Some(10_000), Some(1), Some(2));
        let state = state_with(previous.clone(), Duration::ZERO);

        assert!(!should_publish_model_download_progress(&state, &previous, false));

        let small_delta = task_progress(
            "model.gguf",
            previous.current + MODEL_DOWNLOAD_PROGRESS_MIN_BYTES_DELTA - 1,
            Some(10_000),
            Some(1),
            Some(2),
        );
        assert!(!should_publish_model_download_progress(&state, &small_delta, false));

        let large_delta = task_progress(
            "model.gguf",
            previous.current + MODEL_DOWNLOAD_PROGRESS_MIN_BYTES_DELTA,
            Some(10_000),
            Some(1),
            Some(2),
        );
        assert!(should_publish_model_download_progress(&state, &large_delta, false));
    }

    #[test]
    fn progress_publish_rules_publish_shape_changes_and_stale_updates() {
        let previous = task_progress("model.gguf", 1_000, Some(10_000), Some(1), Some(2));
        let fresh_state = state_with(previous, Duration::ZERO);

        assert!(should_publish_model_download_progress(
            &fresh_state,
            &task_progress("tokenizer.json", 1, Some(10_000), Some(2), Some(2)),
            false
        ));
        assert!(should_publish_model_download_progress(
            &fresh_state,
            &task_progress("model.gguf", 999, Some(10_000), Some(1), Some(2)),
            false
        ));
        assert!(should_publish_model_download_progress(
            &fresh_state,
            &task_progress("model.gguf", 1_001, Some(12_000), Some(1), Some(2)),
            false
        ));

        let stale_state = state_with(
            task_progress("model.gguf", 1_000, Some(10_000), Some(1), Some(2)),
            MODEL_DOWNLOAD_PROGRESS_MIN_INTERVAL + Duration::from_millis(1),
        );
        assert!(should_publish_model_download_progress(
            &stale_state,
            &task_progress("model.gguf", 1_001, Some(10_000), Some(1), Some(2)),
            false
        ));
    }

    fn state_with(progress: TaskProgress, age: Duration) -> ModelDownloadProgressState {
        ModelDownloadProgressState {
            last_progress: Some(progress),
            last_published_at: Some(Instant::now() - age),
        }
    }

    fn task_progress(
        filename: &str,
        current: u64,
        total: Option<u64>,
        step: Option<u32>,
        step_count: Option<u32>,
    ) -> TaskProgress {
        TaskProgress {
            label: Some(filename.to_owned()),
            message: None,
            current,
            total,
            unit: Some("bytes".to_owned()),
            step,
            step_count,
            logs: None,
        }
    }
}
