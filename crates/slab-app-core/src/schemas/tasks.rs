use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::domain::models::{
    AcceptedOperation, TaskProgress, TaskResult, TaskStatus as DomainTaskStatus, TaskView,
    TimedTextSegment,
};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OperationAcceptedResponse {
    pub operation_id: String,
}

/// Result payload returned by `GET /v1/tasks/{id}/result`.
///
/// Fields are populated depending on the task type:
/// - Single-image tasks: `image` contains a `data:image/png;base64,…` data URI.
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
    /// Absolute output path for file-producing utility tasks such as FFmpeg conversion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    /// Text content, present for `whisper` and other text-producing task results.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Timed text segments, present for Whisper transcriptions with timestamps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<TimedTextSegmentResponse>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TimedTextSegmentResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Deserialize, ToSchema, IntoParams, Validate)]
pub struct TaskTypeQuery {
    #[serde(rename = "type")]
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "type must not be empty"
    ))]
    pub task_type: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct TaskResponse {
    pub id: String,
    pub task_type: String,
    pub status: TaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<TaskProgressResponse>,
    pub error_msg: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaskProgressResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub current: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logs: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

impl From<AcceptedOperation> for OperationAcceptedResponse {
    fn from(result: AcceptedOperation) -> Self {
        Self { operation_id: result.operation_id }
    }
}

impl From<TaskResult> for TaskResultPayload {
    fn from(result: TaskResult) -> Self {
        Self {
            image: result.image,
            images: result.images,
            video_path: result.video_path,
            output_path: result.output_path,
            text: result.text,
            segments: result
                .segments
                .map(|segments| segments.into_iter().map(Into::into).collect()),
        }
    }
}

impl From<TimedTextSegment> for TimedTextSegmentResponse {
    fn from(segment: TimedTextSegment) -> Self {
        Self { start_ms: segment.start_ms, end_ms: segment.end_ms, text: segment.text }
    }
}

impl From<TaskView> for TaskResponse {
    fn from(view: TaskView) -> Self {
        Self {
            id: view.id,
            task_type: view.task_type,
            status: view.status.into(),
            progress: view.progress.map(Into::into),
            error_msg: view.error_msg,
            created_at: view.created_at,
            updated_at: view.updated_at,
        }
    }
}

impl From<TaskProgress> for TaskProgressResponse {
    fn from(progress: TaskProgress) -> Self {
        Self {
            label: progress.label,
            message: progress.message,
            current: progress.current,
            total: progress.total,
            unit: progress.unit,
            step: progress.step,
            step_count: progress.step_count,
            logs: progress.logs,
        }
    }
}

impl From<DomainTaskStatus> for TaskStatus {
    fn from(value: DomainTaskStatus) -> Self {
        match value {
            DomainTaskStatus::Pending => Self::Pending,
            DomainTaskStatus::Running => Self::Running,
            DomainTaskStatus::Succeeded => Self::Succeeded,
            DomainTaskStatus::Failed => Self::Failed,
            DomainTaskStatus::Cancelled => Self::Cancelled,
            DomainTaskStatus::Interrupted => Self::Interrupted,
        }
    }
}
