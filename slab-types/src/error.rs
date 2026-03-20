use thiserror::Error;

/// Global error type for the slab-types crate.
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
}
