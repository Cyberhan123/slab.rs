//! Errors raised by cloud-provider resolution and (eventually) genai chat execution.

use thiserror::Error;

/// Failures from cloud-provider credential resolution and cloud chat execution.
///
/// Variants carry enough structure for `slab-app-core` to map them onto its own
/// `AppCoreError` (bad request vs. backend-not-ready) without this crate depending on it.
#[derive(Debug, Error)]
pub enum CloudError {
    /// A provider has neither an `api_key`, an `api_key_env`, nor a canonical env var set.
    #[error(
        "cloud provider '{provider_id}' is missing api key (set settings api_key or api_key_env)"
    )]
    MissingApiKey { provider_id: String },

    /// An `api_key`/`api_key_env` value was present but could not be resolved to a secret.
    #[error("cloud provider '{provider_id}' api key could not be resolved: {message}")]
    KeyResolutionFailed { provider_id: String, message: String },

    /// A cloud chat/stream request failed with a 4xx-class error (bad model, bad request).
    #[error("{0}")]
    BadRequest(String),

    /// A cloud chat/stream request failed with a transport/5xx-class error (provider down).
    #[error("{0}")]
    BackendNotReady(String),
}

impl CloudError {
    /// Whether callers should treat this as a client-side (4xx) error.
    pub fn is_bad_request(&self) -> bool {
        matches!(self, Self::KeyResolutionFailed { .. } | Self::BadRequest(_))
    }
}
