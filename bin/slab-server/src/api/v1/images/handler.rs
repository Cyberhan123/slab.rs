use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::v1::images::schema::{
    ImageGenerationRequest, ImageGenerationTaskResponse, ImageMode,
};
use crate::api::v1::tasks::schema::OperationAcceptedResponse;
use crate::api::validation::ValidatedJson;
use crate::error::ServerError;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::ImageService;

#[derive(OpenApi)]
#[openapi(
    paths(
        generate_images,
        list_image_generations,
        get_image_generation,
        get_image_generation_artifact,
        get_image_generation_reference
    ),
    components(schemas(
        ImageGenerationRequest,
        ImageGenerationTaskResponse,
        ImageMode,
        OperationAcceptedResponse
    ))
)]
pub struct ImagesApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/images/generations", post(generate_images).get(list_image_generations))
        .route("/images/generations/{id}", axum::routing::get(get_image_generation))
        .route(
            "/images/generations/{id}/artifacts/{index}",
            axum::routing::get(get_image_generation_artifact),
        )
        .route(
            "/images/generations/{id}/reference",
            axum::routing::get(get_image_generation_reference),
        )
}

#[utoipa::path(
    post,
    path = "/v1/images/generations",
    tag = "images",
    request_body = ImageGenerationRequest,
    responses(
        (status = 202, description = "Task accepted", body = OperationAcceptedResponse),
        (status = 400, description = "Bad request (invalid parameters)"),
        (status = 500, description = "Backend error"),
    )
)]
async fn generate_images(
    State(service): State<ImageService>,
    ValidatedJson(req): ValidatedJson<ImageGenerationRequest>,
) -> Result<(StatusCode, Json<OperationAcceptedResponse>), ServerError> {
    let response = service.generate_images(req.try_into()?).await?;
    Ok((StatusCode::ACCEPTED, Json(response.into())))
}

#[utoipa::path(
    get,
    path = "/v1/images/generations",
    tag = "images",
    responses(
        (status = 200, description = "Image generation tasks listed", body = [ImageGenerationTaskResponse]),
        (status = 500, description = "Backend error"),
    )
)]
async fn list_image_generations(
    State(service): State<ImageService>,
) -> Result<Json<Vec<ImageGenerationTaskResponse>>, ServerError> {
    Ok(Json(service.list_generation_tasks().await?.into_iter().map(Into::into).collect()))
}

#[utoipa::path(
    get,
    path = "/v1/images/generations/{id}",
    tag = "images",
    params(("id" = String, Path, description = "Image generation task ID")),
    responses(
        (status = 200, description = "Image generation task detail", body = ImageGenerationTaskResponse),
        (status = 404, description = "Task not found"),
        (status = 500, description = "Backend error"),
    )
)]
async fn get_image_generation(
    State(service): State<ImageService>,
    Path(id): Path<String>,
) -> Result<Json<ImageGenerationTaskResponse>, ServerError> {
    Ok(Json(service.get_generation_task(&id).await?.into()))
}

#[utoipa::path(
    get,
    path = "/v1/images/generations/{id}/artifacts/{index}",
    tag = "images",
    params(
        ("id" = String, Path, description = "Image generation task ID"),
        ("index" = usize, Path, description = "Artifact index")
    ),
    responses(
        (status = 200, description = "Image artifact bytes"),
        (status = 404, description = "Artifact not found"),
        (status = 500, description = "Backend error"),
    )
)]
async fn get_image_generation_artifact(
    State(service): State<ImageService>,
    Path((id, index)): Path<(String, usize)>,
) -> Result<impl IntoResponse, ServerError> {
    let bytes = service.read_generated_artifact(&id, index).await?;
    Ok((StatusCode::OK, [(CONTENT_TYPE, "image/png"), (CACHE_CONTROL, "no-store")], bytes))
}

#[utoipa::path(
    get,
    path = "/v1/images/generations/{id}/reference",
    tag = "images",
    params(("id" = String, Path, description = "Image generation task ID")),
    responses(
        (status = 200, description = "Reference image bytes"),
        (status = 404, description = "Reference image not found"),
        (status = 500, description = "Backend error"),
    )
)]
async fn get_image_generation_reference(
    State(service): State<ImageService>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServerError> {
    let bytes = service.read_reference_image(&id).await?;
    Ok((StatusCode::OK, [(CONTENT_TYPE, "image/png"), (CACHE_CONTROL, "no-store")], bytes))
}
