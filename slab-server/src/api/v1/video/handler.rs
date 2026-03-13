use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use base64::Engine as _;
use utoipa::OpenApi;

use crate::api::v1::tasks::schema::OperationAcceptedResponse;
use crate::api::v1::video::schema::VideoGenerationRequest;
use crate::api::validation::ValidatedJson;
use crate::context::AppState;
use crate::domain::models::{DecodedVideoInitImage, VideoGenerationCommand};
use crate::domain::services::to_operation_accepted_response;
use crate::domain::services::VideoService;
use crate::error::ServerError;

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
    let response = service.generate_video(to_video_command(req)?).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(to_operation_accepted_response(response)),
    ))
}

fn to_video_command(
    request: VideoGenerationRequest,
) -> Result<VideoGenerationCommand, ServerError> {
    let init_image = request
        .init_image
        .as_deref()
        .map(decode_init_image)
        .transpose()?;

    Ok(VideoGenerationCommand {
        model: request.model,
        prompt: request.prompt,
        negative_prompt: request.negative_prompt,
        width: request.width,
        height: request.height,
        video_frames: request.video_frames,
        fps: request.fps,
        cfg_scale: request.cfg_scale,
        guidance: request.guidance,
        steps: request.steps,
        seed: request.seed,
        sample_method: request.sample_method,
        scheduler: request.scheduler,
        init_image,
        strength: request.strength,
    })
}

fn decode_init_image(data_uri: &str) -> Result<DecodedVideoInitImage, ServerError> {
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
    Ok(DecodedVideoInitImage {
        data: rgb.into_raw(),
        width,
        height,
        channels: 3,
    })
}
