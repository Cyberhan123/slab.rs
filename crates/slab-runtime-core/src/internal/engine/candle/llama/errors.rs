use thiserror::Error;
use tokio::sync::mpsc;

/// A unique identifier for a Candle LLaMA inference session.
pub type SessionId = u64;

#[derive(Debug, Error)]
pub enum CandleLlamaEngineError {
    #[error("model not loaded; call model.load first")]
    ModelNotLoaded,

    #[error("lock poisoned while trying to {operation}")]
    LockPoisoned { operation: &'static str },

    #[error("invalid worker count: {num_workers} (must be > 0)")]
    InvalidWorkerCount { num_workers: usize },

    #[error("model path contains invalid UTF-8")]
    InvalidModelPathUtf8,

    #[error("failed to load model from {model_path}: {message}")]
    LoadModel { model_path: String, message: String },

    #[error("failed to load tokenizer from {tokenizer_path}: {message}")]
    LoadTokenizer { tokenizer_path: String, message: String },

    #[error("tokenizer not found: no tokenizer.json in {dir}")]
    TokenizerNotFound { dir: String },

    #[error("tokenisation failed: {message}")]
    TokenizeFailed { message: String },

    #[error("candle inference error: {message}")]
    Inference { message: String },

    #[error("inference worker shut down unexpectedly")]
    WorkerShutdown,

    #[error("session {session_id} not found")]
    SessionNotFound { session_id: SessionId },

    #[error("inference stream error: {message}")]
    InferenceStreamError { message: String },
}

/// A chunk of streaming output from the Candle LLaMA inference engine.
#[derive(Debug, Clone)]
pub enum StreamChunk {
    /// A piece of generated text.
    Token(String),
    /// Generation completed normally.
    Done,
    /// Generation terminated with an error.
    Error(String),
}

/// A handle to a streaming generation response.
pub type StreamHandle = mpsc::Receiver<StreamChunk>;
