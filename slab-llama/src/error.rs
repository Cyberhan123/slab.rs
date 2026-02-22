use std::ffi::NulError;
use std::str::Utf8Error;

/// Errors that can occur when using the llama API.
#[derive(Debug, thiserror::Error)]
pub enum LlamaError {
    /// Failed to initialize or load something (null pointer returned).
    #[error("llama.cpp returned a null pointer")]
    NullPointer,

    /// A null byte was detected in a user-provided string.
    #[error("null byte in user-provided string: {0}")]
    NullByteInString(#[from] NulError),

    /// Invalid UTF-8 detected in a string from llama.cpp.
    #[error("invalid UTF-8 in string from llama.cpp: {0}")]
    InvalidUtf8(#[from] Utf8Error),

    /// Failed to load the model (llama returned null).
    #[error("failed to load the llama model")]
    ModelLoadFailed,

    /// Failed to create a context (llama returned null).
    #[error("failed to create llama context")]
    ContextCreateFailed,

    /// Tokenization failed (returned negative token count).
    #[error("tokenization failed with code {0}")]
    TokenizeFailed(i32),

    /// Decode failed.
    #[error("decode failed with code {0}")]
    DecodeFailed(i32),

    /// Token-to-piece conversion failed.
    #[error("token to piece conversion failed with code {0}")]
    TokenToPieceFailed(i32),

    /// Batch is full - cannot add more tokens.
    #[error("batch is full, cannot add more tokens")]
    BatchFull,

    /// Failed to load a LoRA adapter (llama returned null).
    #[error("failed to load LoRA adapter")]
    LoraAdapterLoadFailed,

    /// Failed to set LoRA adapters on a context.
    #[error("failed to set LoRA adapters with code {0}")]
    SetAdaptersFailed(i32),

    /// A state operation failed (returned 0 bytes).
    #[error("llama state operation failed")]
    StateFailed,

    /// A LoRA adapter metadata lookup failed (key not found or index out of range).
    #[error("LoRA adapter metadata lookup failed")]
    AdapterMetaFailed,
}
