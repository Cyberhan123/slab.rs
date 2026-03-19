use std::sync::Arc;

use crate::dispatch::DispatchPlanner;
use crate::model::AutoModel;
use crate::scheduler::kernel::ExecutionKernel;
use crate::scheduler::orchestrator::Orchestrator;
use crate::spec::ModelSpec;

use super::builder::BuiltinDriversConfig;

#[derive(Clone)]
pub struct Runtime {
    inner: Arc<RuntimeInner>,
}

#[derive(Debug)]
pub(crate) struct RuntimeInner {
    pub orchestrator: Orchestrator,
    pub planner: DispatchPlanner,
    pub builtin_drivers: BuiltinDriversConfig,
}

impl Runtime {
    pub(crate) fn new(
        orchestrator: Orchestrator,
        planner: DispatchPlanner,
        builtin_drivers: BuiltinDriversConfig,
    ) -> Self {
        Self {
            inner: Arc::new(RuntimeInner {
                orchestrator,
                planner,
                builtin_drivers,
            }),
        }
    }

    pub fn model(&self, spec: impl Into<ModelSpec>) -> AutoModel {
        AutoModel::new(self.clone(), spec.into())
    }

    pub fn builtin_drivers(&self) -> &BuiltinDriversConfig {
        &self.inner.builtin_drivers
    }

    pub(crate) fn orchestrator(&self) -> Orchestrator {
        self.inner.orchestrator.clone()
    }

    pub(crate) fn kernel(&self) -> ExecutionKernel {
        ExecutionKernel::new(self.inner.orchestrator.clone())
    }

    pub(crate) fn planner(&self) -> &DispatchPlanner {
        &self.inner.planner
    }
}

impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime")
            .field("builtin_drivers", &self.inner.builtin_drivers)
            .field("driver_count", &self.inner.planner.descriptors().len())
            .finish()
    }
}
