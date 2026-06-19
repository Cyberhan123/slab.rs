use tonic::{Request, Response, Status};
use tracing::instrument;

use slab_proto::slab::ipc::v1 as pb;

use crate::application::dtos as dto;

use super::{GrpcServiceImpl, extract_request_id, forward};

#[tonic::async_trait]
impl pb::onnx_service_server::OnnxService for GrpcServiceImpl {
    #[instrument(skip_all, fields(request_id, backend = "onnx.text"))]
    async fn run_text(
        &self,
        request: Request<pb::OnnxTextRequest>,
    ) -> Result<Response<pb::OnnxTextResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        forward(
            request,
            dto::decode_onnx_text_request,
            || self.application.onnx_text(),
            |service, dto| async move { service.run_text(dto).await },
            dto::encode_onnx_text_response,
        )
        .await
    }

    #[instrument(skip_all, fields(request_id, backend = "onnx.embedding"))]
    async fn run_embedding(
        &self,
        request: Request<pb::OnnxEmbeddingRequest>,
    ) -> Result<Response<pb::OnnxEmbeddingResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        forward(
            request,
            dto::decode_onnx_embedding_request,
            || self.application.onnx_embedding(),
            |service, dto| async move { service.run_embedding(dto).await },
            dto::encode_onnx_embedding_response,
        )
        .await
    }

    #[instrument(skip_all, fields(request_id, backend = "onnx.text"))]
    async fn load_text_model(
        &self,
        request: Request<pb::OnnxTextLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        forward(
            request,
            dto::decode_onnx_text_load_request,
            || self.application.onnx_text(),
            |service, dto| async move { service.load_text_model(dto).await },
            dto::encode_model_status_response,
        )
        .await
    }

    #[instrument(skip_all, fields(request_id, backend = "onnx.text"))]
    async fn unload_text_model(
        &self,
        request: Request<pb::ModelUnloadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);
        forward(
            request,
            |_| Ok(()),
            || self.application.onnx_text(),
            |service, _| async move { service.unload_text_model().await },
            dto::encode_model_status_response,
        )
        .await
    }

    #[instrument(skip_all, fields(request_id, backend = "onnx.embedding"))]
    async fn load_embedding_model(
        &self,
        request: Request<pb::OnnxEmbeddingLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        forward(
            request,
            dto::decode_onnx_embedding_load_request,
            || self.application.onnx_embedding(),
            |service, dto| async move { service.load_embedding_model(dto).await },
            dto::encode_model_status_response,
        )
        .await
    }

    #[instrument(skip_all, fields(request_id, backend = "onnx.embedding"))]
    async fn unload_embedding_model(
        &self,
        request: Request<pb::ModelUnloadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);
        forward(
            request,
            |_| Ok(()),
            || self.application.onnx_embedding(),
            |service, _| async move { service.unload_embedding_model().await },
            dto::encode_model_status_response,
        )
        .await
    }
}
