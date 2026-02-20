pub mod whisper;
pub mod ffmpeg;
pub mod dylib;
pub mod subtitle;
use thiserror::Error;

#[derive(Debug, Error)]
/// All errors the API can throw
pub enum ServiceError {
    /// I/O Error
    #[error("I/O error {0}")]
    IoError(#[from] std::io::Error),

    /// The part file is corrupted
    #[error("Invalid part file - corrupted file")]
    InvalidResume,

    #[error("FFmpeg service error {0}")]
    FFmpegError(#[from] ffmpeg::FFmpegServiceError),

    #[error("Whisper service error {0}")]
    WhisperError(#[from] whisper::WhisperServiceError),

    /// source and Display delegate to anyhow::Error
    #[error(transparent)]
    Other(#[from] anyhow::Error),  
}