use thiserror::Error;

use crate::provider::HubProvider;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubErrorKind {
    NetworkUnavailable,
    UnsupportedProvider,
    InvalidRepoId,
    OperationFailed,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct HubError {
    kind: HubErrorKind,
    provider: Option<HubProvider>,
    message: String,
}

impl HubError {
    pub fn kind(&self) -> HubErrorKind {
        self.kind
    }

    pub fn provider(&self) -> Option<HubProvider> {
        self.provider
    }

    pub(crate) fn new(
        kind: HubErrorKind,
        provider: Option<HubProvider>,
        message: impl Into<String>,
    ) -> Self {
        Self { kind, provider, message: message.into() }
    }

    pub(crate) fn operation(provider: HubProvider, message: impl Into<String>) -> Self {
        Self::new(HubErrorKind::OperationFailed, Some(provider), message)
    }
}

pub(crate) fn map_reqwest_error(
    provider: HubProvider,
    context: impl Into<String>,
    error: reqwest::Error,
) -> HubError {
    let context = context.into();
    let message = format!("{context}: {error}");
    let kind = if is_reqwest_network_error(&error) {
        HubErrorKind::NetworkUnavailable
    } else {
        HubErrorKind::OperationFailed
    };
    HubError::new(kind, Some(provider), message)
}

pub(crate) fn is_network_message(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    [
        "connection",
        "dns",
        "network",
        "timed out",
        "timeout",
        "tls",
        "refused",
        "unreachable",
        "reset by peer",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

pub(crate) fn is_reqwest_network_error(error: &reqwest::Error) -> bool {
    error.is_connect() || error.is_timeout() || (error.is_request() && !error.is_status())
}

pub(crate) fn is_networkish_error_message(message: &str) -> bool {
    is_network_message(message)
}

pub(crate) fn is_network_io_error(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::TimedOut
            | std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::AddrInUse
            | std::io::ErrorKind::AddrNotAvailable
            | std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::UnexpectedEof
    )
}

#[cfg(feature = "provider-hf-hub")]
pub(crate) fn map_hf_hub_error(
    provider: HubProvider,
    context: impl Into<String>,
    error: hf_hub::api::tokio::ApiError,
) -> HubError {
    let context = context.into();
    let kind = match &error {
        hf_hub::api::tokio::ApiError::RequestError(reqwest_error)
            if is_networkish_error_message(&reqwest_error.to_string()) =>
        {
            HubErrorKind::NetworkUnavailable
        }
        hf_hub::api::tokio::ApiError::IoError(io_error) if is_network_io_error(io_error) => {
            HubErrorKind::NetworkUnavailable
        }
        _ => HubErrorKind::OperationFailed,
    };
    HubError::new(kind, Some(provider), format!("{context}: {error}"))
}

#[cfg(feature = "provider-models-cat")]
pub(crate) fn map_models_cat_error(
    provider: HubProvider,
    context: impl Into<String>,
    error: models_cat::OpsError,
) -> HubError {
    let context = context.into();
    let kind = match &error {
        models_cat::OpsError::RequestError(reqwest_error)
            if is_networkish_error_message(&reqwest_error.to_string()) =>
        {
            HubErrorKind::NetworkUnavailable
        }
        models_cat::OpsError::IoError(io_error) if is_network_io_error(io_error) => {
            HubErrorKind::NetworkUnavailable
        }
        _ => HubErrorKind::OperationFailed,
    };
    HubError::new(kind, Some(provider), format!("{context}: {error}"))
}

#[cfg(feature = "provider-huggingface-hub-rust")]
pub(crate) fn map_huggingface_hub_rust_error(
    provider: HubProvider,
    context: impl Into<String>,
    error: huggingface_hub::HFError,
) -> HubError {
    let message = error.to_string();
    let kind = if is_network_message(&message) {
        HubErrorKind::NetworkUnavailable
    } else {
        HubErrorKind::OperationFailed
    };
    HubError::new(kind, Some(provider), format!("{}: {message}", context.into()))
}
