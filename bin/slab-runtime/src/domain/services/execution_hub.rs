use std::sync::Arc;

use crate::domain::models::RuntimeEnabledBackends;
use crate::domain::runtime::Orchestrator;

#[derive(Clone)]
pub struct ExecutionHub {
    inner: Arc<ExecutionState>,
}

#[derive(Debug)]
pub(crate) struct ExecutionState {
    pub orchestrator: Orchestrator,
    pub enabled_backends: RuntimeEnabledBackends,
}

impl ExecutionHub {
    pub(crate) fn new(
        orchestrator: Orchestrator,
        enabled_backends: RuntimeEnabledBackends,
    ) -> Self {
        Self { inner: Arc::new(ExecutionState { orchestrator, enabled_backends }) }
    }

    pub(crate) fn orchestrator(&self) -> Orchestrator {
        self.inner.orchestrator.clone()
    }

    pub(crate) fn enabled_backends(&self) -> &RuntimeEnabledBackends {
        &self.inner.enabled_backends
    }
}

impl std::fmt::Debug for ExecutionHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionHub")
            .field("service_count", &self.inner.enabled_backends.len())
            .finish()
    }
}
