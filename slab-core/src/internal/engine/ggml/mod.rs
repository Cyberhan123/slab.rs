pub(crate) mod config;
pub mod diffusion;
pub mod llama;
pub mod whisper;
use thiserror::Error;

#[derive(Debug, Error)]
/// All errors the API can throw
pub enum GGMLEngineError {
    /// I/O Error
    #[error("I/O error {0}")]
    Io(#[from] std::io::Error),

    #[error("engine/ggml/whisper/error {0}")]
    Whisper(#[from] whisper::GGMLWhisperEngineError),

    #[error("engine/ggml/llama/error {0}")]
    Llama(#[from] llama::GGMLLlamaEngineError),

    #[error("engine/ggml/diffusion/error {0}")]
    Diffusion(#[from] diffusion::GGMLDiffusionEngineError),
}
