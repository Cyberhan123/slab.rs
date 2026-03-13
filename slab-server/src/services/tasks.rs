use crate::context::WorkerState;
use crate::domain::models::{TaskResult, TaskView};
use crate::domain::services::TaskApplicationService;
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
    ) -> Result<Vec<TaskView>, ServerError> {
        let service = TaskApplicationService::new(self.state.clone());
        service.list_tasks(task_type).await
    }

    pub async fn get_task(&self, id: &str) -> Result<TaskView, ServerError> {
        let service = TaskApplicationService::new(self.state.clone());
        service.get_task(id).await
    }

    pub async fn get_task_result(&self, id: &str) -> Result<TaskResult, ServerError> {
        let service = TaskApplicationService::new(self.state.clone());
        service.get_task_result(id).await
    }

    pub async fn cancel_task(&self, id: &str) -> Result<TaskView, ServerError> {
        let service = TaskApplicationService::new(self.state.clone());
        service.cancel_task(id).await
    }

    pub async fn validate_restartable(&self, id: &str) -> Result<(), ServerError> {
        let service = TaskApplicationService::new(self.state.clone());
        service.validate_restartable(id).await
    }
}
