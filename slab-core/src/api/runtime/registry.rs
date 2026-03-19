use std::sync::Arc;

use crate::base::error::CoreError;
use crate::internal::dispatch::DriverResolver;
use crate::internal::scheduler::kernel::ExecutionKernel;
use crate::internal::scheduler::orchestrator::Orchestrator;
use crate::model::ModelSpec;

use super::super::pipeline::Pipeline;
use super::builtins::DriversConfig;

#[derive(Clone)]
pub struct Runtime {
    inner: Arc<RuntimeRegistry>,
}

#[derive(Debug)]
pub(crate) struct RuntimeRegistry {
    pub orchestrator: Orchestrator,
    pub resolver: DriverResolver,
    pub drivers: DriversConfig,
}

impl Runtime {
    pub(crate) fn new(
        orchestrator: Orchestrator,
        resolver: DriverResolver,
        drivers: DriversConfig,
    ) -> Self {
        Self {
            inner: Arc::new(RuntimeRegistry {
                orchestrator,
                resolver,
                drivers,
            }),
        }
    }

    pub fn pipeline(&self, spec: ModelSpec) -> Result<Pipeline, CoreError> {
        Pipeline::new(self.clone(), spec)
    }

    pub fn drivers(&self) -> &DriversConfig {
        &self.inner.drivers
    }

    pub(crate) fn orchestrator(&self) -> Orchestrator {
        self.inner.orchestrator.clone()
    }

    pub(crate) fn kernel(&self) -> ExecutionKernel {
        ExecutionKernel::new(self.inner.orchestrator.clone())
    }

    pub(crate) fn resolver(&self) -> &DriverResolver {
        &self.inner.resolver
    }
}

impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime")
            .field("drivers", &self.inner.drivers)
            .field("driver_count", &self.inner.resolver.descriptors().len())
            .finish()
    }
}
