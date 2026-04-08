use slab_runtime_core::scheduler::CpuStage;
use slab_runtime_core::{CoreError, Payload};
use slab_types::{Capability, DriverDescriptor, DriverLoadStyle, ModelFamily};

#[derive(Debug, Clone)]
pub(crate) struct ResolvedDriver {
    pub driver_id: String,
    pub backend_id: String,
    pub family: ModelFamily,
    pub capability: Capability,
    pub supports_streaming: bool,
    #[allow(dead_code)]
    pub load_style: DriverLoadStyle,
}

impl ResolvedDriver {
    pub(crate) fn invocation(
        &self,
        capability: Capability,
        streaming: bool,
    ) -> Result<ResolvedInvocation, CoreError> {
        validate_invocation(self, capability, streaming)?;

        Ok(ResolvedInvocation {
            driver: self.clone(),
            op_name: op_name_for(capability, streaming).to_owned(),
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedInvocation {
    pub driver: ResolvedDriver,
    pub op_name: String,
}

#[derive(Clone)]
pub(crate) struct InvocationPlan {
    pub invocation: ResolvedInvocation,
    pub initial_payload: Payload,
    pub preprocess_stages: Vec<CpuStage>,
    pub op_options: Payload,
    pub streaming: bool,
}

impl InvocationPlan {
    pub(crate) fn new(
        resolved: ResolvedDriver,
        capability: Capability,
        streaming: bool,
        initial_payload: Payload,
        preprocess_stages: Vec<CpuStage>,
        op_options: Payload,
    ) -> Result<Self, CoreError> {
        Ok(Self {
            invocation: resolved.invocation(capability, streaming)?,
            initial_payload,
            preprocess_stages,
            op_options,
            streaming,
        })
    }
}

impl std::fmt::Debug for InvocationPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InvocationPlan")
            .field("invocation", &self.invocation)
            .field("preprocess_stage_count", &self.preprocess_stages.len())
            .field("streaming", &self.streaming)
            .finish()
    }
}

fn validate_invocation(
    resolved: &ResolvedDriver,
    capability: Capability,
    streaming: bool,
) -> Result<(), CoreError> {
    if resolved.capability != capability {
        return Err(CoreError::UnsupportedCapability {
            family: format!("{:?}", resolved.family),
            capability: format!("{:?}", capability),
        });
    }

    if streaming && !resolved.supports_streaming {
        return Err(CoreError::UnsupportedOperation {
            backend: resolved.driver_id.clone(),
            op: "stream".to_owned(),
        });
    }

    Ok(())
}

fn op_name_for(capability: Capability, streaming: bool) -> &'static str {
    match (capability, streaming) {
        (Capability::TextGeneration, true) => "inference.stream",
        (Capability::ImageGeneration, _) => "inference.image",
        (Capability::TextGeneration, false)
        | (Capability::AudioTranscription, _)
        | (Capability::ImageEmbedding, _) => "inference",
        // `Capability` is `#[non_exhaustive]`; future variants default to the
        // generic "inference" op until an explicit mapping is added.
        (_, _) => "inference",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_driver_builds_streaming_invocation_route() {
        let resolved = ResolvedDriver {
            driver_id: "candle.llama".to_owned(),
            backend_id: "candle.llama".to_owned(),
            family: ModelFamily::Llama,
            capability: Capability::TextGeneration,
            supports_streaming: true,
            load_style: DriverLoadStyle::ModelOnly,
        };

        let invocation = resolved
            .invocation(Capability::TextGeneration, true)
            .expect("streaming invocation should resolve");

        assert_eq!(invocation.op_name, "inference.stream");
    }

    #[test]
    fn resolved_driver_rejects_unsupported_streaming_invocation() {
        let resolved = ResolvedDriver {
            driver_id: "onnx.text".to_owned(),
            backend_id: "onnx".to_owned(),
            family: ModelFamily::Onnx,
            capability: Capability::TextGeneration,
            supports_streaming: false,
            load_style: DriverLoadStyle::ModelOnly,
        };

        let error = resolved
            .invocation(Capability::TextGeneration, true)
            .expect_err("non-streaming driver should reject stream invocation");

        assert!(matches!(
            error,
            CoreError::UnsupportedOperation { ref op, .. } if op == "stream"
        ));
    }
}
