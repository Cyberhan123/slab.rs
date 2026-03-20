use base64::Engine as _;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, instrument};

use slab_core::api::ImageGenerationRequest;
use slab_proto::{convert, slab::ipc::v1 as pb};
use slab_types::diffusion::{DiffusionImageRequest, DiffusionVideoRequest};
use slab_types::media::RawImageInput;

use super::{extract_request_id, proto_to_status, runtime_to_status, BackendKind, GrpcServiceImpl};

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
        let request = convert::decode_diffusion_image_request(&req).map_err(proto_to_status)?;

        debug!(
            prompt_len = request.prompt.len(),
            n = request.count,
            width = request.width,
            height = request.height,
            "diffusion generate_image request received"
        );

        let pipeline = self.pipeline_for_backend(BackendKind::Diffusion).await?;
        let generated = pipeline
            .run_image_generation(build_image_generation_request(&request))
            .await
            .map_err(|error| {
                error!(error = %error, "diffusion image generation failed");
                runtime_to_status(error)
            })?;

        let response =
            convert::diffusion_image_response_from_generated(&generated).map_err(|error| {
                Status::internal(format!("failed to normalize generated image response: {error}"))
            })?;
        let grpc_response =
            convert::encode_diffusion_image_response(&response).map_err(|error| {
                Status::internal(format!("failed to encode generated image response: {error}"))
            })?;
        info!(
            images_json_bytes = grpc_response.images_json.len(),
            image_count = response.images.len(),
            "diffusion image generation completed"
        );
        Ok(Response::new(grpc_response))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.diffusion"))]
    async fn generate_video(
        &self,
        request: Request<pb::VideoRequest>,
    ) -> Result<Response<pb::VideoResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let req = request.into_inner();
        let request = convert::decode_diffusion_video_request(&req).map_err(proto_to_status)?;

        debug!(
            prompt_len = request.prompt.len(),
            video_frames = request.video_frames,
            "diffusion generate_video request received"
        );

        let pipeline = self.pipeline_for_backend(BackendKind::Diffusion).await?;
        let generated = pipeline
            .run_image_generation(build_video_generation_request(&request))
            .await
            .map_err(|error| {
                error!(error = %error, "diffusion video generation failed");
                runtime_to_status(error)
            })?;

        let response =
            convert::diffusion_video_response_from_generated(&generated).map_err(|error| {
                Status::internal(format!("failed to normalize generated video response: {error}"))
            })?;
        let grpc_response =
            convert::encode_diffusion_video_response(&response).map_err(|error| {
                Status::internal(format!("failed to encode generated video response: {error}"))
            })?;
        info!(
            frames_json_bytes = grpc_response.frames_json.len(),
            frame_count = response.frames.len(),
            "diffusion video generation completed"
        );
        Ok(Response::new(grpc_response))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.diffusion"))]
    async fn load_model(
        &self,
        request: Request<pb::ModelLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        debug!("diffusion load_model request received");
        let status =
            self.load_model_for_backend(BackendKind::Diffusion, request.into_inner()).await?;
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
        let status =
            self.reload_library_for_backend(BackendKind::Diffusion, request.into_inner()).await?;
        Ok(Response::new(status))
    }
}

fn build_image_generation_request(req: &DiffusionImageRequest) -> ImageGenerationRequest {
    let mut options = req.options.clone();
    options.insert("batch_count".to_owned(), serde_json::json!(req.count.max(1)));

    if let Some(cfg_scale) = req.cfg_scale {
        options.insert("cfg_scale".to_owned(), serde_json::json!(cfg_scale));
    }
    if let Some(clip_skip) = req.clip_skip {
        options.insert("clip_skip".to_owned(), serde_json::json!(clip_skip));
    }
    if let Some(strength) = req.strength {
        options.insert("strength".to_owned(), serde_json::json!(strength));
    }
    if let Some(eta) = req.eta {
        options.insert("eta".to_owned(), serde_json::json!(eta));
    }
    if let Some(sample_method) = req.sample_method.as_ref() {
        options.insert("sample_method".to_owned(), serde_json::json!(sample_method));
    }
    if let Some(scheduler) = req.scheduler.as_ref() {
        options.insert("scheduler".to_owned(), serde_json::json!(scheduler));
    }
    insert_init_image_options(&mut options, req.init_image.as_ref());

    ImageGenerationRequest {
        prompt: req.prompt.clone(),
        negative_prompt: req.negative_prompt.clone(),
        width: req.width.max(1),
        height: req.height.max(1),
        steps: req.steps.unwrap_or_default().max(1) as u32,
        guidance: req.guidance.unwrap_or_default(),
        seed: req.seed,
        options,
    }
}

fn build_video_generation_request(req: &DiffusionVideoRequest) -> ImageGenerationRequest {
    let mut options = req.options.clone();
    options.insert("batch_count".to_owned(), serde_json::json!(req.video_frames.max(1)));
    if let Some(cfg_scale) = req.cfg_scale {
        options.insert("cfg_scale".to_owned(), serde_json::json!(cfg_scale));
    }
    if let Some(strength) = req.strength {
        options.insert("strength".to_owned(), serde_json::json!(strength));
    }
    options.insert("fps".to_owned(), serde_json::json!(req.fps));

    if let Some(sample_method) = req.sample_method.as_ref() {
        options.insert("sample_method".to_owned(), serde_json::json!(sample_method));
    }
    if let Some(scheduler) = req.scheduler.as_ref() {
        options.insert("scheduler".to_owned(), serde_json::json!(scheduler));
    }
    insert_init_image_options(&mut options, req.init_image.as_ref());

    ImageGenerationRequest {
        prompt: req.prompt.clone(),
        negative_prompt: req.negative_prompt.clone(),
        width: req.width.max(1),
        height: req.height.max(1),
        steps: req.steps.unwrap_or_default().max(1) as u32,
        guidance: req.guidance.unwrap_or_default(),
        seed: req.seed,
        options,
    }
}

fn insert_init_image_options(
    options: &mut slab_core::api::JsonOptions,
    init_image: Option<&RawImageInput>,
) {
    let Some(init_image) = init_image else {
        return;
    };

    options.insert(
        "init_image_b64".to_owned(),
        serde_json::json!(base64::engine::general_purpose::STANDARD.encode(&init_image.data)),
    );
    options.insert("init_image_width".to_owned(), serde_json::json!(init_image.width));
    options.insert("init_image_height".to_owned(), serde_json::json!(init_image.height));
    options.insert(
        "init_image_channels".to_owned(),
        serde_json::json!(u32::from(init_image.channels.max(1))),
    );
}
