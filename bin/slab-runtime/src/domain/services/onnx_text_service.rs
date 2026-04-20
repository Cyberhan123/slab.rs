use slab_runtime_core::backend::RequestRoute;

use crate::application::dtos as dto;
use crate::domain::models::{OnnxInferenceResponse, OnnxLoadConfig};
use crate::domain::runtime::CoreError;

use super::ExecutionHub;
use super::driver_runtime::DriverRuntime;
use super::helpers::{
    contract_tensor_to_raw_tensor, invalid_model, onnx_tensors_to_request, required_path,
};

#[derive(Clone, Debug)]
pub(crate) struct OnnxTextService {
    runtime: DriverRuntime,
}

impl OnnxTextService {
    pub(crate) fn new(
        execution: ExecutionHub,
        request: dto::OnnxTextLoadRequest,
    ) -> Result<Self, CoreError> {
        let model_path = required_path("onnx_text.model_path", request.model_path)?;
        let load_payload = OnnxLoadConfig {
            model_path: model_path.clone(),
            execution_providers: request.execution_providers.unwrap_or_default(),
            intra_op_num_threads: request
                .intra_op_num_threads
                .map(usize::try_from)
                .transpose()
                .map_err(|_| {
                    invalid_model("onnx_text.intra_op_num_threads", "exceeds usize range")
                })?,
            inter_op_num_threads: request
                .inter_op_num_threads
                .map(usize::try_from)
                .transpose()
                .map_err(|_| {
                    invalid_model("onnx_text.inter_op_num_threads", "exceeds usize range")
                })?,
        };

        Ok(Self { runtime: DriverRuntime::new_typed(execution, "onnx.text", "onnx", load_payload) })
    }

    pub(crate) async fn load(&self) -> Result<(), CoreError> {
        self.runtime.load().await
    }

    pub(crate) async fn unload(&self) -> Result<(), CoreError> {
        self.runtime.unload().await
    }

    pub(crate) async fn run(
        &self,
        request: dto::OnnxTextRequest,
    ) -> Result<dto::OnnxTextResponse, CoreError> {
        let response: OnnxInferenceResponse = self
            .runtime
            .invoke_without_options(
                RequestRoute::Inference,
                onnx_tensors_to_request(&request.inputs)?,
                Vec::new(),
            )
            .await?;
        Ok(dto::OnnxTextResponse {
            outputs: response.outputs.into_iter().map(contract_tensor_to_raw_tensor).collect(),
        })
    }
}
