use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::v1::tasks::schema::OperationAcceptedResponse;
use crate::api::v1::video::schema::VideoGenerationRequest;
use crate::api::validation::ValidatedJson;
use crate::error::ServerError;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::VideoService;

#[derive(OpenApi)]
#[openapi(
    paths(generate_video),
    components(schemas(VideoGenerationRequest, OperationAcceptedResponse))
)]
pub struct VideoApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/video/generations", post(generate_video))
}

#[utoipa::path(
    post,
    path = "/v1/video/generations",
    tag = "video",
    request_body = VideoGenerationRequest,
    responses(
        (status = 202, description = "Task accepted", body = OperationAcceptedResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn generate_video(
    State(service): State<VideoService>,
    ValidatedJson(req): ValidatedJson<VideoGenerationRequest>,
) -> Result<(StatusCode, Json<OperationAcceptedResponse>), ServerError> {
    let response = service.generate_video(req.try_into()?).await?;
    Ok((StatusCode::ACCEPTED, Json(response.into())))
}
