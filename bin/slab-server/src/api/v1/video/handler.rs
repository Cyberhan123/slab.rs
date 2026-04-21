use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::v1::tasks::schema::OperationAcceptedResponse;
use crate::api::v1::video::schema::{VideoGenerationRequest, VideoGenerationTaskResponse};
use crate::api::validation::ValidatedJson;
use crate::error::ServerError;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::VideoService;

#[derive(OpenApi)]
#[openapi(
    paths(
        generate_video,
        list_video_generations,
        get_video_generation,
        get_video_generation_artifact,
        get_video_generation_reference
    ),
    components(schemas(
        VideoGenerationRequest,
        VideoGenerationTaskResponse,
        OperationAcceptedResponse
    ))
)]
pub struct VideoApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/video/generations", post(generate_video).get(list_video_generations))
        .route("/video/generations/{id}", axum::routing::get(get_video_generation))
        .route(
            "/video/generations/{id}/artifact",
            axum::routing::get(get_video_generation_artifact),
        )
        .route(
            "/video/generations/{id}/reference",
            axum::routing::get(get_video_generation_reference),
        )
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

#[utoipa::path(
    get,
    path = "/v1/video/generations",
    tag = "video",
    responses(
        (status = 200, description = "Video generation tasks listed", body = [VideoGenerationTaskResponse]),
        (status = 500, description = "Backend error"),
    )
)]
async fn list_video_generations(
    State(service): State<VideoService>,
) -> Result<Json<Vec<VideoGenerationTaskResponse>>, ServerError> {
    Ok(Json(service.list_generation_tasks().await?.into_iter().map(Into::into).collect()))
}

#[utoipa::path(
    get,
    path = "/v1/video/generations/{id}",
    tag = "video",
    params(("id" = String, Path, description = "Video generation task ID")),
    responses(
        (status = 200, description = "Video generation task detail", body = VideoGenerationTaskResponse),
        (status = 404, description = "Task not found"),
        (status = 500, description = "Backend error"),
    )
)]
async fn get_video_generation(
    State(service): State<VideoService>,
    Path(id): Path<String>,
) -> Result<Json<VideoGenerationTaskResponse>, ServerError> {
    Ok(Json(service.get_generation_task(&id).await?.into()))
}

#[utoipa::path(
    get,
    path = "/v1/video/generations/{id}/artifact",
    tag = "video",
    params(("id" = String, Path, description = "Video generation task ID")),
    responses(
        (status = 200, description = "Generated MP4 bytes"),
        (status = 404, description = "Artifact not found"),
        (status = 500, description = "Backend error"),
    )
)]
async fn get_video_generation_artifact(
    State(service): State<VideoService>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServerError> {
    let bytes = service.read_generated_video(&id).await?;
    Ok((StatusCode::OK, [(CONTENT_TYPE, "video/mp4"), (CACHE_CONTROL, "no-store")], bytes))
}

#[utoipa::path(
    get,
    path = "/v1/video/generations/{id}/reference",
    tag = "video",
    params(("id" = String, Path, description = "Video generation task ID")),
    responses(
        (status = 200, description = "Reference image bytes"),
        (status = 404, description = "Reference image not found"),
        (status = 500, description = "Backend error"),
    )
)]
async fn get_video_generation_reference(
    State(service): State<VideoService>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServerError> {
    let bytes = service.read_reference_image(&id).await?;
    Ok((StatusCode::OK, [(CONTENT_TYPE, "image/png"), (CACHE_CONTROL, "no-store")], bytes))
}
