use std::ffi::{c_int, NulError};
use std::str::Utf8Error;
use thiserror::Error;

/// If you have not configured a logging trampoline with [crate::whisper_sys_log::install_whisper_log_trampoline] or
/// [crate::whisper_sys_tracing::install_whisper_tracing_trampoline],
/// then `whisper.cpp`'s errors will be output to stderr,
/// so you can check there for more information upon receiving a `WhisperError`.
#[derive(Debug, Clone, Error)]
pub enum WhisperError {
    #[error("Failed to initialize backend: {0}")]
    InitBackendError(String),
    /// Failed to create a new context.
    #[error("Failed to create a new whisper context.")]
    InitError,
    /// User didn't initialize spectrogram
    #[error("User didn't initialize spectrogram.")]
    SpectrogramNotInitialized,
    /// Encode was not called.
    #[error("Encode was not called.")]
    EncodeNotComplete,
    /// Decode was not called.
    #[error("Decode was not called.")]
    DecodeNotComplete,
    /// Failed to calculate the spectrogram for some reason.
    #[error("Failed to calculate the spectrogram for some reason.")]
    UnableToCalculateSpectrogram,
    /// Failed to evaluate model.
    #[error("Failed to evaluate model.")]
    UnableToCalculateEvaluation,
    /// Failed to run the encoder
    #[error("Failed to run the encoder.")]
    FailedToEncode,
    /// Failed to run the decoder
    #[error("Failed to run the decoder.")]
    FailedToDecode,
    /// Invalid number of mel bands.
    #[error("Invalid number of mel bands.")]
    InvalidMelBands,
    /// Invalid thread count
    #[error("Invalid thread count.")]
    InvalidThreadCount,
    /// Invalid UTF-8 detected in a string from Whisper.
    #[error("Invalid UTF-8 detected in a string from Whisper. Valid up to index {valid_up_to}, error length: {error_len:?}")]
    InvalidUtf8 {
        error_len: Option<usize>,
        valid_up_to: usize,
    },
    /// A null byte was detected in a user-provided string.
    #[error("A null byte was detected in a user-provided string. Index: {idx}")]
    NullByteInString {
        idx: usize,
    },
    /// Whisper returned a null pointer.
    #[error("Whisper returned a null pointer.")]
    NullPointer,
    /// Generic whisper error. Varies depending on the function.
    #[error("Generic whisper error. Varies depending on the function. Error code: {0}")]
    GenericError(c_int),
    /// Whisper failed to convert the provided text into tokens.
    #[error("Whisper failed to convert the provided text into tokens.")]
    InvalidText,
    /// Creating a state pointer failed. Check stderr for more information.
    #[error("Creating a state pointer failed.")]
    FailedToCreateState,
    /// No samples were provided.
    #[error("Input sample buffer was empty.")]
    NoSamples,
    /// Input and output slices were not the same length.
    #[error("Input and output slices were not the same length. Input: {input_len}, Output: {output_len}")]
    InputOutputLengthMismatch {
        input_len: usize,
        output_len: usize,
    },
    /// Input slice was not an even number of samples.
    #[error("Input slice was not an even number of samples: got {0} (must be an even number)")]
    HalfSampleMissing(usize),
    /// Failed to load the whisper dynamic library.
    #[error("Failed to load the whisper dynamic library: {0}")]
    LoadLibraryError(String),
    /// `enable_vad(true)` was called before a VAD model path was set.
    #[error("VAD model path must be set via set_vad_model_path before enabling VAD")]
    VadModelPathNotSet,
}

impl From<Utf8Error> for WhisperError {
    fn from(e: Utf8Error) -> Self {
        Self::InvalidUtf8 {
            error_len: e.error_len(),
            valid_up_to: e.valid_up_to(),
        }
    }
}

impl From<NulError> for WhisperError {
    fn from(e: NulError) -> Self {
        Self::NullByteInString {
            idx: e.nul_position(),
        }
    }
}

impl From<libloading::Error> for WhisperError {
    fn from(e: libloading::Error) -> Self {
        Self::LoadLibraryError(e.to_string())
    }
}
