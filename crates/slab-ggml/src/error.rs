use std::ffi::NulError;
use std::str::Utf8Error;

/// Errors that can occur when using the GGML API.
#[derive(Debug, thiserror::Error)]
pub enum GGMLError {
    #[error("GGML error: can't find parent directory of library path")]
    NotParentDir,

    #[error("GGML error loading library: {0}")]
    LibraryLoadError(#[from] ::libloading::Error),

    #[error("GGML symbol '{symbol}' is unavailable: {message}")]
    MissingSymbol { symbol: &'static str, message: String },

    /// Failed to initialize or load something (null pointer returned).
    #[error("GGML returned a null pointer")]
    NullPointer,

    /// A null byte was detected in a user-provided string.
    #[error("null byte in user-provided string: {0}")]
    NullByteInString(#[from] NulError),

    /// Invalid UTF-8 detected in a string from GGML.
    #[error("invalid UTF-8 in string from GGML: {0}")]
    InvalidUtf8(#[from] Utf8Error),
}
