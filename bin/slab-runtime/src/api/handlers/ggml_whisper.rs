use tonic::{Request, Response, Status};
use tracing::instrument;

use slab_proto::slab::ipc::v1 as pb;

use crate::application::dtos as dto;

use super::{GrpcServiceImpl, extract_request_id, forward};

#[tonic::async_trait]
impl pb::ggml_whisper_service_server::GgmlWhisperService for GrpcServiceImpl {
    #[instrument(skip_all, fields(request_id, backend = "ggml.whisper"))]
    async fn transcribe(
        &self,
        request: Request<pb::GgmlWhisperTranscribeRequest>,
    ) -> Result<Response<pb::GgmlWhisperTranscribeResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        forward(
            request,
            dto::decode_ggml_whisper_transcribe_request,
            || self.application.ggml_whisper(),
            |service, dto| async move { service.transcribe(dto).await },
            dto::encode_ggml_whisper_transcribe_response,
        )
        .await
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.whisper"))]
    async fn load_model(
        &self,
        request: Request<pb::GgmlWhisperLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        forward(
            request,
            dto::decode_ggml_whisper_load_request,
            || self.application.ggml_whisper(),
            |service, dto| async move { service.load_model(dto).await },
            dto::encode_model_status_response,
        )
        .await
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.whisper"))]
    async fn unload_model(
        &self,
        request: Request<pb::ModelUnloadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);
        forward(
            request,
            |_| Ok(()),
            || self.application.ggml_whisper(),
            |service, _| async move { service.unload_model().await },
            dto::encode_model_status_response,
        )
        .await
    }
}
