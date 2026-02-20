use thiserror::Error;

/// Errors that can be returned by slab-libfetch operations.
#[derive(Debug, Error)]
pub enum FetchError {
    /// An HTTP request failed (network error, non-2xx status, etc.).
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// A filesystem I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to serialize or deserialize JSON.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Failed to extract a ZIP archive.
    #[error("ZIP extraction error: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// The GitHub API response was missing an expected field or had an unexpected shape.
    #[error("Invalid GitHub API response: {message}")]
    InvalidResponse { message: String },

    /// A path could not be represented as UTF-8.
    #[error("Path contains invalid UTF-8: {message}")]
    InvalidPath { message: String },

    /// The currently installed asset belongs to a different repository.
    #[error("Installed asset is for a different repository: {0}")]
    RepositoryMismatch(String),
}
