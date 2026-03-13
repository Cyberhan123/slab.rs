use crate::entities::contexts::task::domain::TaskRecord;
use std::future::Future;

pub trait TaskRepository: Send + Sync + 'static {
    fn insert_task(
        &self,
        record: TaskRecord,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn update_task_status(
        &self,
        id: &str,
        status: &str,
        result_data: Option<&str>,
        error_msg: Option<&str>,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn get_task(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<Option<TaskRecord>, sqlx::Error>> + Send;
    fn list_tasks(
        &self,
        task_type: Option<&str>,
    ) -> impl Future<Output = Result<Vec<TaskRecord>, sqlx::Error>> + Send;
    fn interrupt_running_tasks(&self) -> impl Future<Output = Result<u64, sqlx::Error>> + Send;
}
