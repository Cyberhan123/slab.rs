use std::sync::Arc;

use slab_runtime_core::scheduler::Orchestrator;
use slab_types::ModelSpec;

use super::backend_session::BackendSession;
use crate::domain::models::BackendCatalog;

#[derive(Clone)]
pub struct ExecutionHub {
    inner: Arc<ExecutionState>,
}

#[derive(Debug)]
pub(crate) struct ExecutionState {
    pub orchestrator: Orchestrator,
    pub catalog: BackendCatalog,
}

impl ExecutionHub {
    pub(crate) fn new(orchestrator: Orchestrator, catalog: BackendCatalog) -> Self {
        Self { inner: Arc::new(ExecutionState { orchestrator, catalog }) }
    }

    pub fn session_for_backend(
        &self,
        spec: ModelSpec,
        backend_target: impl Into<String>,
    ) -> Result<BackendSession, slab_runtime_core::CoreError> {
        BackendSession::new_for_backend(self.clone(), spec, backend_target)
    }

    pub(crate) fn orchestrator(&self) -> Orchestrator {
        self.inner.orchestrator.clone()
    }

    pub(crate) fn catalog(&self) -> &BackendCatalog {
        &self.inner.catalog
    }
}

impl std::fmt::Debug for ExecutionHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionHub")
            .field("driver_count", &self.inner.catalog.descriptors().len())
            .finish()
    }
}