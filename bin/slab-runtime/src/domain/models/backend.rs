use slab_runtime_core::scheduler::CpuStage;
use slab_runtime_core::{CoreError, Payload};
use slab_types::{
    Capability, DriverDescriptor, DriverLoadStyle, ModelFamily, ModelSource, ModelSourceKind,
    ModelSpec,
};

#[derive(Debug, Clone)]
pub(crate) struct ResolvedBackend {
    pub driver_id: String,
    pub backend_id: String,
    pub family: ModelFamily,
    pub capability: Capability,
    pub supports_streaming: bool,
    #[allow(dead_code)]
    pub load_style: DriverLoadStyle,
}

impl ResolvedBackend {
    pub(crate) fn invocation(
        &self,
        capability: Capability,
        streaming: bool,
    ) -> Result<ResolvedInvocation, CoreError> {
        validate_invocation(self, capability, streaming)?;

        Ok(ResolvedInvocation {
            backend: self.clone(),
            op_name: op_name_for(capability, streaming).to_owned(),
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedInvocation {
    pub backend: ResolvedBackend,
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
        resolved: ResolvedBackend,
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

#[derive(Debug, Clone, Default)]
pub(crate) struct BackendCatalog {
    descriptors: Vec<DriverDescriptor>,
}

impl BackendCatalog {
    pub(crate) fn new(descriptors: Vec<DriverDescriptor>) -> Self {
        Self { descriptors }
    }

    pub(crate) fn descriptors(&self) -> &[DriverDescriptor] {
        &self.descriptors
    }

    pub(crate) fn bind_for_target(
        &self,
        spec: &ModelSpec,
        target_id: &str,
        capability: Capability,
        streaming: bool,
    ) -> Result<ResolvedBackend, CoreError> {
        let source_kind = validate_runtime_capability(spec, capability)?;
        let mut candidates: Vec<&DriverDescriptor> = self
            .descriptors
            .iter()
            .filter(|descriptor| descriptor.family == spec.family)
            .filter(|descriptor| descriptor.capability == spec.capability)
            .filter(|descriptor| descriptor.supported_sources.contains(&source_kind))
            .filter(|descriptor| {
                descriptor.driver_id == target_id || descriptor.backend_id == target_id
            })
            .collect();

        if candidates.is_empty() {
            return Err(CoreError::NoViableDriver {
                family: format!("{:?}", spec.family),
                capability: format!("{:?}", spec.capability),
            });
        }

        if streaming {
            if !candidates.iter().any(|descriptor| descriptor.supports_streaming) {
                return Err(CoreError::UnsupportedOperation {
                    backend: target_id.to_owned(),
                    op: "stream".to_owned(),
                });
            }

            candidates.retain(|descriptor| descriptor.supports_streaming);
        }

        candidates.sort_by(|left, right| {
            left.priority.cmp(&right.priority).then_with(|| left.driver_id.cmp(&right.driver_id))
        });
        Ok(to_resolved_backend(candidates[0]))
    }
}

fn validate_runtime_capability(
    spec: &ModelSpec,
    capability: Capability,
) -> Result<ModelSourceKind, CoreError> {
    if !spec.capability.is_runtime_execution() || !capability.is_runtime_execution() {
        return Err(CoreError::UnsupportedCapability {
            family: format!("{:?}", spec.family),
            capability: format!("{:?}", capability),
        });
    }

    if spec.capability != capability {
        return Err(CoreError::UnsupportedCapability {
            family: format!("{:?}", spec.family),
            capability: format!("{:?}", capability),
        });
    }

    Ok(source_kind(&spec.source))
}

fn to_resolved_backend(descriptor: &DriverDescriptor) -> ResolvedBackend {
    ResolvedBackend {
        driver_id: descriptor.driver_id.clone(),
        backend_id: descriptor.backend_id.clone(),
        family: descriptor.family,
        capability: descriptor.capability,
        supports_streaming: descriptor.supports_streaming,
        load_style: descriptor.load_style,
    }
}

fn validate_invocation(
    resolved: &ResolvedBackend,
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
        (_, _) => "inference",
    }
}

fn source_kind(source: &ModelSource) -> ModelSourceKind {
    match source {
        ModelSource::LocalPath { .. } => ModelSourceKind::LocalPath,
        ModelSource::LocalArtifacts { .. } => ModelSourceKind::LocalArtifacts,
        ModelSource::HuggingFace { .. } => ModelSourceKind::HuggingFace,
        _ => ModelSourceKind::LocalPath,
    }
}
