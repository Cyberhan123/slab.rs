//! Cloud-provider credential resolution.

use slab_config::CloudProviderConfig;
use slab_config::secret_port::{EnvSecretAdapter, resolve_secret_or_plain};
use tracing::warn;

use crate::adapter_kind::family_to_adapter_kind;
use crate::error::CloudError;

/// Resolve the effective API key for a configured provider.
///
/// Resolution order (mirrors the legacy `cloud.rs` behavior, plus a genai fallback):
/// 1. `api_key` — plaintext passes through; `secret://env/<VAR>` handles resolve in-process.
/// 2. `api_key_env` — read the named env var; tolerate a literal key pasted into the field.
/// 3. The adapter kind's canonical env var (e.g. `OPENAI_API_KEY`) if the user left both blank,
///    so a freshly configured provider works as soon as the canonical env var is set.
pub fn resolve_api_key(provider: &CloudProviderConfig) -> Result<String, CloudError> {
    if let Some(key) = provider.api_key.as_deref().filter(|key| !key.trim().is_empty()) {
        return resolve_secret_or_plain(&EnvSecretAdapter::default(), key).map_err(|message| {
            CloudError::KeyResolutionFailed { provider_id: provider.id.clone(), message }
        });
    }

    if let Some(env_key) = provider.api_key_env.as_deref() {
        let env_key = env_key.trim();
        if let Some(value) = env_value(env_key) {
            return Ok(value);
        }

        // Be tolerant to common misconfiguration: users paste a literal API key into `api_key_env`.
        if !env_key.is_empty() && !looks_like_env_var_name(env_key) {
            warn!(
                provider_id = %provider.id,
                "api_key_env does not look like an env var name; treating it as a literal api key"
            );
            return Ok(env_key.to_owned());
        }
    }

    // Fall back to the adapter kind's canonical env var (e.g. OPENAI_API_KEY, ANTHROPIC_API_KEY).
    if let Some(default_env) = family_to_adapter_kind(provider.family).default_key_env_name()
        && let Some(value) = env_value(default_env)
    {
        return Ok(value);
    }

    Err(CloudError::MissingApiKey { provider_id: provider.id.clone() })
}

/// Read `name` from the environment, returning its trimmed value only when non-empty.
fn env_value(name: &str) -> Option<String> {
    let value = std::env::var(name).ok()?;
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// Whether a string looks like an env-var name (used to decide tolerance for `api_key_env`).
fn looks_like_env_var_name(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(ch) if ch == '_' || ch.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;
    use slab_config::{CloudProviderConfig, ProviderFamily};

    fn provider(id: &str, family: ProviderFamily) -> CloudProviderConfig {
        CloudProviderConfig {
            id: id.to_owned(),
            family,
            name: id.to_owned(),
            api_base: String::new(),
            api_key: None,
            api_key_env: None,
        }
    }

    #[test]
    fn plaintext_api_key_passes_through() {
        let p = CloudProviderConfig {
            api_key: Some("sk-test".to_owned()),
            ..provider("openai", ProviderFamily::Openai)
        };
        assert_eq!(resolve_api_key(&p).unwrap(), "sk-test");
    }

    #[test]
    fn missing_key_is_missing_api_key_error() {
        // Use an adapter whose canonical env var is unset in test envs.
        let p = provider("cohere", ProviderFamily::Cohere);
        // SAFETY: single-threaded unit test.
        unsafe { std::env::remove_var("COHERE_API_KEY") };
        let err = resolve_api_key(&p).unwrap_err();
        assert!(matches!(err, CloudError::MissingApiKey { .. }));
        assert_eq!(
            err.to_string(),
            "cloud provider 'cohere' is missing api key (set settings api_key or api_key_env)"
        );
    }
}
