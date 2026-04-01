use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct ConvertRequest {
    /// Absolute path to the source file.
    #[validate(custom(
        function = "crate::api::validation::validate_absolute_path",
        message = "source_path must be an absolute path without '..'"
    ))]
    pub source_path: String,
    /// Desired output format (e.g. `"mp3"`, `"wav"`, `"mp4"`).
    #[validate(custom(
        function = "crate::api::validation::validate_ffmpeg_output_format",
        message = "output_format is unsupported"
    ))]
    pub output_format: String,
    /// Optional output path; defaults to source path with new extension.
    #[validate(custom(
        function = "crate::api::validation::validate_absolute_path",
        message = "output_path must be an absolute path without '..'"
    ))]
    pub output_path: Option<String>,
}

use slab_app_core::domain::models::FfmpegConvertCommand;

impl From<ConvertRequest> for FfmpegConvertCommand {
    fn from(request: ConvertRequest) -> Self {
        Self {
            source_path: request.source_path,
            output_format: request.output_format,
            output_path: request.output_path,
        }
    }
}
