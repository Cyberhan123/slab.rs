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

#[allow(dead_code)]
#[derive(Debug, Error)]
pub(crate) enum OnnxWorkerError {
    #[error("contract error: {message}")]
    Contract { message: String },
    #[error("load failed: {message}")]
    Load { message: String },
    #[error("unload failed: {message}")]
    Unload { message: String },
    #[error("inference failed: {message}")]
    Inference { message: String },
    #[error("sync failed: {message}")]
    Sync { message: String },
    #[error("internal error: {message}")]
    Internal { message: String },
}

#[allow(dead_code)]
impl OnnxWorkerError {
    pub(crate) fn contract(message: impl Into<String>) -> Self {
        Self::Contract { message: message.into() }
    }

    pub(crate) fn load(message: impl Into<String>) -> Self {
        Self::Load { message: message.into() }
    }

    pub(crate) fn unload(message: impl Into<String>) -> Self {
        Self::Unload { message: message.into() }
    }

    pub(crate) fn inference(message: impl Into<String>) -> Self {
        Self::Inference { message: message.into() }
    }

    pub(crate) fn sync(message: impl Into<String>) -> Self {
        Self::Sync { message: message.into() }
    }

    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self::Internal { message: message.into() }
    }
}
