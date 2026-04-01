use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::v1::images::schema::{ImageGenerationRequest, ImageMode};
use crate::api::v1::tasks::schema::OperationAcceptedResponse;
use crate::api::validation::ValidatedJson;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::ImageService;
use crate::error::ServerError;

#[derive(OpenApi)]
#[openapi(
    paths(generate_images),
    components(schemas(ImageGenerationRequest, ImageMode, OperationAcceptedResponse))
)]
pub struct ImagesApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/images/generations", post(generate_images))
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
