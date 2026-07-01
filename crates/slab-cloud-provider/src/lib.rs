//! Cloud model-provider infrastructure wrapping the [`genai`] crate.
//!
//! This crate is the single owner of slab's cloud-provider knowledge:
//! - the mapping between slab's [`ProviderFamily`] and genai's [`genai::adapter::AdapterKind`],
//! - cloud-provider credential resolution,
//! - the curated default model catalog used to activate cloud models when a provider is configured.
//!
//! `slab-app-core` consumes this crate; this crate must not depend on `slab-app-core` (the model
//! catalog is returned as plain [`activation::CloudModelSpec`] data so `slab-app-core` owns the
//! DB writes).
//!
//! [`ProviderFamily`]: slab_config::ProviderFamily

pub mod activation;
pub mod adapter_kind;
pub mod error;
pub mod provider;

pub use activation::{CloudModelSpec, default_models_for_provider};
pub use adapter_kind::family_to_adapter_kind;
pub use error::CloudError;
pub use provider::resolve_api_key;
