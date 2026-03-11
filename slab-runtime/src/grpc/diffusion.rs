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
            size = %req.size,
            "diffusion generate_image request received"
        );

        let payload = serde_json::json!({
            "prompt": req.prompt,
            "n": req.n,
            "size": req.size,
            "model": req.model,
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

        info!(image_bytes = output.len(), "diffusion image generation completed");
        Ok(Response::new(pb::ImageResponse {
            image: output.to_vec(),
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
