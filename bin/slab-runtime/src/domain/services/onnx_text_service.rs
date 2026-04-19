use slab_runtime_core::Payload;
use slab_runtime_core::backend::RequestRoute;

use crate::application::dtos as dto;
use crate::domain::models::OnnxLoadConfig;
use crate::domain::runtime::CoreError;

use super::ExecutionHub;
use super::driver_runtime::DriverRuntime;
use super::helpers::{
    invalid_model, onnx_outputs_from_payload, onnx_tensors_to_json, required_path,
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
        let load_payload = Payload::typed(OnnxLoadConfig {
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
        });

        Ok(Self { runtime: DriverRuntime::new(execution, "onnx", load_payload) })
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
        let payload = self
            .runtime
            .submit(
                RequestRoute::Inference,
                Payload::Json(onnx_tensors_to_json(&request.inputs)?),
                Vec::new(),
                Payload::None,
            )
            .await?
            .result()
            .await?;
        Ok(dto::OnnxTextResponse { outputs: onnx_outputs_from_payload(payload)? })
    }
}
