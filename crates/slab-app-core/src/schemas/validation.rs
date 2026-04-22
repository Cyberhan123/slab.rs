//! Shared validation functions used across all schema modules.
//!
//! These functions are designed to work with the [`validator`] crate's
//! `#[validate(custom(function = "..."))]` attribute.

use slab_types::RuntimeBackendId;
use std::str::FromStr;
use validator::ValidationError;

use crate::domain::models::ManagedModelBackendId;

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

pub fn validate_optional_absolute_path(value: &str) -> Result<(), ValidationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    validate_absolute_path(trimmed)
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

pub fn validate_managed_model_backend_id(value: &str) -> Result<(), ValidationError> {
    validate_non_blank(value)?;
    ManagedModelBackendId::from_str(value)
        .map(|_| ())
        .map_err(|_| ValidationError::new("backend_id"))
}

pub fn validate_optional_managed_model_backend_id(
    value: &Option<String>,
) -> Result<(), ValidationError> {
    match value.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => validate_managed_model_backend_id(value),
        None => Ok(()),
    }
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

    let trimmed = value.trim();
    if ALLOWED_FFMPEG_OUTPUT_FORMATS.iter().any(|fmt| fmt.eq_ignore_ascii_case(trimmed)) {
        Ok(())
    } else {
        Err(ValidationError::new("ffmpeg_output_format"))
    }
}
