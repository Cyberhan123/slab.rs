pub(crate) use super::engine::GGMLDiffusionEngineError;

use thiserror::Error;

#[allow(dead_code)]
#[derive(Debug, Error)]
pub(crate) enum GGMLDiffusionWorkerError {
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
impl GGMLDiffusionWorkerError {
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
