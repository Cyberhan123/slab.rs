pub(crate) use super::engine::GGMLDiffusionEngineError;

use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum GGMLDiffusionWorkerError {
    #[error("load failed: {message}")]
    Load { message: String },
    #[error("unload failed: {message}")]
    Unload { message: String },
    #[error("inference failed: {message}")]
    Inference { message: String },
}

impl GGMLDiffusionWorkerError {
    pub(crate) fn load(message: impl Into<String>) -> Self {
        Self::Load { message: message.into() }
    }

    pub(crate) fn unload(message: impl Into<String>) -> Self {
        Self::Unload { message: message.into() }
    }

    pub(crate) fn inference(message: impl Into<String>) -> Self {
        Self::Inference { message: message.into() }
    }
}
