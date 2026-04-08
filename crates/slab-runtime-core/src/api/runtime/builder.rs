use tokio::runtime::Handle;

use crate::base::error::CoreError;
use crate::internal::dispatch::DriverResolver;
use crate::internal::scheduler::backend::admission::{ResourceManager, ResourceManagerConfig};
use crate::internal::scheduler::orchestrator::Orchestrator;

use super::registration::RuntimeBackendRegistration;
use super::registry::Runtime;

pub struct RuntimeBuilder {
    queue_capacity: usize,
    backend_capacity: usize,
    registrations: Vec<RuntimeBackendRegistration>,
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        Self { queue_capacity: 64, backend_capacity: 4, registrations: Vec::new() }
    }

    pub fn queue_capacity(mut self, queue_capacity: usize) -> Self {
        self.queue_capacity = queue_capacity;
        self
    }

    pub fn backend_capacity(mut self, backend_capacity: usize) -> Self {
        self.backend_capacity = backend_capacity;
        self
    }

    pub fn register_backend(mut self, registration: RuntimeBackendRegistration) -> Self {
        self.registrations.push(registration);
        self
    }

    pub fn build(self) -> Result<Runtime, CoreError> {
        ensure_tokio_runtime()?;

        let Self { queue_capacity, backend_capacity, registrations } = self;

        let worker_count = backend_capacity;
        let mut resource_manager = ResourceManager::with_config(ResourceManagerConfig {
            backend_capacity: worker_count,
            ..ResourceManagerConfig::default()
        });

        let mut descriptors = Vec::new();
        for registration in registrations {
            (registration.register)(&mut resource_manager, worker_count)?;
            descriptors.extend(registration.descriptors);
        }
        let resolver = DriverResolver::new(descriptors);
        let orchestrator = Orchestrator::start(resource_manager, queue_capacity);

        Ok(Runtime::new(orchestrator, resolver))
    }
}

fn ensure_tokio_runtime() -> Result<(), CoreError> {
    let _ = Handle::try_current().map_err(|err| CoreError::DeploymentFailed {
        driver_id: "runtime".to_owned(),
        message: format!("RuntimeBuilder::build must run inside a Tokio runtime: {err}"),
    })?;
    Ok(())
}
