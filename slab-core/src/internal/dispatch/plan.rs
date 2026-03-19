use crate::internal::scheduler::stage::CpuStage;
use crate::internal::scheduler::types::Payload;
use crate::spec::{Capability, ModelFamily, TaskKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModelSourceKind {
    LocalPath,
    LocalArtifacts,
    HuggingFace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DriverLoadStyle {
    DynamicLibraryThenModel,
    ModelOnly,
}

#[derive(Debug, Clone)]
pub(crate) struct DriverDescriptor {
    pub driver_id: String,
    pub backend_id: String,
    pub family: ModelFamily,
    pub capability: Capability,
    pub supported_sources: Vec<ModelSourceKind>,
    pub supports_streaming: bool,
    pub load_style: DriverLoadStyle,
    pub priority: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedDriver {
    pub driver_id: String,
    pub backend_id: String,
    pub family: ModelFamily,
    pub capability: Capability,
    pub task_kind: TaskKind,
    pub op_name: String,
    pub supports_streaming: bool,
    pub load_style: DriverLoadStyle,
}

#[derive(Clone)]
pub(crate) struct InvocationPlan {
    pub resolved: ResolvedDriver,
    pub initial_payload: Payload,
    pub preprocess_stages: Vec<CpuStage>,
    pub op_options: Payload,
    pub streaming: bool,
}

impl std::fmt::Debug for InvocationPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InvocationPlan")
            .field("resolved", &self.resolved)
            .field("preprocess_stage_count", &self.preprocess_stages.len())
            .field("streaming", &self.streaming)
            .finish()
    }
}
