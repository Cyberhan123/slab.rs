use std::sync::Arc;

use anyhow::Context as _;
use slab_runtime_core::backend::{ResourceManager, ResourceManagerConfig};
use slab_runtime_core::scheduler::Orchestrator;

use crate::config::RuntimeConfig;
use crate::domain::runtime::{DriverResolver, Runtime};
use crate::infra::{backends, grpc::GrpcServiceImpl};

#[derive(Clone)]
pub struct RuntimeContext {
    pub config: Arc<RuntimeConfig>,
    pub runtime: Runtime,
    pub grpc_service: GrpcServiceImpl,
}

impl RuntimeContext {
    pub fn new(config: Arc<RuntimeConfig>) -> anyhow::Result<Self> {
        let drivers = backends::RuntimeDriversConfig::from(config.as_ref());
        let worker_count = config.backend_capacity;
        let mut resource_manager = ResourceManager::with_config(ResourceManagerConfig {
            backend_capacity: worker_count,
            ..ResourceManagerConfig::default()
        });
        backends::register_backends(&drivers, &mut resource_manager, worker_count)
            .context("failed to register runtime backends")?;

        let runtime = Runtime::new(
            Orchestrator::start(resource_manager, config.queue_capacity),
            DriverResolver::new(backends::descriptors(&drivers)),
        );
        let grpc_service = GrpcServiceImpl::new(runtime.clone(), config.enabled_backends);

        Ok(Self { config, runtime, grpc_service })
    }
}
