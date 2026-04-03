use std::path::PathBuf;

use slab_llama::LlamaError;
use thiserror::Error;
use tokio::sync::mpsc;

/// A unique identifier for an inference session.
pub type SessionId = u64;

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

    #[error("Failed to create llama context")]
    CreateContext {
        #[source]
        source: LlamaError,
    },

    #[error("Failed to tokenize prompt")]
    TokenizeFailed {
        #[source]
        source: LlamaError,
    },

    #[error("Failed to apply chat template")]
    ApplyChatTemplate {
        #[source]
        source: LlamaError,
    },

    #[error("Session {session_id} not found")]
    SessionNotFound { session_id: SessionId },

    #[error("Session capacity exceeded: max concurrent sessions per worker is {max_sessions}")]
    SessionCapacityExceeded { max_sessions: usize },

    #[error("Inference worker shut down unexpectedly")]
    WorkerShutdown,

    #[error("Failed to spawn inference worker thread")]
    SpawnWorkerFailed {
        #[source]
        source: std::io::Error,
    },

    #[error("Inference stream error: {message}")]
    InferenceStreamError { message: String },
}

/// A chunk of streaming output from the inference engine.
#[derive(Debug, Clone)]
pub enum StreamChunk {
    /// A piece of generated text.
    Token(String),
    /// Generation completed normally.
    Done,
    /// Generation terminated due to an error.
    Error(String),
}

/// A handle to a streaming generation response.
///
/// Yields [`StreamChunk`] items as tokens are produced.  The stream ends
/// with [`StreamChunk::Done`] or [`StreamChunk::Error`].
pub type StreamHandle = mpsc::Receiver<StreamChunk>;
