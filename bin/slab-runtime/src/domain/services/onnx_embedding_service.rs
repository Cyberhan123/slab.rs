use slab_runtime_core::{CoreError, Payload};
use slab_types::{Capability, ModelFamily, OnnxLoadConfig};

use crate::application::dtos as dto;

use super::ExecutionHub;
use super::driver_runtime::DriverRuntime;
use super::helpers::{
    embedding_image_to_tensor, invalid_model, model_spec, onnx_named_output_from_payload,
    onnx_tensors_to_json, required_path, required_string,
};

#[derive(Clone, Debug)]
pub(crate) struct OnnxEmbeddingService {
    runtime: DriverRuntime,
    input_tensor_name: String,
    output_tensor_name: String,
}

impl OnnxEmbeddingService {
    pub(crate) fn new(
        execution: ExecutionHub,
        request: dto::OnnxEmbeddingLoadRequest,
    ) -> Result<Self, CoreError> {
        let model_path = required_path("onnx_embedding.model_path", request.model_path)?;
        let input_tensor_name =
            required_string("onnx_embedding.input_tensor_name", request.input_tensor_name)?;
        let output_tensor_name =
            required_string("onnx_embedding.output_tensor_name", request.output_tensor_name)?;
        let load_payload = Payload::typed(OnnxLoadConfig {
            model_path: model_path.clone(),
            execution_providers: request.execution_providers.unwrap_or_default(),
            intra_op_num_threads: request
                .intra_op_num_threads
                .map(usize::try_from)
                .transpose()
                .map_err(|_| {
                    invalid_model("onnx_embedding.intra_op_num_threads", "exceeds usize range")
                })?,
            inter_op_num_threads: request
                .inter_op_num_threads
                .map(usize::try_from)
                .transpose()
                .map_err(|_| {
                    invalid_model("onnx_embedding.inter_op_num_threads", "exceeds usize range")
                })?,
        });

        Ok(Self {
            runtime: DriverRuntime::new(
                execution,
                model_spec(ModelFamily::Onnx, Capability::ImageEmbedding, model_path),
                "onnx",
                load_payload,
            ),
            input_tensor_name,
            output_tensor_name,
        })
    }

    pub(crate) async fn load(&self) -> Result<(), CoreError> {
        self.runtime.load().await
    }

    pub(crate) async fn unload(&self) -> Result<(), CoreError> {
        self.runtime.unload().await
    }

    pub(crate) async fn run(
        &self,
        request: dto::OnnxEmbeddingRequest,
    ) -> Result<dto::OnnxEmbeddingResponse, CoreError> {
        let image = request
            .image
            .ok_or_else(|| invalid_model("onnx_embedding.image", "missing required payload"))?;
        let input_tensor = embedding_image_to_tensor(&image.data, &self.input_tensor_name)?;
        let payload = self
            .runtime
            .submit(
                Capability::ImageEmbedding,
                false,
                Payload::Json(onnx_tensors_to_json(&[input_tensor])?),
                Vec::new(),
                Payload::None,
            )
            .await?
            .result()
            .await?;

        Ok(dto::OnnxEmbeddingResponse {
            output: Some(onnx_named_output_from_payload(payload, &self.output_tensor_name)?),
        })
    }
}
