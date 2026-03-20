use base64::Engine as _;
use image::GenericImageView;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, instrument, warn};

use slab_core::api::{ImageGenerationRequest, JsonOptions};
use slab_proto::slab::ipc::v1 as pb;

use super::{extract_request_id, runtime_to_status, BackendKind, GrpcServiceImpl};

#[tonic::async_trait]
impl pb::diffusion_service_server::DiffusionService for GrpcServiceImpl {
    #[instrument(skip_all, fields(request_id, backend = "ggml.diffusion"))]
    async fn generate_image(
        &self,
        request: Request<pb::ImageRequest>,
    ) -> Result<Response<pb::ImageResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let req = request.into_inner();
        if req.prompt.trim().is_empty() {
            warn!("generate_image rejected: prompt is empty");
            return Err(Status::invalid_argument("prompt must not be empty"));
        }
        if req.sample_steps < 0 {
            return Err(Status::invalid_argument("sample_steps must be >= 0"));
        }

        debug!(
            prompt_len = req.prompt.len(),
            n = req.n,
            width = req.width,
            height = req.height,
            "diffusion generate_image request received"
        );

        let pipeline = self.pipeline_for_backend(BackendKind::Diffusion).await?;
        let response = pipeline
            .run_image_generation(build_image_generation_request(&req))
            .await
            .map_err(|error| {
                error!(error = %error, "diffusion image generation failed");
                runtime_to_status(error)
            })?;

        let images_json = encode_png_image_payload(&response.images)?;
        info!(
            images_json_bytes = images_json.len(),
            image_count = response.images.len(),
            "diffusion image generation completed"
        );
        Ok(Response::new(pb::ImageResponse { images_json }))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.diffusion"))]
    async fn generate_video(
        &self,
        request: Request<pb::VideoRequest>,
    ) -> Result<Response<pb::VideoResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let req = request.into_inner();
        if req.prompt.trim().is_empty() {
            warn!("generate_video rejected: prompt is empty");
            return Err(Status::invalid_argument("prompt must not be empty"));
        }
        if req.sample_steps < 0 {
            return Err(Status::invalid_argument("sample_steps must be >= 0"));
        }

        debug!(
            prompt_len = req.prompt.len(),
            video_frames = req.video_frames,
            "diffusion generate_video request received"
        );

        let pipeline = self.pipeline_for_backend(BackendKind::Diffusion).await?;
        let response = pipeline
            .run_image_generation(build_video_generation_request(&req))
            .await
            .map_err(|error| {
                error!(error = %error, "diffusion video generation failed");
                runtime_to_status(error)
            })?;

        let frames_json = encode_raw_frame_payload(&response.images)?;
        info!(
            frames_json_bytes = frames_json.len(),
            frame_count = response.images.len(),
            "diffusion video generation completed"
        );
        Ok(Response::new(pb::VideoResponse { frames_json }))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.diffusion"))]
    async fn load_model(
        &self,
        request: Request<pb::ModelLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        debug!("diffusion load_model request received");
        let status = self
            .load_model_for_backend(BackendKind::Diffusion, request.into_inner())
            .await?;
        Ok(Response::new(status))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.diffusion"))]
    async fn unload_model(
        &self,
        request: Request<pb::ModelUnloadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        debug!("diffusion unload_model request received");
        let _ = request.into_inner();
        let status = self.unload_model_for_backend(BackendKind::Diffusion).await?;
        Ok(Response::new(status))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.diffusion"))]
    async fn reload_library(
        &self,
        request: Request<pb::ReloadLibraryRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        debug!("diffusion reload_library request received");
        let status = self
            .reload_library_for_backend(BackendKind::Diffusion, request.into_inner())
            .await?;
        Ok(Response::new(status))
    }
}

fn build_image_generation_request(req: &pb::ImageRequest) -> ImageGenerationRequest {
    let mut options = JsonOptions::default();
    options.insert("batch_count".to_owned(), serde_json::json!(req.n.max(1)));
    options.insert("cfg_scale".to_owned(), serde_json::json!(req.cfg_scale));
    options.insert("clip_skip".to_owned(), serde_json::json!(req.clip_skip));
    options.insert("strength".to_owned(), serde_json::json!(req.strength));
    options.insert("eta".to_owned(), serde_json::json!(req.eta));

    if !req.sample_method.is_empty() {
        options.insert(
            "sample_method".to_owned(),
            serde_json::json!(req.sample_method),
        );
    }
    if !req.scheduler.is_empty() {
        options.insert("scheduler".to_owned(), serde_json::json!(req.scheduler));
    }
    insert_init_image_options(
        &mut options,
        &req.init_image_data,
        req.init_image_width,
        req.init_image_height,
        req.init_image_channels,
    );

    ImageGenerationRequest {
        prompt: req.prompt.clone(),
        negative_prompt: (!req.negative_prompt.is_empty()).then_some(req.negative_prompt.clone()),
        width: req.width.max(1),
        height: req.height.max(1),
        steps: req.sample_steps.max(1) as u32,
        guidance: req.guidance,
        seed: Some(req.seed),
        options,
    }
}

fn build_video_generation_request(req: &pb::VideoRequest) -> ImageGenerationRequest {
    let mut options = JsonOptions::default();
    options.insert(
        "batch_count".to_owned(),
        serde_json::json!(req.video_frames.max(1)),
    );
    options.insert("cfg_scale".to_owned(), serde_json::json!(req.cfg_scale));
    options.insert("strength".to_owned(), serde_json::json!(req.strength));
    options.insert("fps".to_owned(), serde_json::json!(req.fps));

    if !req.sample_method.is_empty() {
        options.insert(
            "sample_method".to_owned(),
            serde_json::json!(req.sample_method),
        );
    }
    if !req.scheduler.is_empty() {
        options.insert("scheduler".to_owned(), serde_json::json!(req.scheduler));
    }
    insert_init_image_options(
        &mut options,
        &req.init_image_data,
        req.init_image_width,
        req.init_image_height,
        req.init_image_channels,
    );

    ImageGenerationRequest {
        prompt: req.prompt.clone(),
        negative_prompt: (!req.negative_prompt.is_empty()).then_some(req.negative_prompt.clone()),
        width: req.width.max(1),
        height: req.height.max(1),
        steps: req.sample_steps.max(1) as u32,
        guidance: req.guidance,
        seed: Some(req.seed),
        options,
    }
}

fn insert_init_image_options(
    options: &mut JsonOptions,
    init_image_data: &[u8],
    width: u32,
    height: u32,
    channels: u32,
) {
    if init_image_data.is_empty() {
        return;
    }

    options.insert(
        "init_image_b64".to_owned(),
        serde_json::json!(base64::engine::general_purpose::STANDARD.encode(init_image_data)),
    );
    options.insert("init_image_width".to_owned(), serde_json::json!(width));
    options.insert("init_image_height".to_owned(), serde_json::json!(height));
    options.insert(
        "init_image_channels".to_owned(),
        serde_json::json!(channels.max(1)),
    );
}

fn encode_png_image_payload(images: &[Vec<u8>]) -> Result<Vec<u8>, Status> {
    let encoded: Vec<serde_json::Value> = images
        .iter()
        .map(|image_bytes| {
            let decoded = image::load_from_memory(image_bytes).map_err(|error| {
                Status::internal(format!("failed to decode generated image: {error}"))
            })?;
            let (width, height) = decoded.dimensions();
            let channels = decoded.color().channel_count();

            Ok(serde_json::json!({
                "b64": base64::engine::general_purpose::STANDARD.encode(image_bytes),
                "width": width,
                "height": height,
                "channels": channels,
            }))
        })
        .collect::<Result<_, Status>>()?;

    serde_json::to_vec(&encoded).map_err(|error| {
        Status::internal(format!("failed to encode generated image JSON: {error}"))
    })
}

fn encode_raw_frame_payload(images: &[Vec<u8>]) -> Result<Vec<u8>, Status> {
    let encoded: Vec<serde_json::Value> = images
        .iter()
        .map(|image_bytes| {
            let decoded = image::load_from_memory(image_bytes).map_err(|error| {
                Status::internal(format!("failed to decode generated frame: {error}"))
            })?;
            let (width, height) = decoded.dimensions();

            let (raw, channels) = if decoded.color().channel_count() == 4 {
                (decoded.to_rgba8().into_raw(), 4u8)
            } else {
                (decoded.to_rgb8().into_raw(), 3u8)
            };

            Ok(serde_json::json!({
                "b64": base64::engine::general_purpose::STANDARD.encode(raw),
                "width": width,
                "height": height,
                "channels": channels,
            }))
        })
        .collect::<Result<_, Status>>()?;

    serde_json::to_vec(&encoded).map_err(|error| {
        Status::internal(format!("failed to encode generated frame JSON: {error}"))
    })
}
