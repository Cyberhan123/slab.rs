use tonic::{Request, Response, Status};
use tracing::instrument;

use slab_proto::slab::ipc::v1 as pb;

use crate::application::dtos as dto;

use super::{GrpcServiceImpl, application_to_status, extract_request_id, proto_to_status};

#[tonic::async_trait]
impl pb::candle_diffusion_service_server::CandleDiffusionService for GrpcServiceImpl {
    #[instrument(skip_all, fields(request_id, backend = "candle.diffusion"))]
    async fn generate_image(
        &self,
        request: Request<pb::CandleDiffusionGenerateImageRequest>,
    ) -> Result<Response<pb::CandleDiffusionGenerateImageResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let dto = dto::decode_candle_diffusion_generate_image_request(&request.into_inner())
            .map_err(proto_to_status)?;
        let response = self
            .application
            .candle_diffusion()
            .map_err(application_to_status)?
            .generate_image(dto)
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_candle_diffusion_generate_image_response(&response)))
    }

    #[instrument(skip_all, fields(request_id, backend = "candle.diffusion"))]
    async fn load_model(
        &self,
        request: Request<pb::CandleDiffusionLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let dto = dto::decode_candle_diffusion_load_request(&request.into_inner())
            .map_err(proto_to_status)?;
        let status = self
            .application
            .candle_diffusion()
            .map_err(application_to_status)?
            .load_model(dto)
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_model_status_response(&status)))
    }

    #[instrument(skip_all, fields(request_id, backend = "candle.diffusion"))]
    async fn unload_model(
        &self,
        request: Request<pb::ModelUnloadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);
        let _ = request.into_inner();

        let status = self
            .application
            .candle_diffusion()
            .map_err(application_to_status)?
            .unload_model()
            .await
            .map_err(application_to_status)?;
        Ok(Response::new(dto::encode_model_status_response(&status)))
    }
}
