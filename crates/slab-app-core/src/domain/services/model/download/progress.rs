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
