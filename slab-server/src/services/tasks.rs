use crate::api::v1::tasks::schema::{TaskResponse, TaskResultPayload};
use crate::context::WorkerState;
use crate::domain::services::{to_task_result_response, TaskApplicationService};
use crate::error::ServerError;

#[derive(Clone)]
pub struct TasksService {
    state: WorkerState,
}

impl TasksService {
    pub fn new(state: WorkerState) -> Self {
        Self { state }
    }

    pub async fn list_tasks(
        &self,
        task_type: Option<&str>,
    ) -> Result<Vec<TaskResponse>, ServerError> {
        let service = TaskApplicationService::new(self.state.clone());
        service.list_tasks(task_type).await
    }

    pub async fn get_task(&self, id: &str) -> Result<TaskResponse, ServerError> {
        let service = TaskApplicationService::new(self.state.clone());
        service.get_task(id).await
    }

    pub async fn get_task_result(&self, id: &str) -> Result<TaskResultPayload, ServerError> {
        let service = TaskApplicationService::new(self.state.clone());
        let result = service.get_task_result(id).await?;
        Ok(to_task_result_response(result))
    }

    pub async fn cancel_task(&self, id: &str) -> Result<TaskResponse, ServerError> {
        let service = TaskApplicationService::new(self.state.clone());
        service.cancel_task(id).await
    }

    pub async fn validate_restartable(&self, id: &str) -> Result<(), ServerError> {
        let service = TaskApplicationService::new(self.state.clone());
        service.validate_restartable(id).await
    }
}
