use slab_runtime_core::backend::RequestRoute;

use crate::application::dtos as dto;
use crate::domain::models::{OnnxInferenceRequest, OnnxInferenceResponse, OnnxLoadConfig};
use crate::domain::runtime::CoreError;

use super::ExecutionHub;
use super::driver_runtime::DriverRuntime;
use super::helpers::{
    contract_tensor_to_raw_tensor, embedding_image_to_contract_tensor, invalid_model,
    required_path, required_string,
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
        let load_payload = OnnxLoadConfig {
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
        };

        Ok(Self {
            runtime: DriverRuntime::new_typed(execution, "onnx.embedding", "onnx", load_payload),
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
        let input_tensor =
            embedding_image_to_contract_tensor(&image.data, &self.input_tensor_name)?;
        let response: OnnxInferenceResponse = self
            .runtime
            .invoke_without_options(
                RequestRoute::Inference,
                OnnxInferenceRequest { inputs: vec![input_tensor] },
                Vec::new(),
            )
            .await?;

        let output = response
            .outputs
            .into_iter()
            .find(|tensor| tensor.name == self.output_tensor_name)
            .ok_or_else(|| CoreError::ResultDecodeFailed {
                task_kind: "onnx.embedding".to_owned(),
                message: format!("ONNX output tensor `{}` not found", self.output_tensor_name),
            })?;

        Ok(dto::OnnxEmbeddingResponse { output: Some(contract_tensor_to_raw_tensor(output)) })
    }
}
