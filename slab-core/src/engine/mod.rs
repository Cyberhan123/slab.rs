pub mod ggml;
//todo
pub mod candle;


use thiserror::Error;

#[derive(Debug, Error)]
/// All errors the API can throw
pub enum EngineError {
    /// I/O Error
    #[error("I/O error {0}")]
    IoError(#[from] std::io::Error),

    #[error("GGML error {0}")]
    GGMLEngineError(#[from] ggml::GGMLEngineError),
}

impl From<ggml::whisper::GGMLWhisperEngineError> for EngineError {
    fn from(err: ggml::whisper::GGMLWhisperEngineError) -> Self {
        EngineError::GGMLEngineError(ggml::GGMLEngineError::from(err))
    }
}


impl From<ggml::llama::GGMLLlamaEngineError> for EngineError {
    fn from(err: ggml::llama::GGMLLlamaEngineError) -> Self {
        EngineError::GGMLEngineError(ggml::GGMLEngineError::from(err))
    }
}

impl From<ggml::diffusion::GGMLDiffusionEngineError> for EngineError {
    fn from(err: ggml::diffusion::GGMLDiffusionEngineError) -> Self {
        EngineError::GGMLEngineError(ggml::GGMLEngineError::from(err))
    }
}