use std::sync::Arc;

use crate::domain::models::BackendCatalog;
use crate::domain::runtime::Orchestrator;

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
