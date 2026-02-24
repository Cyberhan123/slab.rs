use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConvertRequest {
    /// Absolute path to the source file.
    pub source_path: String,
    /// Desired output format (e.g. `"mp3"`, `"wav"`, `"mp4"`).
    pub output_format: String,
    /// Optional output path; defaults to source path with new extension.
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConvertResponse {
    pub task_id: String,
}
