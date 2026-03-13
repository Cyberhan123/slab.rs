use std::future::Future;
use std::pin::Pin;

use crate::domain::models::TaskResult;
use crate::error::ServerError;

pub trait TaskResultPort: Send + Sync {
    fn get_task_result(
        &self,
        id: String,
    ) -> Pin<Box<dyn Future<Output = Result<TaskResult, ServerError>> + Send + '_>>;
}

pub struct GetTaskResultUseCase<P> {
    port: P,
}

impl<P> GetTaskResultUseCase<P>
where
    P: TaskResultPort,
{
    pub fn new(port: P) -> Self {
        Self { port }
    }

    pub async fn execute(&self, id: String) -> Result<TaskResult, ServerError> {
        self.port.get_task_result(id).await
    }
}
