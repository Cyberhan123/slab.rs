use tracing::{info, warn};

use crate::context::WorkerState;
use crate::domain::models::{TaskResult, TaskView};
use crate::domain::services::to_task_view;
use crate::error::ServerError;
use crate::infra::db::TaskStore;
use crate::infra::rpc::adapter::payload_to_task_result;

#[derive(Clone)]
pub struct TaskApplicationService {
    state: WorkerState,
}

impl TaskApplicationService {
    pub fn new(state: WorkerState) -> Self {
        Self { state }
    }

    pub async fn list_tasks(
        &self,
        task_type: Option<&str>,
    ) -> Result<Vec<TaskView>, ServerError> {
        let records = self.state.store().list_tasks(task_type).await?;
        Ok(records
            .into_iter()
            .map(|record| to_task_view(&record))
            .collect())
    }

    pub async fn get_task(&self, id: &str) -> Result<TaskView, ServerError> {
        let mut record = self
            .state
            .store()
            .get_task(id)
            .await?
            .ok_or_else(|| ServerError::NotFound(format!("task {id} not found")))?;

        if let Some(core_tid) = record.core_task_id {
            if let Ok(view) = slab_core::api::status(core_tid as u64).await {
                let live_status = core_task_status(&view.status);
                let live_error = match &view.status {
                    slab_core::TaskStatus::Failed { error } => Some(error.to_string()),
                    _ => None,
                };
                if live_status != record.status
                    || live_error.as_deref() != record.error_msg.as_deref()
                {
                    self.state
                        .store()
                        .update_task_status(id, live_status, None, live_error.as_deref())
                        .await
                        .unwrap_or_else(|e| warn!(error = %e, "failed to sync task status"));
                    record.status = live_status.to_owned();
                    record.error_msg = live_error;
                }
            }
        }

        Ok(to_task_view(&record))
    }

    pub async fn get_task_result(&self, id: &str) -> Result<TaskResult, ServerError> {
        let record = self
            .state
            .store()
            .get_task(id)
            .await?
            .ok_or_else(|| ServerError::NotFound(format!("task {id} not found")))?;

        if let Some(core_tid) = record.core_task_id {
            match slab_core::api::result(core_tid as u64).await {
                Ok(Some(payload)) => {
                    let result_payload = payload_to_task_result(&record.task_type, &payload);
                    if let Ok(result_json) = serialize_task_result(&result_payload) {
                        self.state
                            .store()
                            .update_task_status(id, "succeeded", Some(&result_json), None)
                            .await
                            .unwrap_or_else(|e| warn!(error = %e, "failed to persist result"));
                    } else {
                        warn!(task_id = %id, "failed to serialize result payload");
                    }
                    return Ok(result_payload);
                }
                Ok(None) => {
                    if let Some(data) = record.result_data {
                        let result_payload = deserialize_task_result(&data).unwrap_or_else(|e| {
                            warn!(task_id = %id, error = %e, "persisted result_data is not a TaskResultPayload; returning as text");
                            TaskResult {
                                image: None,
                                images: None,
                                video_path: None,
                                text: Some(data),
                            }
                        });
                        return Ok(result_payload);
                    }
                    return Err(ServerError::BadRequest(format!(
                        "task {id} is not completed yet"
                    )));
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    self.state
                        .store()
                        .update_task_status(id, "failed", None, Some(&err_msg))
                        .await
                        .unwrap_or_else(
                            |db_e| warn!(error = %db_e, "failed to sync failed task error"),
                        );
                    return Err(ServerError::Runtime(e));
                }
            }
        }

        match record.status.as_str() {
            "succeeded" => Ok(record
                .result_data
                .map(|data| {
                    deserialize_task_result(&data).unwrap_or(TaskResult {
                        image: None,
                        images: None,
                        video_path: None,
                        text: Some(data),
                    })
                })
                .unwrap_or(TaskResult {
                    image: None,
                    images: None,
                    video_path: None,
                    text: None,
                })),
            status => Err(ServerError::BadRequest(format!(
                "task is not succeeded (status: {status})"
            ))),
        }
    }

    pub async fn cancel_task(&self, id: &str) -> Result<TaskView, ServerError> {
        let record = self
            .state
            .store()
            .get_task(id)
            .await?
            .ok_or_else(|| ServerError::NotFound(format!("task {id} not found")))?;

        if !is_cancellable(&record.status) {
            return Err(ServerError::BadRequest(format!(
                "task {id} is not cancellable (status: {})",
                record.status
            )));
        }

        self.state
            .store()
            .update_task_status(id, "cancelled", None, None)
            .await?;

        if let Some(core_tid) = record.core_task_id {
            if let Err(e) = slab_core::api::cancel(core_tid as u64) {
                warn!(task_id = %id, error = %e, "failed to cancel slab-core task");
            }
        }
        self.state.cancel_operation(id);

        info!(task_id = %id, "task cancelled");
        let updated =
            self.state.store().get_task(id).await?.ok_or_else(|| {
                ServerError::NotFound(format!("task {id} not found after cancel"))
            })?;
        Ok(to_task_view(&updated))
    }

    pub async fn validate_restartable(&self, id: &str) -> Result<(), ServerError> {
        let record = self
            .state
            .store()
            .get_task(id)
            .await?
            .ok_or_else(|| ServerError::NotFound(format!("task {id} not found")))?;

        if !is_restartable(&record.status) {
            return Err(ServerError::BadRequest(format!(
                "task {id} cannot be restarted (status: {})",
                record.status
            )));
        }

        Ok(())
    }
}

fn is_cancellable(status: &str) -> bool {
    matches!(status, "pending" | "running")
}

fn is_restartable(status: &str) -> bool {
    matches!(status, "failed" | "cancelled" | "interrupted")
}

fn serialize_task_result(result: &TaskResult) -> Result<String, serde_json::Error> {
    serde_json::to_string(&serde_json::json!({
        "image": result.image,
        "images": result.images,
        "video_path": result.video_path,
        "text": result.text,
    }))
}

fn deserialize_task_result(raw: &str) -> Result<TaskResult, serde_json::Error> {
    let value: serde_json::Value = serde_json::from_str(raw)?;
    Ok(TaskResult {
        image: value
            .get("image")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        images: value.get("images").and_then(|v| v.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        }),
        video_path: value
            .get("video_path")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        text: value
            .get("text")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
    })
}

fn core_task_status(status: &slab_core::TaskStatus) -> &'static str {
    match status {
        slab_core::TaskStatus::Pending => "pending",
        slab_core::TaskStatus::Running { .. } => "running",
        slab_core::TaskStatus::Succeeded { .. }
        | slab_core::TaskStatus::ResultConsumed
        | slab_core::TaskStatus::SucceededStreaming => "succeeded",
        slab_core::TaskStatus::Failed { .. } => "failed",
        slab_core::TaskStatus::Cancelled => "cancelled",
    }
}
