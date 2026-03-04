use crate::entities::TaskRecord;
use serde::{Deserialize, Serialize};
use slab_core::TaskStatus;
use utoipa::{IntoParams, ToSchema};

/// Result payload returned by `GET /v1/tasks/{id}/result`.
///
/// Exactly one field is populated depending on the task type:
/// - `image` tasks: `image` contains a `data:image/png;base64,…` data URI.
/// - Text-producing tasks (whisper, etc.): `text` contains the UTF-8 result.
/// - JSON-returning tasks: both fields may be absent; the body is a JSON object.
#[derive(Serialize, ToSchema)]
pub struct TaskResultPayload {
    /// Base64-encoded PNG data URI, present for `image` task results.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Text content, present for `whisper` and other text-producing task results.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

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
    pub error_msg: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl TaskRecord {
    pub fn to_response(&self) -> TaskResponse {
        TaskResponse {
            id: self.id.clone(),
            task_type: self.task_type.clone(),
            status: self.status.clone(),
            error_msg: self.error_msg.clone(),
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
            // Result was already consumed; task is still considered succeeded.
            TaskStatus::ResultConsumed => "succeeded",
            TaskStatus::SucceededStreaming => "succeeded",
            TaskStatus::Failed { .. } => "failed",
            TaskStatus::Cancelled => "cancelled",
        }
    }
}
