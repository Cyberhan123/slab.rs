use tracing::info;

use crate::context::WorkerState;
use crate::domain::models::{TaskResult, TaskView};
use crate::error::ServerError;
use crate::infra::db::TaskStore;

#[derive(Clone)]
pub struct TaskApplicationService {
    state: WorkerState,
}

impl TaskApplicationService {
    pub fn new(state: WorkerState) -> Self {
        Self { state }
    }

    pub async fn list_tasks(&self, task_type: Option<&str>) -> Result<Vec<TaskView>, ServerError> {
        let records = self.state.store().list_tasks(task_type).await?;
        Ok(records
            .into_iter()
            .map(|record| TaskView::from(&record))
            .collect())
    }

    pub async fn get_task(&self, id: &str) -> Result<TaskView, ServerError> {
        let record = self
            .state
            .store()
            .get_task(id)
            .await?
            .ok_or_else(|| ServerError::NotFound(format!("task {id} not found")))?;

        Ok(TaskView::from(&record))
    }

    pub async fn get_task_result(&self, id: &str) -> Result<TaskResult, ServerError> {
        let record = self
            .state
            .store()
            .get_task(id)
            .await?
            .ok_or_else(|| ServerError::NotFound(format!("task {id} not found")))?;

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
        self.state.cancel_operation(id);

        info!(task_id = %id, "task cancelled");
        let updated = self
            .state
            .store()
            .get_task(id)
            .await?
            .ok_or_else(|| ServerError::NotFound(format!("task {id} not found after cancel")))?;
        Ok(TaskView::from(&updated))
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
