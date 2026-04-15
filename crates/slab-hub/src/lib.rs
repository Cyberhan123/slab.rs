mod client;
mod endpoints;
mod error;
mod progress;
mod provider;
mod providers;

pub use client::HubClient;
pub use endpoints::HubEndpoints;
pub use error::{HubError, HubErrorKind};
pub use progress::{DownloadProgress, DownloadProgressUpdate};
pub use provider::{HubProvider, HubProviderPreference};

#[cfg(test)]
mod tests;
