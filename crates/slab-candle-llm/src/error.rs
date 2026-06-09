use thiserror::Error;

#[derive(Debug, Error)]
pub enum CandleLlmError {
    #[error("model not loaded; call load_model first")]
    ModelNotLoaded,
    #[error("unsupported model kind {kind}: {message}")]
    UnsupportedModel { kind: String, message: String },
    #[error("unsupported option {option}: {message}")]
    UnsupportedOption { option: &'static str, message: String },
    #[error("invalid asset layout at {path}: {message}")]
    InvalidAssetLayout { path: String, message: String },
    #[error("prompt must not be empty")]
    EmptyPrompt,
    #[error("max_tokens must be greater than 0")]
    InvalidMaxTokens,
    #[error("invalid sampling config: {message}")]
    InvalidSampling { message: String },
    #[error("failed to load model from {model_path}: {message}")]
    LoadModel { model_path: String, message: String },
    #[error("failed to load tokenizer from {tokenizer_path}: {message}")]
    LoadTokenizer { tokenizer_path: String, message: String },
    #[error("tokenization failed: {message}")]
    Tokenize { message: String },
    #[error("inference failed: {message}")]
    Inference { message: String },
}

impl CandleLlmError {
    pub(crate) fn load_model(path: impl std::fmt::Display, error: impl std::fmt::Display) -> Self {
        Self::LoadModel { model_path: path.to_string(), message: error.to_string() }
    }

    pub(crate) fn inference(message: impl Into<String>) -> Self {
        Self::Inference { message: message.into() }
    }
}
