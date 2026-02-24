pub mod diffusion;
pub mod llama;
pub mod whisper;
use thiserror::Error;

#[derive(Debug, Error)]
/// All errors the API can throw
pub enum GGMLEngineError {
    /// I/O Error
    #[error("I/O error {0}")]
    IoError(#[from] std::io::Error),

    #[error("engine/ggml/whisper/error {0}")]
    GGMLWhisperError(#[from] whisper::GGMLWhisperEngineError),

    #[error("engine/ggml/llama/error {0}")]
    GGMLLlamaError(#[from] llama::GGMLLlamaEngineError),

    #[error("engine/ggml/diffusion/error {0}")]
    DiffusionError(#[from] diffusion::GGMLDiffusionEngineError),
}
