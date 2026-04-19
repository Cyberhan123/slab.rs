mod cli;
mod server;
mod signals;
mod telemetry;

use std::sync::Arc;

use anyhow::Context;
use slab_runtime_core::backend::{ResourceManager, ResourceManagerConfig};
use tracing::info;

use crate::api::handlers::GrpcServiceImpl;
use crate::application::services::RuntimeApplication;
use crate::domain::models::BackendCatalog;
use crate::domain::runtime::Orchestrator;
use crate::domain::services::ExecutionHub;
use crate::infra::backends;
use crate::infra::config::RuntimeConfig;

pub use cli::Cli;

pub async fn run(cli: Cli) -> anyhow::Result<()> {
    let config = Arc::new(cli.into_runtime_config()?);
    let _log_guards =
        telemetry::init_tracing(&config.log_level, config.log_json, config.log_file.as_deref())?;
    telemetry::install_panic_hook();
    telemetry::log_startup(&config);

    let grpc_service = build_grpc_service(Arc::clone(&config))?;
    info!(grpc_bind = %config.grpc_bind, "starting slab-runtime gRPC server");
    server::serve_grpc(&config.grpc_bind, config.shutdown_on_stdin_close, grpc_service).await?;
    info!("slab-runtime stopped");
    Ok(())
}

fn build_grpc_service(config: Arc<RuntimeConfig>) -> anyhow::Result<GrpcServiceImpl> {
    let drivers = backends::RuntimeDriversConfig::from(config.as_ref());
    let worker_count = config.backend_capacity;
    let mut resource_manager = ResourceManager::with_config(ResourceManagerConfig {
        backend_capacity: worker_count,
        ..ResourceManagerConfig::default()
    });
    backends::register_backends(&drivers, &mut resource_manager, worker_count)
        .context("failed to register runtime backends")?;

    let execution = ExecutionHub::new(
        Orchestrator::start(resource_manager, config.queue_capacity),
        BackendCatalog::new(backends::descriptors(&drivers)),
    );
    let application = RuntimeApplication::new(execution, config.enabled_backends);
    Ok(GrpcServiceImpl::new(application))
}
