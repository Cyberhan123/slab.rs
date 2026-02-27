use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Audio transcription request using file upload
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompletionRequestUpload {
    /// The audio file to transcribe (multipart form data)
    #[schema(format = "binary")]
    pub file: String,
}

/// Response for successful transcription request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TranscriptionResponse {
    /// The task ID for tracking transcription progress
    pub task_id: String,
}

/// Legacy path-based request (deprecated - use file upload instead)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompletionRequest {
    /// The audio file path to transcribe (DEPRECATED: use multipart file upload instead)
    #[serde(rename = "path")]
    pub path: String,
}
