use std::sync::Arc;

use base64::Engine as _;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::validation::ValidatedJson;
use crate::api::v1::images::schema::{ImageGenerationRequest, ImageMode};
use crate::api::v1::tasks::schema::OperationAcceptedResponse;
use crate::context::AppState;
use crate::domain::services::to_operation_accepted_response;
use crate::error::ServerError;
use crate::services::images::{
    DecodedImageInput, ImageGenerationCommand, ImageGenerationMode, ImagesService,
};

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
    State(service): State<ImagesService>,
    ValidatedJson(req): ValidatedJson<ImageGenerationRequest>,
) -> Result<(StatusCode, Json<OperationAcceptedResponse>), ServerError> {
    let response = service.generate_images(to_image_command(req)?).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(to_operation_accepted_response(response)),
    ))
}

fn to_image_command(request: ImageGenerationRequest) -> Result<ImageGenerationCommand, ServerError> {
    let mode = match request.mode {
        ImageMode::Txt2Img => ImageGenerationMode::Txt2Img,
        ImageMode::Img2Img => ImageGenerationMode::Img2Img,
    };
    let init_image = match mode {
        ImageGenerationMode::Txt2Img => None,
        ImageGenerationMode::Img2Img => request
            .init_image
            .as_deref()
            .map(decode_init_image)
            .transpose()?,
    };

    Ok(ImageGenerationCommand {
        model: request.model,
        prompt: request.prompt,
        negative_prompt: request.negative_prompt,
        n: request.n,
        width: request.width,
        height: request.height,
        cfg_scale: request.cfg_scale,
        guidance: request.guidance,
        steps: request.steps,
        seed: request.seed,
        sample_method: request.sample_method,
        scheduler: request.scheduler,
        clip_skip: request.clip_skip,
        eta: request.eta,
        strength: request.strength,
        init_image,
        mode,
    })
}

fn decode_init_image(data_uri: &str) -> Result<DecodedImageInput, ServerError> {
    let b64 = if let Some(pos) = data_uri.find("base64,") {
        &data_uri[pos + "base64,".len()..]
    } else {
        data_uri
    };

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|error| {
            ServerError::BadRequest(format!("init_image base64 decode failed: {error}"))
        })?;

    let image = image::load_from_memory(&bytes)
        .map_err(|error| ServerError::BadRequest(format!("init_image decode failed: {error}")))?;

    let rgb = image.to_rgb8();
    let (width, height) = rgb.dimensions();
    Ok(DecodedImageInput {
        data: rgb.into_raw(),
        width,
        height,
        channels: 3,
    })
}
