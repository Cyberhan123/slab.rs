use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OperationAcceptedResponse {
    pub operation_id: String,
}

/// Result payload returned by `GET /v1/tasks/{id}/result`.
///
/// Fields are populated depending on the task type:
/// - Single-image tasks: `image` contains a `data:image/png;base64,鈥 data URI.
/// - Multi-image diffusion tasks: `images` contains an array of data URIs; `image`
///   also holds the first one for backward compatibility.
/// - Video tasks: `video_path` holds the path of the assembled MP4 file.
/// - Text-producing tasks (whisper, etc.): `text` contains the UTF-8 result.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct TaskResultPayload {
    /// Base64-encoded PNG data URI, present for single-image and as the first
    /// image for multi-image task results.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Array of base64-encoded PNG data URIs for multi-image task results.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
    /// Absolute path to the assembled MP4 video file for video task results.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_path: Option<String>,
    /// Text content, present for `whisper` and other text-producing task results.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Deserialize, ToSchema, IntoParams, Validate)]
pub struct TaskTypeQuery {
    #[serde(rename = "type")]
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "type must not be empty"
    ))]
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
