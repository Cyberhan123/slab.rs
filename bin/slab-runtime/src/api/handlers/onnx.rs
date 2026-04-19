use tonic::{Request, Response, Status};
use tracing::instrument;

use slab_proto::slab::ipc::v1 as pb;

use crate::application::dtos as dto;

use super::{GrpcServiceImpl, application_to_status, extract_request_id, proto_to_status};

#[tonic::async_trait]
impl pb::onnx_service_server::OnnxService for GrpcServiceImpl {
    #[instrument(skip_all, fields(request_id, backend = "onnx.text"))]
    async fn run_text(
        &self,
        request: Request<pb::OnnxTextRequest>,
    ) -> Result<Response<pb::OnnxTextResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let dto = dto::decode_onnx_text_request(&request.into_inner()).map_err(proto_to_status)?;
        let response = self
            .application
            .onnx_text()
            .map_err(application_to_status)?
            .run_text(dto)
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_onnx_text_response(&response)))
    }

    #[instrument(skip_all, fields(request_id, backend = "onnx.embedding"))]
    async fn run_embedding(
        &self,
        request: Request<pb::OnnxEmbeddingRequest>,
    ) -> Result<Response<pb::OnnxEmbeddingResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let dto =
            dto::decode_onnx_embedding_request(&request.into_inner()).map_err(proto_to_status)?;
        let response = self
            .application
            .onnx_embedding()
            .map_err(application_to_status)?
            .run_embedding(dto)
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_onnx_embedding_response(&response)))
    }

    #[instrument(skip_all, fields(request_id, backend = "onnx.text"))]
    async fn load_text_model(
        &self,
        request: Request<pb::OnnxTextLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let dto =
            dto::decode_onnx_text_load_request(&request.into_inner()).map_err(proto_to_status)?;
        let status = self
            .application
            .onnx_text()
            .map_err(application_to_status)?
            .load_text_model(dto)
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_model_status_response(&status)))
    }

    #[instrument(skip_all, fields(request_id, backend = "onnx.text"))]
    async fn unload_text_model(
        &self,
        request: Request<pb::ModelUnloadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);
        let _ = request.into_inner();

        let status = self
            .application
            .onnx_text()
            .map_err(application_to_status)?
            .unload_text_model()
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_model_status_response(&status)))
    }

    #[instrument(skip_all, fields(request_id, backend = "onnx.embedding"))]
    async fn load_embedding_model(
        &self,
        request: Request<pb::OnnxEmbeddingLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let dto = dto::decode_onnx_embedding_load_request(&request.into_inner())
            .map_err(proto_to_status)?;
        let status = self
            .application
            .onnx_embedding()
            .map_err(application_to_status)?
            .load_embedding_model(dto)
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_model_status_response(&status)))
    }

    #[instrument(skip_all, fields(request_id, backend = "onnx.embedding"))]
    async fn unload_embedding_model(
        &self,
        request: Request<pb::ModelUnloadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);
        let _ = request.into_inner();

        let status = self
            .application
            .onnx_embedding()
            .map_err(application_to_status)?
            .unload_embedding_model()
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_model_status_response(&status)))
    }
}
