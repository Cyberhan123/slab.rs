use std::ffi::{NulError, c_int};
use std::str::Utf8Error;
use thiserror::Error;

/// Errors returned by the parakeet wrapper. If you have not installed the logging
/// hook via [`crate::Parakeet::install_logging_hooks`], the underlying library
/// prints diagnostics to stderr — check there for more detail.
#[derive(Debug, Clone, Error)]
pub enum ParakeetError {
    /// Failed to create a new context.
    #[error("Failed to create a new parakeet context.")]
    InitError,
    /// Failed to calculate the spectrogram for some reason.
    #[error("Failed to calculate the spectrogram for some reason.")]
    UnableToCalculateSpectrogram,
    /// Failed to evaluate model.
    #[error("Failed to evaluate model.")]
    UnableToCalculateEvaluation,
    /// Failed to run the encoder.
    #[error("Failed to run the encoder.")]
    FailedToEncode,
    /// Failed to run the decoder.
    #[error("Failed to run the decoder.")]
    FailedToDecode,
    /// Invalid number of mel bands.
    #[error("Invalid number of mel bands.")]
    InvalidMelBands,
    /// Invalid thread count.
    #[error("Invalid thread count.")]
    InvalidThreadCount,
    /// Invalid UTF-8 detected in a string from parakeet.
    #[error(
        "Invalid UTF-8 detected in a string from parakeet. Valid up to index {valid_up_to}, error length: {error_len:?}"
    )]
    InvalidUtf8 { error_len: Option<usize>, valid_up_to: usize },
    /// A null byte was detected in a user-provided string.
    #[error("A null byte was detected in a user-provided string. Index: {idx}")]
    NullByteInString { idx: usize },
    /// Parakeet returned a null pointer.
    #[error("Parakeet returned a null pointer.")]
    NullPointer,
    /// Generic parakeet error. Varies depending on the function.
    #[error("Generic parakeet error. Varies depending on the function. Error code: {0}")]
    GenericError(c_int),
    /// Parakeet failed to convert the provided text into tokens.
    #[error("Parakeet failed to convert the provided text into tokens.")]
    InvalidText,
    /// Creating a state pointer failed.
    #[error("Creating a state pointer failed.")]
    FailedToCreateState,
    /// No samples were provided.
    #[error("Input sample buffer was empty.")]
    NoSamples,
    /// Failed to load the parakeet dynamic library.
    #[error("Failed to load the parakeet dynamic library: {0}")]
    LoadLibraryError(String),
    /// `ContextParams.model_path` was not set for a file-backed context load.
    #[error("ContextParams.model_path must be set before creating a file-backed parakeet context")]
    ModelPathNotSet,
}

impl From<Utf8Error> for ParakeetError {
    fn from(e: Utf8Error) -> Self {
        Self::InvalidUtf8 { error_len: e.error_len(), valid_up_to: e.valid_up_to() }
    }
}

impl From<NulError> for ParakeetError {
    fn from(e: NulError) -> Self {
        Self::NullByteInString { idx: e.nul_position() }
    }
}

impl From<libloading::Error> for ParakeetError {
    fn from(e: libloading::Error) -> Self {
        Self::LoadLibraryError(e.to_string())
    }
}
