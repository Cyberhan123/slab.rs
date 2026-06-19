use tonic::{Request, Response, Status};
use tracing::instrument;

use slab_proto::slab::ipc::v1 as pb;

use crate::application::dtos as dto;

use super::{GrpcServiceImpl, extract_request_id, forward};

#[tonic::async_trait]
impl pb::ggml_diffusion_service_server::GgmlDiffusionService for GrpcServiceImpl {
    #[instrument(skip_all, fields(request_id, backend = "ggml.diffusion"))]
    async fn generate_image(
        &self,
        request: Request<pb::GgmlDiffusionGenerateImageRequest>,
    ) -> Result<Response<pb::GgmlDiffusionGenerateImageResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        forward(
            request,
            dto::decode_ggml_diffusion_generate_image_request,
            || self.application.ggml_diffusion(),
            |service, dto| async move { service.generate_image(dto).await },
            dto::encode_ggml_diffusion_generate_image_response,
        )
        .await
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.diffusion"))]
    async fn generate_video(
        &self,
        request: Request<pb::GgmlDiffusionGenerateVideoRequest>,
    ) -> Result<Response<pb::GgmlDiffusionGenerateVideoResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        forward(
            request,
            dto::decode_ggml_diffusion_generate_video_request,
            || self.application.ggml_diffusion(),
            |service, dto| async move { service.generate_video(dto).await },
            dto::encode_ggml_diffusion_generate_video_response,
        )
        .await
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.diffusion"))]
    async fn load_model(
        &self,
        request: Request<pb::GgmlDiffusionLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        forward(
            request,
            dto::decode_ggml_diffusion_load_request,
            || self.application.ggml_diffusion(),
            |service, dto| async move { service.load_model(dto).await },
            dto::encode_model_status_response,
        )
        .await
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.diffusion"))]
    async fn unload_model(
        &self,
        request: Request<pb::ModelUnloadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);
        forward(
            request,
            |_| Ok(()),
            || self.application.ggml_diffusion(),
            |service, _| async move { service.unload_model().await },
            dto::encode_model_status_response,
        )
        .await
    }
}
