use tonic::{Request, Response, Status};
use tracing::instrument;

use slab_proto::slab::ipc::v1 as pb;

use crate::application::dtos as dto;

use super::{GrpcServiceImpl, application_to_status, extract_request_id, proto_to_status};

#[tonic::async_trait]
impl pb::ggml_whisper_service_server::GgmlWhisperService for GrpcServiceImpl {
    #[instrument(skip_all, fields(request_id, backend = "ggml.whisper"))]
    async fn transcribe(
        &self,
        request: Request<pb::GgmlWhisperTranscribeRequest>,
    ) -> Result<Response<pb::GgmlWhisperTranscribeResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let dto = dto::decode_ggml_whisper_transcribe_request(&request.into_inner())
            .map_err(proto_to_status)?;
        let response = self
            .application
            .ggml_whisper()
            .map_err(application_to_status)?
            .transcribe(dto)
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_ggml_whisper_transcribe_response(&response)))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.whisper"))]
    async fn load_model(
        &self,
        request: Request<pb::GgmlWhisperLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let dto = dto::decode_ggml_whisper_load_request(&request.into_inner())
            .map_err(proto_to_status)?;
        let status = self
            .application
            .ggml_whisper()
            .map_err(application_to_status)?
            .load_model(dto)
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_model_status_response(&status)))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.whisper"))]
    async fn unload_model(
        &self,
        request: Request<pb::ModelUnloadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);
        let _ = request.into_inner();

        let status = self
            .application
            .ggml_whisper()
            .map_err(application_to_status)?
            .unload_model()
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_model_status_response(&status)))
    }
}
