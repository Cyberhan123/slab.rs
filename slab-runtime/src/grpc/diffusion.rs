use base64::Engine as _;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, instrument, warn};

use slab_core::api::Backend;
use slab_proto::slab::ipc::v1 as pb;

use super::{
    extract_request_id, load_model_for_backend, reload_library_for_backend, runtime_to_status,
    unload_model_for_backend, GrpcServiceImpl,
};

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
        if req.prompt.is_empty() {
            warn!("generate_image rejected: prompt is empty");
            return Err(Status::invalid_argument("prompt must not be empty"));
        }

        debug!(
            prompt_len = req.prompt.len(),
            n = req.n,
            width = req.width,
            height = req.height,
            "diffusion generate_image request received"
        );

        // Build init_image_b64 if raw pixel data was provided.
        let init_image_b64 = if !req.init_image_data.is_empty() {
            Some(base64::engine::general_purpose::STANDARD.encode(&req.init_image_data))
        } else {
            None
        };

        let payload = serde_json::json!({
            "prompt": req.prompt,
            "negative_prompt": req.negative_prompt,
            // Dimensions: the server layer ensures these are non-zero before the gRPC call.
            // Fall back to 512 here only as a last-resort guard against malformed requests.
            "width": if req.width == 0 { 512u32 } else { req.width },
            "height": if req.height == 0 { 512u32 } else { req.height },
            // Numeric parameters: pass through as-is.  The slab-server layer applies all
            // defaults via Option::unwrap_or before the gRPC call, so 0 / 0.0 here is a
            // legitimate value (e.g. cfg_scale=0 for distilled models, strength=0 for a
            // no-op img2img), not an "unset" signal.
            "cfg_scale": req.cfg_scale,
            "guidance": req.guidance,
            "sample_steps": if req.sample_steps == 0 { 20i32 } else { req.sample_steps },
            "seed": req.seed,
            "sample_method": if req.sample_method.is_empty() { "auto" } else { &req.sample_method },
            "scheduler": if req.scheduler.is_empty() { "auto" } else { &req.scheduler },
            "clip_skip": req.clip_skip,
            "strength": req.strength,
            "eta": req.eta,
            "batch_count": if req.n == 0 { 1u32 } else { req.n },
            "init_image_b64": init_image_b64,
            "init_image_width": req.init_image_width,
            "init_image_height": req.init_image_height,
            "init_image_channels": if req.init_image_channels == 0 { 3u32 } else { req.init_image_channels },
        });

        let output = slab_core::api::backend(Backend::GGMLDiffusion)
            .inference()
            .input(slab_core::Payload::Json(payload))
            .run_wait()
            .await
            .map_err(|e| {
                error!(error = %e, "diffusion inference failed");
                runtime_to_status(e)
            })?;

        info!(images_json_bytes = output.len(), "diffusion image generation completed");
        Ok(Response::new(pb::ImageResponse {
            images_json: output.to_vec(),
        }))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.diffusion"))]
    async fn generate_video(
        &self,
        request: Request<pb::VideoRequest>,
    ) -> Result<Response<pb::VideoResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let req = request.into_inner();
        if req.prompt.is_empty() {
            warn!("generate_video rejected: prompt is empty");
            return Err(Status::invalid_argument("prompt must not be empty"));
        }

        debug!(
            prompt_len = req.prompt.len(),
            video_frames = req.video_frames,
            "diffusion generate_video request received"
        );

        let init_image_b64 = if !req.init_image_data.is_empty() {
            Some(base64::engine::general_purpose::STANDARD.encode(&req.init_image_data))
        } else {
            None
        };

        let payload = serde_json::json!({
            "prompt": req.prompt,
            "negative_prompt": req.negative_prompt,
            "width": if req.width == 0 { 512u32 } else { req.width },
            "height": if req.height == 0 { 512u32 } else { req.height },
            // Numeric parameters: pass through as-is (see generate_image comment).
            "cfg_scale": req.cfg_scale,
            "guidance": req.guidance,
            "sample_steps": if req.sample_steps == 0 { 20i32 } else { req.sample_steps },
            "seed": req.seed,
            "sample_method": if req.sample_method.is_empty() { "auto" } else { &req.sample_method },
            "scheduler": if req.scheduler.is_empty() { "auto" } else { &req.scheduler },
            "strength": req.strength,
            "batch_count": if req.video_frames == 0 { 16i32 } else { req.video_frames },
            "init_image_b64": init_image_b64,
            "init_image_width": req.init_image_width,
            "init_image_height": req.init_image_height,
            "init_image_channels": if req.init_image_channels == 0 { 3u32 } else { req.init_image_channels },
        });

        let output = slab_core::api::backend(Backend::GGMLDiffusion)
            .inference()
            .input(slab_core::Payload::Json(payload))
            .run_wait()
            .await
            .map_err(|e| {
                error!(error = %e, "diffusion video inference failed");
                runtime_to_status(e)
            })?;

        info!(frames_json_bytes = output.len(), "diffusion video generation completed");
        Ok(Response::new(pb::VideoResponse {
            frames_json: output.to_vec(),
        }))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.diffusion"))]
    async fn load_model(
        &self,
        request: Request<pb::ModelLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        debug!("diffusion load_model request received");
        let status = load_model_for_backend(Backend::GGMLDiffusion, request.into_inner()).await?;
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
        let status = unload_model_for_backend(Backend::GGMLDiffusion).await?;
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
            reload_library_for_backend(Backend::GGMLDiffusion, request.into_inner()).await?;
        Ok(Response::new(status))
    }
}
