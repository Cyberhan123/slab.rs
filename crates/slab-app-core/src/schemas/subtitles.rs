use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::{Validate, ValidationError};

use crate::domain::models::RenderSubtitleCommand;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SubtitleFormatRequest {
    Srt,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SubtitleVariantRequest {
    Source,
    Translated,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
#[validate(schema(function = "validate_subtitle_entry"))]
pub struct SubtitleEntryRequest {
    pub start_ms: u64,
    pub end_ms: u64,
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "text must not be empty"
    ))]
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct RenderSubtitleRequest {
    /// Absolute path to the source video/audio file used for default output naming.
    #[validate(custom(
        function = "crate::schemas::validation::validate_absolute_path",
        message = "source_path must be an absolute path without '..'"
    ))]
    pub source_path: String,
    pub variant: SubtitleVariantRequest,
    pub format: SubtitleFormatRequest,
    #[validate(length(min = 1, message = "entries must not be empty"))]
    #[validate(nested)]
    pub entries: Vec<SubtitleEntryRequest>,
    /// Optional absolute output path. Defaults to `<source_stem>.<variant>.srt`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[validate(custom(
        function = "crate::schemas::validation::validate_optional_absolute_path",
        message = "output_path must be an absolute path without '..'"
    ))]
    pub output_path: Option<String>,
    /// Whether an existing output file should be overwritten. Defaults to true.
    #[serde(default = "default_overwrite")]
    pub overwrite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RenderSubtitleResponse {
    pub output_path: String,
    pub format: String,
    pub entry_count: usize,
}

impl From<RenderSubtitleRequest> for RenderSubtitleCommand {
    fn from(request: RenderSubtitleRequest) -> Self {
        Self {
            source_path: request.source_path,
            variant: request.variant.into(),
            entries: request.entries.into_iter().map(Into::into).collect(),
            output_path: request.output_path,
            overwrite: request.overwrite,
        }
    }
}

impl From<SubtitleVariantRequest> for crate::domain::models::SubtitleVariant {
    fn from(value: SubtitleVariantRequest) -> Self {
        match value {
            SubtitleVariantRequest::Source => Self::Source,
            SubtitleVariantRequest::Translated => Self::Translated,
        }
    }
}

impl From<SubtitleEntryRequest> for crate::domain::models::RenderSubtitleEntry {
    fn from(value: SubtitleEntryRequest) -> Self {
        Self { start_ms: value.start_ms, end_ms: value.end_ms, text: value.text }
    }
}

fn default_overwrite() -> bool {
    true
}

fn validate_subtitle_entry(entry: &SubtitleEntryRequest) -> Result<(), ValidationError> {
    if entry.end_ms <= entry.start_ms {
        let mut error = ValidationError::new("invalid_timespan");
        error.message = Some("end_ms must be greater than start_ms".into());
        return Err(error);
    }

    Ok(())
}
