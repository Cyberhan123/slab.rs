pub mod diffusion;
pub mod llama;
pub mod whisper;
use thiserror::Error;

#[derive(Debug, Error)]
/// Top-level error type for the Candle engine layer.
pub enum CandleEngineError {
    #[error("engine/candle/llama/error {0}")]
    Llama(#[from] llama::CandleLlamaEngineError),

    #[error("engine/candle/whisper/error {0}")]
    Whisper(#[from] whisper::CandleWhisperEngineError),

    #[error("engine/candle/diffusion/error {0}")]
    Diffusion(#[from] diffusion::CandleDiffusionEngineError),
}
