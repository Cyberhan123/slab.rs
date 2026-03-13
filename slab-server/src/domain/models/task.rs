use crate::infra::db::TaskRecord;

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct AcceptedOperation {
    pub operation_id: String,
}

#[derive(Debug, Clone)]
pub struct TaskView {
    pub id: String,
    pub task_type: String,
    pub status: String,
    pub error_msg: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskResult {
    pub image: Option<String>,
    pub images: Option<Vec<String>>,
    pub video_path: Option<String>,
    pub text: Option<String>,
}

impl From<&TaskRecord> for TaskView {
    fn from(record: &TaskRecord) -> Self {
        Self {
            id: record.id.clone(),
            task_type: record.task_type.clone(),
            status: record.status.clone(),
            error_msg: record.error_msg.clone(),
            created_at: record.created_at.to_rfc3339(),
            updated_at: record.updated_at.to_rfc3339(),
        }
    }
}
