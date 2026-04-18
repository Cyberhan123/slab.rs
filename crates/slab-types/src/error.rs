use thiserror::Error;

/// Global error type for the slab-types crate.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum SlabTypeError {
    /// A required value was missing or null.
    #[error("missing value: {0}")]
    MissingValue(String),

    /// A value failed validation against its schema.
    #[error("validation error at '{path}': {message}")]
    Validation { path: String, message: String },

    /// A value could not be parsed from its raw representation.
    #[error("parse error: {0}")]
    Parse(String),

    /// An internal error occurred during schema generation or processing.
    #[error("internal error: {0}")]
    Internal(String),

    /// A structured validation error with details.
    #[error("{0}")]
    ValidationError(#[from] ValidationError),
}

/// Detailed validation error for request types.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ValidationError {
    #[error("temperature must be between 0.0 and 2.0, got {0}")]
    TemperatureOutOfRange(f32),

    #[error("top_p must be between 0.0 and 1.0, got {0}")]
    TopPOutOfRange(f32),

    #[error("top_k must be positive or zero, got {0}")]
    TopKOutOfRange(i32),

    #[error("min_p must be between 0.0 and 1.0, got {0}")]
    MinPOutOfRange(f32),

    #[error("frequency_penalty must be between -2.0 and 2.0, got {0}")]
    FrequencyPenaltyOutOfRange(f32),

    #[error("presence_penalty must be between -2.0 and 2.0, got {0}")]
    PresencePenaltyOutOfRange(f32),

    #[error("max_tokens must be positive, got {0}")]
    MaxTokensOutOfRange(u32),

    #[error("width must be between 64 and 4096, got {0}")]
    WidthOutOfRange(u32),

    #[error("height must be between 64 and 4096, got {0}")]
    HeightOutOfRange(u32),

    #[error("image count must be positive, got {0}")]
    CountOutOfRange(u32),

    #[error("invalid language code: '{0}'")]
    InvalidLanguageCode(String),

    #[error("prompt cannot be empty")]
    EmptyPrompt,

    #[error("invalid ISO 639 language code: '{0}'")]
    InvalidIso639LanguageCode(String),
}
