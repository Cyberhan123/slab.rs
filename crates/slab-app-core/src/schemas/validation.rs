//! Shared validation functions used across all schema modules.
//!
//! These functions are designed to work with the [`validator`] crate's
//! `#[validate(custom(function = "..."))]` attribute.

use slab_types::RuntimeBackendId;
use std::str::FromStr;
use validator::ValidationError;

const ALLOWED_FFMPEG_OUTPUT_FORMATS: &[&str] = &[
    "mp3", "mp4", "wav", "flac", "ogg", "opus", "webm", "avi", "mkv", "mov", "aac", "m4a", "m4v",
    "f32le", "pcm",
];

pub fn validate_non_blank(value: &str) -> Result<(), ValidationError> {
    if value.trim().is_empty() {
        return Err(ValidationError::new("blank"));
    }
    Ok(())
}

pub fn validate_absolute_path(value: &str) -> Result<(), ValidationError> {
    if value.trim().is_empty() {
        return Err(ValidationError::new("blank"));
    }

    let path = std::path::Path::new(value);
    if !path.is_absolute() {
        return Err(ValidationError::new("absolute_path"));
    }

    if path.components().any(|component| component == std::path::Component::ParentDir) {
        return Err(ValidationError::new("path_traversal"));
    }

    Ok(())
}

pub fn validate_positive_u32(value: u32) -> Result<(), ValidationError> {
    if value == 0 {
        return Err(ValidationError::new("positive_u32"));
    }

    Ok(())
}

pub fn validate_backend_id(value: &str) -> Result<(), ValidationError> {
    validate_non_blank(value)?;
    RuntimeBackendId::from_str(value).map(|_| ()).map_err(|_| ValidationError::new("backend_id"))
}

pub fn validate_chat_role(value: &str) -> Result<(), ValidationError> {
    validate_non_blank(value)?;

    if matches!(value, "system" | "developer" | "user" | "assistant" | "tool" | "function") {
        Ok(())
    } else {
        Err(ValidationError::new("chat_role"))
    }
}

pub fn validate_ffmpeg_output_format(value: &str) -> Result<(), ValidationError> {
    validate_non_blank(value)?;

    if ALLOWED_FFMPEG_OUTPUT_FORMATS.contains(&value.trim().to_ascii_lowercase().as_str()) {
        Ok(())
    } else {
        Err(ValidationError::new("ffmpeg_output_format"))
    }
}
