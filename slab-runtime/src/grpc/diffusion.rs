use tonic::{Request, Response, Status};

use slab_core::api::Backend;
use slab_proto::slab::ipc::v1 as pb;

use super::{
    load_model_for_backend, reload_library_for_backend, runtime_to_status,
    unload_model_for_backend, GrpcServiceImpl,
};

#[tonic::async_trait]
impl pb::diffusion_service_server::DiffusionService for GrpcServiceImpl {
    async fn generate_image(
        &self,
        request: Request<pb::ImageRequest>,
    ) -> Result<Response<pb::ImageResponse>, Status> {
        let req = request.into_inner();
        if req.prompt.is_empty() {
            return Err(Status::invalid_argument("prompt must not be empty"));
        }

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
            .map_err(runtime_to_status)?;

        Ok(Response::new(pb::ImageResponse {
            image: output.to_vec(),
        }))
    }

    async fn load_model(
        &self,
        request: Request<pb::ModelLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let status = load_model_for_backend(Backend::GGMLDiffusion, request.into_inner()).await?;
        Ok(Response::new(status))
    }

    async fn unload_model(
        &self,
        _request: Request<pb::ModelUnloadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let status = unload_model_for_backend(Backend::GGMLDiffusion).await?;
        Ok(Response::new(status))
    }

    async fn reload_library(
        &self,
        request: Request<pb::ReloadLibraryRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let status =
            reload_library_for_backend(Backend::GGMLDiffusion, request.into_inner()).await?;
        Ok(Response::new(status))
    }
}
