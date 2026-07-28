use tonic::{Request, Response, Status};
use tracing::instrument;

use slab_proto::slab::ipc::v1 as pb;

use crate::application::dtos as dto;

use super::{GrpcServiceImpl, extract_request_id, forward};

#[tonic::async_trait]
impl pb::ggml_parakeet_service_server::GgmlParakeetService for GrpcServiceImpl {
    #[instrument(skip_all, fields(request_id, backend = "ggml.parakeet"))]
    async fn transcribe(
        &self,
        request: Request<pb::GgmlParakeetTranscribeRequest>,
    ) -> Result<Response<pb::GgmlParakeetTranscribeResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        forward(
            request,
            dto::decode_ggml_parakeet_transcribe_request,
            || self.application.ggml_parakeet(),
            |service, dto| async move { service.transcribe(dto).await },
            dto::encode_ggml_parakeet_transcribe_response,
        )
        .await
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.parakeet"))]
    async fn load_model(
        &self,
        request: Request<pb::GgmlParakeetLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        forward(
            request,
            dto::decode_ggml_parakeet_load_request,
            || self.application.ggml_parakeet(),
            |service, dto| async move { service.load_model(dto).await },
            dto::encode_model_status_response,
        )
        .await
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.parakeet"))]
    async fn unload_model(
        &self,
        request: Request<pb::ModelUnloadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);
        forward(
            request,
            |_| Ok(()),
            || self.application.ggml_parakeet(),
            |service, _| async move { service.unload_model().await },
            dto::encode_model_status_response,
        )
        .await
    }
}
