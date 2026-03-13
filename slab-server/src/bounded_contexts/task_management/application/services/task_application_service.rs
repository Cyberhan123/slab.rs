use std::sync::Arc;

use base64::Engine as _;
use tracing::{info, warn};

use crate::bounded_contexts::task_management::domain::TaskAggregate;
use crate::contexts::task::domain::TaskResult;
use crate::contexts::task::interface::http::mappers::task_mapper::to_task_response;
use crate::entities::TaskStore;
use crate::error::ServerError;
use crate::schemas::v1::task::{TaskResponse, TaskResultPayload};
use crate::state::TaskContext;

#[derive(Clone)]
pub struct TaskApplicationService {
    context: Arc<TaskContext>,
}

impl TaskApplicationService {
    pub fn new(context: Arc<TaskContext>) -> Self {
        Self { context }
    }

    pub async fn list_tasks(
        &self,
        task_type: Option<&str>,
    ) -> Result<Vec<TaskResponse>, ServerError> {
        let records = self.context.store.list_tasks(task_type).await?;
        Ok(records
            .into_iter()
            .map(|record| to_task_response(&record))
            .collect())
    }

    pub async fn get_task(&self, id: &str) -> Result<TaskResponse, ServerError> {
        let mut record = self
            .context
            .store
            .get_task(id)
            .await?
            .ok_or_else(|| ServerError::NotFound(format!("task {id} not found")))?;

        if let Some(core_tid) = record.core_task_id {
            if let Ok(view) = slab_core::api::status(core_tid as u64).await {
                let live_status = view.status.as_str();
                let live_error = match &view.status {
                    slab_core::TaskStatus::Failed { error } => Some(error.to_string()),
                    _ => None,
                };
                if live_status != record.status
                    || live_error.as_deref() != record.error_msg.as_deref()
                {
                    self.context
                        .store
                        .update_task_status(id, live_status, None, live_error.as_deref())
                        .await
                        .unwrap_or_else(|e| warn!(error = %e, "failed to sync task status"));
                    record.status = live_status.to_owned();
                    record.error_msg = live_error;
                }
            }
        }

        Ok(to_task_response(&record))
    }

    pub async fn get_task_result(&self, id: &str) -> Result<TaskResult, ServerError> {
        let record = self
            .context
            .store
            .get_task(id)
            .await?
            .ok_or_else(|| ServerError::NotFound(format!("task {id} not found")))?;

        if let Some(core_tid) = record.core_task_id {
            match slab_core::api::result(core_tid as u64).await {
                Ok(Some(payload)) => {
                    let result_payload = map_payload(&record.task_type, &payload);
                    if let Ok(result_json) = serde_json::to_string(&result_payload) {
                        self.context
                            .store
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
                        let result_payload =
                            deserialize_task_result(&data).unwrap_or_else(|e| {
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
                    self.context
                        .store
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

    pub async fn cancel_task(&self, id: &str) -> Result<TaskResponse, ServerError> {
        let record = self
            .context
            .store
            .get_task(id)
            .await?
            .ok_or_else(|| ServerError::NotFound(format!("task {id} not found")))?;

        let aggregate = TaskAggregate::from_record(record.clone());
        if !aggregate.is_cancellable() {
            return Err(ServerError::BadRequest(format!(
                "task {id} is not cancellable (status: {})",
                record.status
            )));
        }

        self.context
            .store
            .update_task_status(id, "cancelled", None, None)
            .await?;

        if let Some(core_tid) = record.core_task_id {
            if let Err(e) = slab_core::api::cancel(core_tid as u64) {
                warn!(task_id = %id, error = %e, "failed to cancel slab-core task");
            }
        }
        self.context.task_manager.cancel(id);

        info!(task_id = %id, "task cancelled");
        let updated =
            self.context.store.get_task(id).await?.ok_or_else(|| {
                ServerError::NotFound(format!("task {id} not found after cancel"))
            })?;
        Ok(to_task_response(&updated))
    }

    pub async fn validate_restartable(&self, id: &str) -> Result<(), ServerError> {
        let record = self
            .context
            .store
            .get_task(id)
            .await?
            .ok_or_else(|| ServerError::NotFound(format!("task {id} not found")))?;

        if !TaskAggregate::from_record(record.clone()).is_restartable() {
            return Err(ServerError::BadRequest(format!(
                "task {id} cannot be restarted (status: {})",
                record.status
            )));
        }

        Ok(())
    }
}

fn map_payload(task_type: &str, payload: &slab_core::Payload) -> TaskResult {
    match payload {
        slab_core::Payload::Bytes(bytes) => {
            if task_type == "image" {
                let encoded = base64::engine::general_purpose::STANDARD.encode(bytes.as_ref());
                let uri = format!("data:image/png;base64,{encoded}");
                TaskResult {
                    image: Some(uri.clone()),
                    images: Some(vec![uri]),
                    video_path: None,
                    text: None,
                }
            } else {
                TaskResult {
                    image: None,
                    images: None,
                    video_path: None,
                    text: Some(String::from_utf8_lossy(bytes).to_string()),
                }
            }
        }
        slab_core::Payload::Text(text) => TaskResult {
            image: None,
            images: None,
            video_path: None,
            text: Some(text.clone()),
        },
        slab_core::Payload::Json(value) => {
            let image = value
                .get("image")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            let images = value.get("images").and_then(|v| {
                v.as_array().map(|arr| {
                    arr.iter()
                        .filter_map(|item| item.as_str().map(str::to_owned))
                        .collect::<Vec<_>>()
                })
            });
            let video_path = value
                .get("video_path")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            let text = value
                .get("text")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .or_else(|| {
                    if image.is_none() && video_path.is_none() {
                        Some(value.to_string())
                    } else {
                        None
                    }
                });

            TaskResult {
                image,
                images,
                video_path,
                text,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;

    use super::map_payload;

    #[test]
    fn image_payload_maps_to_data_uri() {
        let payload = slab_core::Payload::Bytes(bytes::Bytes::from_static(b"\x89PNG\r\n\x1a\n"));
        let result = map_payload("image", &payload);

        let uri = result.image.expect("image must exist");
        assert!(uri.starts_with("data:image/png;base64,"));
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(uri.trim_start_matches("data:image/png;base64,"))
            .expect("must decode");
        assert_eq!(decoded, b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn unknown_json_maps_to_text() {
        let payload = slab_core::Payload::Json(serde_json::json!({ "unexpected": 1 }));
        let result = map_payload("task", &payload);
        assert!(result
            .text
            .as_deref()
            .unwrap_or_default()
            .contains("unexpected"));
    }
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
