use std::path::PathBuf;

use slab_llama::{LlamaError, runtime::LlamaRuntimeError};
use thiserror::Error;

pub use slab_llama::runtime::{SessionId, StreamChunk, StreamHandle};

#[derive(Debug, Error)]
pub enum GGMLLlamaEngineError {
    #[error("Lock poisoned while trying to {operation}")]
    LockPoisoned { operation: &'static str },

    #[error("Model path contains invalid UTF-8")]
    InvalidModelPathUtf8,

    #[error("Llama model not loaded")]
    ModelNotLoaded,

    #[error("Invalid llama worker count: {num_workers} (must be > 0)")]
    InvalidWorkerCount { num_workers: usize },

    #[error("Failed to initialize llama dynamic library at: {path}")]
    InitializeDynamicLibrary {
        path: PathBuf,
        #[source]
        source: libloading::Error,
    },

    #[error("Failed to load llama model from: {model_path}")]
    LoadModel {
        model_path: String,
        #[source]
        source: LlamaError,
    },

    #[error("Session key '{key}' is already active")]
    SessionKeyBusy { key: String },

    #[error(transparent)]
    Runtime(#[from] LlamaRuntimeError),

    #[error("Inference stream error: {message}")]
    InferenceStreamError { message: String },
}
