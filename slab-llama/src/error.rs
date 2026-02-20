use std::ffi::NulError;
use std::str::Utf8Error;

/// Errors that can occur when using the llama API.
#[derive(Debug, Clone)]
pub enum LlamaError {
    /// Failed to initialize or load something (null pointer returned).
    NullPointer,
    /// A null byte was detected in a user-provided string.
    NullByteInString { idx: usize },
    /// Invalid UTF-8 detected in a string from llama.cpp.
    InvalidUtf8 {
        error_len: Option<usize>,
        valid_up_to: usize,
    },
    /// Failed to load the model (llama returned null).
    ModelLoadFailed,
    /// Failed to create a context (llama returned null).
    ContextCreateFailed,
    /// Tokenization failed (returned negative token count).
    TokenizeFailed(i32),
    /// Decode failed.
    DecodeFailed(i32),
    /// Token-to-piece conversion failed.
    TokenToPieceFailed(i32),
    /// Batch is full - cannot add more tokens.
    BatchFull,
}

impl From<NulError> for LlamaError {
    fn from(e: NulError) -> Self {
        Self::NullByteInString {
            idx: e.nul_position(),
        }
    }
}

impl From<Utf8Error> for LlamaError {
    fn from(e: Utf8Error) -> Self {
        Self::InvalidUtf8 {
            error_len: e.error_len(),
            valid_up_to: e.valid_up_to(),
        }
    }
}

impl std::fmt::Display for LlamaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlamaError::NullPointer => write!(f, "llama.cpp returned a null pointer"),
            LlamaError::NullByteInString { idx } => {
                write!(f, "Null byte in user-provided string at index {}", idx)
            }
            LlamaError::InvalidUtf8 {
                valid_up_to,
                error_len: Some(len),
            } => write!(
                f,
                "Invalid UTF-8 at index {} (error length {})",
                valid_up_to, len
            ),
            LlamaError::InvalidUtf8 {
                valid_up_to,
                error_len: None,
            } => write!(f, "Invalid UTF-8 at index {}", valid_up_to),
            LlamaError::ModelLoadFailed => write!(f, "Failed to load the llama model"),
            LlamaError::ContextCreateFailed => write!(f, "Failed to create llama context"),
            LlamaError::TokenizeFailed(code) => {
                write!(f, "Tokenization failed with code {}", code)
            }
            LlamaError::DecodeFailed(code) => write!(f, "Decode failed with code {}", code),
            LlamaError::TokenToPieceFailed(code) => {
                write!(f, "Token to piece conversion failed with code {}", code)
            }
            LlamaError::BatchFull => write!(f, "Batch is full, cannot add more tokens"),
        }
    }
}

impl std::error::Error for LlamaError {}
