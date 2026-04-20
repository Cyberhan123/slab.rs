use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{debug, error, instrument};

use slab_proto::slab::ipc::v1 as pb;

use crate::application::dtos as dto;

use super::{
    GrpcServiceImpl, application_to_status, extract_request_id, proto_to_status, runtime_to_status,
};

#[tonic::async_trait]
impl pb::candle_transformers_service_server::CandleTransformersService for GrpcServiceImpl {
    #[instrument(skip_all, fields(request_id, backend = "candle.llama"))]
    async fn chat(
        &self,
        request: Request<pb::CandleChatRequest>,
    ) -> Result<Response<pb::CandleChatResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let dto =
            dto::decode_candle_chat_request(&request.into_inner()).map_err(proto_to_status)?;
        let response = self
            .application
            .candle_llama()
            .map_err(application_to_status)?
            .chat(dto)
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_candle_chat_response(&response)))
    }

    type ChatStreamStream = ReceiverStream<Result<pb::CandleChatStreamChunk, Status>>;

    #[instrument(skip_all, fields(request_id, backend = "candle.llama"))]
    async fn chat_stream(
        &self,
        request: Request<pb::CandleChatRequest>,
    ) -> Result<Response<Self::ChatStreamStream>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let dto =
            dto::decode_candle_chat_request(&request.into_inner()).map_err(proto_to_status)?;
        let stream = self
            .application
            .candle_llama()
            .map_err(application_to_status)?
            .chat_stream(dto)
            .await
            .map_err(application_to_status)?;

        let (tx, rx) = mpsc::channel::<Result<pb::CandleChatStreamChunk, Status>>(32);
        tokio::spawn(async move {
            tokio::pin!(stream);
            while let Some(chunk) = stream.next().await {
                let message = match chunk {
                    Ok(chunk) => Ok(dto::encode_candle_chat_stream_chunk(&chunk)),
                    Err(error) => {
                        error!(error = %error, "candle llama stream failed");
                        Err(runtime_to_status(error))
                    }
                };
                if tx.send(message).await.is_err() {
                    debug!("candle llama stream receiver dropped");
                    return;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    #[instrument(skip_all, fields(request_id, backend = "candle.whisper"))]
    async fn transcribe(
        &self,
        request: Request<pb::CandleWhisperTranscribeRequest>,
    ) -> Result<Response<pb::CandleWhisperTranscribeResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let dto = dto::decode_candle_whisper_transcribe_request(&request.into_inner())
            .map_err(proto_to_status)?;
        let response = self
            .application
            .candle_whisper()
            .map_err(application_to_status)?
            .transcribe(dto)
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_candle_whisper_transcribe_response(&response)))
    }

    #[instrument(skip_all, fields(request_id, backend = "candle.llama"))]
    async fn load_llama_model(
        &self,
        request: Request<pb::CandleLlamaLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let dto = dto::decode_candle_llama_load_request(&request.into_inner())
            .map_err(proto_to_status)?;
        let status = self
            .application
            .candle_llama()
            .map_err(application_to_status)?
            .load_llama_model(dto)
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_model_status_response(&status)))
    }

    #[instrument(skip_all, fields(request_id, backend = "candle.llama"))]
    async fn unload_llama_model(
        &self,
        request: Request<pb::ModelUnloadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);
        let _ = request.into_inner();

        let status = self
            .application
            .candle_llama()
            .map_err(application_to_status)?
            .unload_llama_model()
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_model_status_response(&status)))
    }

    #[instrument(skip_all, fields(request_id, backend = "candle.whisper"))]
    async fn load_whisper_model(
        &self,
        request: Request<pb::CandleWhisperLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let dto = dto::decode_candle_whisper_load_request(&request.into_inner())
            .map_err(proto_to_status)?;
        let status = self
            .application
            .candle_whisper()
            .map_err(application_to_status)?
            .load_whisper_model(dto)
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_model_status_response(&status)))
    }

    #[instrument(skip_all, fields(request_id, backend = "candle.whisper"))]
    async fn unload_whisper_model(
        &self,
        request: Request<pb::ModelUnloadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);
        let _ = request.into_inner();

        let status = self
            .application
            .candle_whisper()
            .map_err(application_to_status)?
            .unload_whisper_model()
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_model_status_response(&status)))
    }
}
