use std::time::Instant;

use tonic::{Request, Response, Status};
use tracing::{debug, error, info, instrument};

use slab_diffusion::{
    GuidanceParams as DiffusionGuidanceParams, Image as DiffusionImage,
    ImgParams as DiffusionImgParams, SampleMethod as DiffusionSampleMethod,
    SampleParams as DiffusionSampleParams, Scheduler as DiffusionScheduler, SlgParams,
    VideoParams as DiffusionVideoParams,
};
use slab_proto::{convert, slab::ipc::v1 as pb};
use slab_types::diffusion::{DiffusionImageRequest, DiffusionVideoRequest};

use super::{BackendKind, GrpcServiceImpl, extract_request_id, proto_to_status, runtime_to_status};

#[tonic::async_trait]
impl pb::diffusion_service_server::DiffusionService for GrpcServiceImpl {
    #[instrument(skip_all, fields(request_id, backend = "ggml.diffusion"))]
    async fn generate_image(
        &self,
        request: Request<pb::ImageRequest>,
    ) -> Result<Response<pb::ImageResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);
        let started_at = Instant::now();

        let req = request.into_inner();
        let request = convert::decode_diffusion_image_request(&req).map_err(|error| {
            error!(error = %error, "failed to decode diffusion image request");
            proto_to_status(error)
        })?;

        debug!(
            prompt_len = request.prompt.len(),
            n = request.count,
            width = request.width,
            height = request.height,
            has_init_image = request.init_image.is_some(),
            steps = request.steps,
            seed = request.seed,
            "diffusion generate_image request received"
        );

        let pipeline =
            self.pipeline_for_backend(BackendKind::Diffusion).await.map_err(|status| {
                error!(
                    grpc.code = %status.code(),
                    grpc.message = %status.message(),
                    elapsed_ms = started_at.elapsed().as_millis(),
                    "diffusion pipeline unavailable for image request"
                );
                status
            })?;
        let image_params = build_image_params(&request).map_err(|status| {
            error!(
                grpc.code = %status.code(),
                grpc.message = %status.message(),
                elapsed_ms = started_at.elapsed().as_millis(),
                "diffusion image request validation failed"
            );
            status
        })?;
        let generated = pipeline.run_inference_image(image_params).await.map_err(|error| {
            error!(error = %error, "diffusion image generation failed");
            runtime_to_status(error)
        })?;

        let grpc_response = encode_generated_image_response(&generated)?;
        info!(
            images_json_bytes = grpc_response.images_json.len(),
            image_count = generated.len(),
            elapsed_ms = started_at.elapsed().as_millis(),
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
        let started_at = Instant::now();

        let req = request.into_inner();
        let request = convert::decode_diffusion_video_request(&req).map_err(|error| {
            error!(error = %error, "failed to decode diffusion video request");
            proto_to_status(error)
        })?;

        debug!(
            prompt_len = request.prompt.len(),
            video_frames = request.video_frames,
            has_init_image = request.init_image.is_some(),
            steps = request.steps,
            seed = request.seed,
            "diffusion generate_video request received"
        );

        let pipeline =
            self.pipeline_for_backend(BackendKind::Diffusion).await.map_err(|status| {
                error!(
                    grpc.code = %status.code(),
                    grpc.message = %status.message(),
                    elapsed_ms = started_at.elapsed().as_millis(),
                    "diffusion pipeline unavailable for video request"
                );
                status
            })?;
        let video_params = build_video_params(&request).map_err(|status| {
            error!(
                grpc.code = %status.code(),
                grpc.message = %status.message(),
                elapsed_ms = started_at.elapsed().as_millis(),
                "diffusion video request validation failed"
            );
            status
        })?;
        let generated = pipeline
            .run_inference_image(lower_video_to_image_params(&video_params))
            .await
            .map_err(|error| {
                error!(
                    error = %error,
                    elapsed_ms = started_at.elapsed().as_millis(),
                    "diffusion video generation failed"
                );
                runtime_to_status(error)
            })?;

        let grpc_response = encode_generated_video_response(&generated)?;
        info!(
            frames_json_bytes = grpc_response.frames_json.len(),
            frame_count = generated.len(),
            elapsed_ms = started_at.elapsed().as_millis(),
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

        let req = request.get_ref();
        let has_diffusion_overrides = !req.diffusion_model_path.is_empty()
            || !req.vae_path.is_empty()
            || !req.taesd_path.is_empty()
            || !req.lora_model_dir.is_empty()
            || !req.clip_l_path.is_empty()
            || !req.clip_g_path.is_empty()
            || !req.t5xxl_path.is_empty()
            || !req.vae_device.is_empty()
            || !req.clip_device.is_empty()
            || req.flash_attn
            || req.offload_params_to_cpu;

        debug!(
            request_id = %request_id,
            model_path = %req.model_path,
            num_workers = req.num_workers,
            context_length = req.context_length,
            has_diffusion_overrides,
            "diffusion load_model request received"
        );
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
}

fn build_image_params(req: &DiffusionImageRequest) -> Result<DiffusionImgParams, Status> {
    let sample_method = req
        .sample_method
        .as_deref()
        .map(str::parse::<DiffusionSampleMethod>)
        .transpose()
        .map_err(Status::invalid_argument)?;
    let scheduler = req
        .scheduler
        .as_deref()
        .map(str::parse::<DiffusionScheduler>)
        .transpose()
        .map_err(Status::invalid_argument)?;

    if req.count < 1 {
        return Err(Status::invalid_argument("count must be >= 1"));
    }
    if req.width < 1 {
        return Err(Status::invalid_argument("width must be >= 1"));
    }
    if req.height < 1 {
        return Err(Status::invalid_argument("height must be >= 1"));
    }
    if let Some(steps) = req.steps
        && steps < 1
    {
        return Err(Status::invalid_argument("steps must be >= 1"));
    }

    let mut sample_params = DiffusionSampleParams::default();
    if req.cfg_scale.is_some() || req.guidance.is_some() {
        let cfg_scale = req.cfg_scale.or(req.guidance).unwrap_or_default();
        let distilled_guidance = req.guidance.or(req.cfg_scale).unwrap_or_default();
        sample_params.guidance = Some(DiffusionGuidanceParams {
            txt_cfg: cfg_scale,
            img_cfg: cfg_scale,
            distilled_guidance,
            slg: SlgParams::default(),
        });
    }
    sample_params.sample_method = sample_method;
    sample_params.scheduler = scheduler;
    sample_params.sample_steps = req.steps;
    sample_params.eta = req.eta;

    Ok(DiffusionImgParams {
        prompt: Some(req.prompt.clone()),
        negative_prompt: req.negative_prompt.clone(),
        clip_skip: req.clip_skip,
        init_image: req.init_image.as_ref().map(raw_image_to_diffusion_image).transpose()?,
        width: Some(req.width),
        height: Some(req.height),
        sample_params: (sample_params != DiffusionSampleParams::default()).then_some(sample_params),
        strength: req.strength,
        seed: req.seed,
        batch_count: Some(req.count),
        ..Default::default()
    })
}

fn build_video_params(req: &DiffusionVideoRequest) -> Result<DiffusionVideoParams, Status> {
    let sample_method = req
        .sample_method
        .as_deref()
        .map(str::parse::<DiffusionSampleMethod>)
        .transpose()
        .map_err(Status::invalid_argument)?;
    let scheduler = req
        .scheduler
        .as_deref()
        .map(str::parse::<DiffusionScheduler>)
        .transpose()
        .map_err(Status::invalid_argument)?;

    if req.width < 1 {
        return Err(Status::invalid_argument("width must be >= 1"));
    }
    if req.height < 1 {
        return Err(Status::invalid_argument("height must be >= 1"));
    }
    if req.video_frames < 1 {
        return Err(Status::invalid_argument("video_frames must be >= 1"));
    }
    if let Some(steps) = req.steps
        && steps < 1
    {
        return Err(Status::invalid_argument("steps must be >= 1"));
    }

    let mut sample_params = DiffusionSampleParams::default();
    if req.cfg_scale.is_some() || req.guidance.is_some() {
        let cfg_scale = req.cfg_scale.or(req.guidance).unwrap_or_default();
        let distilled_guidance = req.guidance.or(req.cfg_scale).unwrap_or_default();
        sample_params.guidance = Some(DiffusionGuidanceParams {
            txt_cfg: cfg_scale,
            img_cfg: cfg_scale,
            distilled_guidance,
            slg: SlgParams::default(),
        });
    }
    sample_params.sample_method = sample_method;
    sample_params.scheduler = scheduler;
    sample_params.sample_steps = req.steps;

    Ok(DiffusionVideoParams {
        prompt: Some(req.prompt.clone()),
        negative_prompt: req.negative_prompt.clone(),
        init_image: req.init_image.as_ref().map(raw_image_to_diffusion_image).transpose()?,
        width: Some(req.width),
        height: Some(req.height),
        sample_params: (sample_params != DiffusionSampleParams::default()).then_some(sample_params),
        strength: req.strength,
        seed: req.seed,
        video_frames: Some(
            u32::try_from(req.video_frames)
                .map_err(|_| Status::invalid_argument("video_frames exceeds u32 range"))?,
        ),
        ..Default::default()
    })
}

fn lower_video_to_image_params(video: &DiffusionVideoParams) -> DiffusionImgParams {
    DiffusionImgParams {
        prompt: video.prompt.clone(),
        negative_prompt: video.negative_prompt.clone(),
        loras: video.loras.clone(),
        clip_skip: video.clip_skip,
        init_image: video.init_image.clone(),
        width: video.width,
        height: video.height,
        sample_params: video.sample_params.clone(),
        strength: video.strength,
        seed: video.seed,
        batch_count: video.video_frames,
        vae_tiling_params: video.vae_tiling_params.clone(),
        cache: video.cache.clone(),
        ..Default::default()
    }
}

fn raw_image_to_diffusion_image(
    image: &slab_types::media::RawImageInput,
) -> Result<DiffusionImage, Status> {
    if image.channels == 0 {
        return Err(Status::invalid_argument("raw image input channels must be >= 1"));
    }

    Ok(DiffusionImage {
        width: image.width,
        height: image.height,
        channel: u32::from(image.channels),
        data: image.data.clone(),
    })
}

fn encode_generated_image_response(images: &[DiffusionImage]) -> Result<pb::ImageResponse, Status> {
    let response = slab_types::diffusion::DiffusionImageResponse {
        images: images
            .iter()
            .map(diffusion_image_to_generated_image)
            .collect::<Result<Vec<_>, Status>>()?,
        metadata: Default::default(),
    };
    convert::encode_diffusion_image_response(&response).map_err(|error| {
        Status::internal(format!("failed to encode generated image response: {error}"))
    })
}

fn encode_generated_video_response(images: &[DiffusionImage]) -> Result<pb::VideoResponse, Status> {
    let response = slab_types::diffusion::DiffusionVideoResponse {
        frames: images
            .iter()
            .map(|image| {
                Ok(slab_types::media::GeneratedFrame {
                    data: image.data.clone(),
                    width: image.width,
                    height: image.height,
                    channels: diffusion_image_channel_to_u8(image.channel)?,
                })
            })
            .collect::<Result<Vec<_>, Status>>()?,
        metadata: Default::default(),
    };
    convert::encode_diffusion_video_response(&response).map_err(|error| {
        Status::internal(format!("failed to encode generated video response: {error}"))
    })
}

fn diffusion_image_to_generated_image(
    image: &DiffusionImage,
) -> Result<slab_types::media::GeneratedImage, Status> {
    let dynamic = match image.channel {
        3 => image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(
            image.width,
            image.height,
            image.data.clone(),
        )
        .map(image::DynamicImage::ImageRgb8),
        4 => image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
            image.width,
            image.height,
            image.data.clone(),
        )
        .map(image::DynamicImage::ImageRgba8),
        other => {
            return Err(Status::internal(format!(
                "unsupported diffusion image channel count: {other}"
            )));
        }
    }
    .ok_or_else(|| {
        Status::internal(format!(
            "invalid raw diffusion image buffer for {}x{}x{}",
            image.width, image.height, image.channel
        ))
    })?;

    let mut png_bytes = Vec::new();
    dynamic
        .write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .map_err(|error| Status::internal(format!("failed to encode generated PNG: {error}")))?;

    Ok(slab_types::media::GeneratedImage {
        bytes: png_bytes,
        width: image.width,
        height: image.height,
        channels: diffusion_image_channel_to_u8(image.channel)?,
    })
}

fn diffusion_image_channel_to_u8(channel: u32) -> Result<u8, Status> {
    let channel = u8::try_from(channel).map_err(|_| {
        Status::internal(format!("diffusion image channel count exceeds u8 range: {channel}"))
    })?;
    if channel == 0 {
        return Err(Status::internal("diffusion image channel count must be >= 1"));
    }
    Ok(channel)
}
