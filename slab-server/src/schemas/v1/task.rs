use crate::entities::TaskRecord;
use serde::{Deserialize, Serialize};
use slab_core::TaskStatus;
use utoipa::{IntoParams, ToSchema};

#[derive(Deserialize, ToSchema, IntoParams)]
pub struct TaskTypeQuery {
    #[serde(rename = "type")]
    pub task_type: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct TaskResponse {
    pub id: String,
    pub task_type: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl TaskRecord {
    pub fn to_response(&self) -> TaskResponse {
        TaskResponse {
            id: self.id.clone(),
            task_type: self.task_type.clone(),
            status: self.status.clone(),
            created_at: self.created_at.to_rfc3339(),
            updated_at: self.updated_at.to_rfc3339(),
        }
    }
}

pub trait TaskStatusEnumExt {
    fn as_str(&self) -> &'static str;
}

impl TaskStatusEnumExt for TaskStatus {
    fn as_str(&self) -> &'static str {
        match &self {
            TaskStatus::Pending => "pending",
            TaskStatus::Running { .. } => "running",
            TaskStatus::Succeeded { .. } => "succeeded",
            TaskStatus::SucceededStreaming => "succeeded",
            TaskStatus::Failed { .. } => "failed",
            TaskStatus::Cancelled => "cancelled",
        }
    }
}
