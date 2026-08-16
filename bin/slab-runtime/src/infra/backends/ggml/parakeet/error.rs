pub(crate) use super::engine::GGMLParakeetEngineError;

use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum GGMLParakeetWorkerError {
    #[error("contract error: {message}")]
    Contract { message: String },
    #[error("load failed: {message}")]
    Load { message: String },
    #[error("unload failed: {message}")]
    Unload { message: String },
    #[error("inference failed: {message}")]
    Inference { message: String },
}

impl GGMLParakeetWorkerError {
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
}
