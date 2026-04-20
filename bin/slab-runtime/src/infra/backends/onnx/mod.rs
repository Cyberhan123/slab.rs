mod contract;
mod engine;
mod error;
mod worker;

use slab_runtime_core::CoreError;
use slab_runtime_core::backend::{ResourceManager, spawn_workers};

#[derive(Debug, Clone, Copy, Default)]
pub struct OnnxBackendConfig {
    pub enabled: bool,
}

pub fn service_ids(config: &OnnxBackendConfig) -> Vec<&'static str> {
    if !config.enabled {
        return Vec::new();
    }

    vec!["onnx.text", "onnx.embedding"]
}

pub fn register(
    config: &OnnxBackendConfig,
    resource_manager: &mut ResourceManager,
    worker_count: usize,
) -> Result<(), CoreError> {
    if !config.enabled {
        return Ok(());
    }

    resource_manager.register_backend("onnx", move |shared_rx, control_tx| {
        let count = worker_count.max(1);
        spawn_workers(shared_rx, control_tx, count, move |peer_bus| worker::OnnxWorker::new(peer_bus));
    });

    Ok(())
}
