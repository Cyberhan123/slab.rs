use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts, Json, Query, Request};
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::error::ServerError;

// Re-export shared validation functions from slab-app-core.
#[allow(unused_imports)]
pub use slab_app_core::schemas::validation::{
    validate_absolute_path, validate_backend_id, validate_chat_role, validate_ffmpeg_output_format,
    validate_non_blank, validate_positive_u32,
};

pub struct ValidatedJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate + Send,
    Json<T>: FromRequest<S, Rejection = JsonRejection>,
{
    type Rejection = ServerError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state).await.map_err(map_json_rejection)?;
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
        let Query(value) =
            Query::<T>::from_request_parts(parts, state).await.map_err(map_query_rejection)?;
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

fn map_json_rejection(rejection: JsonRejection) -> ServerError {
    ServerError::BadRequest(rejection.body_text())
}

fn map_query_rejection(rejection: QueryRejection) -> ServerError {
    ServerError::BadRequest(rejection.body_text())
}
