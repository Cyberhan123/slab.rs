pub mod adapter;
pub mod backend;
pub(crate) mod config;

use slab_runtime_core::CoreError;
use slab_runtime_core::backend::{ResourceManager, spawn_workers};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OnnxEngineError {
    #[error("ONNX model session not loaded; call model.load first")]
    SessionNotLoaded,

    #[error("Failed to create ONNX session from '{path}': {source}")]
    SessionCreate {
        path: String,
        #[source]
        source: ort::Error,
    },

    #[error("ONNX inference failed: {source}")]
    InferenceFailed {
        #[source]
        source: ort::Error,
    },

    #[error("Failed to decode input tensor '{name}': {reason}")]
    TensorDecode { name: String, reason: String },

    #[error("Failed to encode output tensor '{name}': {reason}")]
    TensorEncode { name: String, reason: String },

    #[error("Invalid ONNX backend payload: {0}")]
    InvalidPayload(String),
}

impl From<OnnxEngineError> for slab_runtime_core::CoreError {
    fn from(error: OnnxEngineError) -> Self {
        slab_runtime_core::CoreError::OnnxEngine(error.to_string())
    }
}

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
        spawn_workers(shared_rx, control_tx, count, move |worker_id, bc_tx| {
            backend::OnnxWorker::new(bc_tx, worker_id)
        });
    });

    Ok(())
}
