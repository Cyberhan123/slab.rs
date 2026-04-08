use std::sync::Arc;

use slab_model_pack::ModelPackRuntimeBridge;

use crate::base::error::CoreError;
use crate::internal::dispatch::DriverResolver;
use crate::internal::scheduler::orchestrator::Orchestrator;
use crate::model::ModelSpec;

use super::super::pipeline::Pipeline;

#[derive(Clone)]
pub struct Runtime {
    inner: Arc<RuntimeRegistry>,
}

#[derive(Debug)]
pub(crate) struct RuntimeRegistry {
    pub orchestrator: Orchestrator,
    pub resolver: DriverResolver,
}

impl Runtime {
    pub(crate) fn new(orchestrator: Orchestrator, resolver: DriverResolver) -> Self {
        Self { inner: Arc::new(RuntimeRegistry { orchestrator, resolver }) }
    }

    pub fn pipeline(&self, spec: ModelSpec) -> Result<Pipeline, CoreError> {
        Pipeline::new(self.clone(), spec)
    }

    pub fn pipeline_from_model_pack(
        &self,
        bridge: ModelPackRuntimeBridge,
    ) -> Result<Pipeline, CoreError> {
        Pipeline::new_with_model_pack_bridge(self.clone(), bridge)
    }

    pub(crate) fn orchestrator(&self) -> Orchestrator {
        self.inner.orchestrator.clone()
    }

    pub(crate) fn resolver(&self) -> &DriverResolver {
        &self.inner.resolver
    }
}

impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime").field("driver_count", &self.inner.resolver.descriptors().len()).finish()
    }
}
