use tracing::info;

use crate::context::WorkerState;
use crate::domain::models::{TaskResult, TaskStatus, TaskView};
use crate::error::AppCoreError;
use crate::infra::db::TaskStore;

#[derive(Clone)]
pub struct TaskApplicationService {
    state: WorkerState,
}

impl TaskApplicationService {
    pub fn new(state: WorkerState) -> Self {
        Self { state }
    }

    pub async fn list_tasks(&self, task_type: Option<&str>) -> Result<Vec<TaskView>, AppCoreError> {
        let records = self.state.store().list_tasks(task_type).await?;
        Ok(records.into_iter().map(|record| TaskView::from(&record)).collect())
    }

    pub async fn get_task(&self, id: &str) -> Result<TaskView, AppCoreError> {
        let record = self
            .state
            .store()
            .get_task(id)
            .await?
            .ok_or_else(|| AppCoreError::NotFound(format!("task {id} not found")))?;

        Ok(TaskView::from(&record))
    }

    pub async fn get_task_result(&self, id: &str) -> Result<TaskResult, AppCoreError> {
        let record = self
            .state
            .store()
            .get_task(id)
            .await?
            .ok_or_else(|| AppCoreError::NotFound(format!("task {id} not found")))?;

        match record.status {
            TaskStatus::Succeeded => Ok(record
                .result_data
                .map(|data| {
                    deserialize_task_result(&data)
                        .unwrap_or(TaskResult { text: Some(data), ..TaskResult::default() })
                })
                .unwrap_or_default()),
            status => {
                Err(AppCoreError::BadRequest(format!("task is not succeeded (status: {status})")))
            }
        }
    }

    pub async fn cancel_task(&self, id: &str) -> Result<TaskView, AppCoreError> {
        let record = self
            .state
            .store()
            .get_task(id)
            .await?
            .ok_or_else(|| AppCoreError::NotFound(format!("task {id} not found")))?;

        if !record.status.is_cancellable() {
            return Err(AppCoreError::BadRequest(format!(
                "task {id} is not cancellable (status: {})",
                record.status
            )));
        }

        self.state.store().update_task_status(id, TaskStatus::Cancelled, None, None).await?;
        self.state.cancel_operation(id);

        info!(task_id = %id, "task cancelled");
        let updated =
            self.state.store().get_task(id).await?.ok_or_else(|| {
                AppCoreError::NotFound(format!("task {id} not found after cancel"))
            })?;
        Ok(TaskView::from(&updated))
    }

    pub async fn validate_restartable(&self, id: &str) -> Result<(), AppCoreError> {
        let record = self
            .state
            .store()
            .get_task(id)
            .await?
            .ok_or_else(|| AppCoreError::NotFound(format!("task {id} not found")))?;

        if !record.status.is_restartable() {
            return Err(AppCoreError::BadRequest(format!(
                "task {id} cannot be restarted (status: {})",
                record.status
            )));
        }

        Ok(())
    }
}

fn deserialize_task_result(raw: &str) -> Result<TaskResult, serde_json::Error> {
    serde_json::from_str(raw)
}

#[cfg(test)]
mod tests {
    use super::deserialize_task_result;

    #[test]
    fn deserializes_structured_task_result() {
        let result = deserialize_task_result(
            r#"{"text":"done","images":["a.png","b.png"],"segments":[{"start_ms":0,"end_ms":120,"text":"hello"}]}"#,
        )
        .expect("task result should deserialize");

        assert_eq!(result.text.as_deref(), Some("done"));
        assert_eq!(result.images.as_deref(), Some(&["a.png".to_owned(), "b.png".to_owned()][..]));
        let segments = result.segments.expect("segments should be present");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text.as_deref(), Some("hello"));
    }

    #[test]
    fn rejects_non_object_task_result_json() {
        assert!(deserialize_task_result(r#""plain text""#).is_err());
    }
}
