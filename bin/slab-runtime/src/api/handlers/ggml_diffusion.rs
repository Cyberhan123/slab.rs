use tonic::{Request, Response, Status};
use tracing::instrument;

use slab_proto::slab::ipc::v1 as pb;

use crate::application::dtos as dto;

use super::{GrpcServiceImpl, application_to_status, extract_request_id, proto_to_status};

#[tonic::async_trait]
impl pb::ggml_diffusion_service_server::GgmlDiffusionService for GrpcServiceImpl {
    #[instrument(skip_all, fields(request_id, backend = "ggml.diffusion"))]
    async fn generate_image(
        &self,
        request: Request<pb::GgmlDiffusionGenerateImageRequest>,
    ) -> Result<Response<pb::GgmlDiffusionGenerateImageResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let dto = dto::decode_ggml_diffusion_generate_image_request(&request.into_inner())
            .map_err(proto_to_status)?;
        let response = self
            .application
            .ggml_diffusion()
            .generate_image(dto)
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_ggml_diffusion_generate_image_response(&response)))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.diffusion"))]
    async fn generate_video(
        &self,
        request: Request<pb::GgmlDiffusionGenerateVideoRequest>,
    ) -> Result<Response<pb::GgmlDiffusionGenerateVideoResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let dto = dto::decode_ggml_diffusion_generate_video_request(&request.into_inner())
            .map_err(proto_to_status)?;
        let response = self
            .application
            .ggml_diffusion()
            .generate_video(dto)
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_ggml_diffusion_generate_video_response(&response)))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.diffusion"))]
    async fn load_model(
        &self,
        request: Request<pb::GgmlDiffusionLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let dto = dto::decode_ggml_diffusion_load_request(&request.into_inner())
            .map_err(proto_to_status)?;
        let status = self
            .application
            .ggml_diffusion()
            .load_model(dto)
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_model_status_response(&status)))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.diffusion"))]
    async fn unload_model(
        &self,
        request: Request<pb::ModelUnloadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);
        let _ = request.into_inner();

        let status = self
            .application
            .ggml_diffusion()
            .unload_model()
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_model_status_response(&status)))
    }
}
