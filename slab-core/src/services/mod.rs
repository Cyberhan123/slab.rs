pub mod whisper;
pub mod ffmpeg;
pub mod dylib;
pub mod subtitle;
pub mod llama;
pub mod diffusion;
use thiserror::Error;

#[derive(Debug, Error)]
/// All errors the API can throw
pub enum ServiceError {
    /// I/O Error
    #[error("I/O error {0}")]
    IoError(#[from] std::io::Error),

    #[error("FFmpeg service error {0}")]
    FFmpegError(#[from] ffmpeg::FFmpegServiceError),

    #[error("Whisper service error {0}")]
    WhisperError(#[from] whisper::WhisperServiceError),

    #[error("Llama service error {0}")]
    LlamaError(#[from] llama::LlamaServiceError),

    #[error("Diffusion service error {0}")]
    DiffusionError(#[from] diffusion::DiffusionServiceError),

    /// source and Display delegate to anyhow::Error
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}