use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts, Json, Query, Request};
use serde::de::DeserializeOwned;
use std::str::FromStr;
use validator::{Validate, ValidationError};

use crate::domain::models::BackendId;
use crate::error::ServerError;

const ALLOWED_FFMPEG_OUTPUT_FORMATS: &[&str] = &[
    "mp3", "mp4", "wav", "flac", "ogg", "opus", "webm", "avi", "mkv", "mov", "aac", "m4a", "m4v",
    "f32le", "pcm",
];

pub struct ValidatedJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate + Send,
    Json<T>: FromRequest<S, Rejection = JsonRejection>,
{
    type Rejection = ServerError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(map_json_rejection)?;
        Ok(Self(validate(value)?))
    }
}

pub struct ValidatedQuery<T>(pub T);

impl<S, T> FromRequestParts<S> for ValidatedQuery<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate + Send,
    Query<T>: FromRequestParts<S, Rejection = QueryRejection>,
{
    type Rejection = ServerError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(map_query_rejection)?;
        Ok(Self(validate(value)?))
    }
}

pub fn validate<T>(value: T) -> Result<T, ServerError>
where
    T: Validate,
{
    value.validate()?;
    Ok(value)
}

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

    if path
        .components()
        .any(|component| component == std::path::Component::ParentDir)
    {
        return Err(ValidationError::new("path_traversal"));
    }

    Ok(())
}
pub fn validate_backend_id(value: &str) -> Result<(), ValidationError> {
    validate_non_blank(value)?;
    BackendId::from_str(value)
        .map(|_| ())
        .map_err(|_| ValidationError::new("backend_id"))
}

pub fn validate_chat_role(value: &str) -> Result<(), ValidationError> {
    validate_non_blank(value)?;

    if matches!(value, "system" | "user" | "assistant") {
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

fn map_json_rejection(rejection: JsonRejection) -> ServerError {
    ServerError::BadRequest(rejection.body_text())
}

fn map_query_rejection(rejection: QueryRejection) -> ServerError {
    ServerError::BadRequest(rejection.body_text())
}
