//! Live remote-model discovery for OpenAI-compatible provider endpoints.
//!
//! Families without a curated catalog (see [`crate::activation`]) have no static model list, but
//! OpenAI-compatible endpoints expose the standard `GET {api_base}/models` listing. genai's
//! `Client::all_model_names` implements exactly that call (with Bearer auth and the
//! `{data: [{id}]}` response shape), so discovery reuses the same adapter stack the chat path
//! uses. This is a live web call: callers own caching, timeouts, and failure fallback — a failed
//! discovery must never clear previously known models.

use genai::Client as GenaiClient;
use genai::resolver::{AuthData, Endpoint};
use slab_config::{CloudProviderConfig, ProviderFamily};
use tracing::warn;

use crate::adapter_kind::family_to_adapter_kind;
use crate::error::CloudError;
use crate::provider::resolve_api_key;

/// Whether the family's endpoint speaks the OpenAI `/models` listing protocol and has no
/// curated catalog to fall back on.
///
/// `OpenaiCompatible` is the "custom endpoint" family: the model list is only knowable by
/// asking the endpoint itself. First-party families ship curated catalogs instead.
pub fn supports_live_discovery(family: ProviderFamily) -> bool {
    matches!(family, ProviderFamily::OpenaiCompatible)
}

/// Fetch the remote model ids advertised by the provider's endpoint.
///
/// Returns an empty list (not an error) when the family does not support live discovery or the
/// provider has no `api_base` to probe. Credential or transport failures surface as
/// [`CloudError`]; the caller decides whether to keep the previously discovered catalog.
pub async fn list_remote_model_ids(
    provider: &CloudProviderConfig,
) -> Result<Vec<String>, CloudError> {
    if !supports_live_discovery(provider.family) {
        return Ok(Vec::new());
    }
    if provider.api_base.trim().is_empty() {
        warn!(
            provider_id = %provider.id,
            "skipping live model discovery: provider has no api_base"
        );
        return Ok(Vec::new());
    }

    let api_key = resolve_api_key(provider)?;
    let endpoint = Endpoint::from_owned(ensure_trailing_slash(&provider.api_base)?);
    let adapter_kind = family_to_adapter_kind(provider.family);

    let models = GenaiClient::default()
        .all_model_names(adapter_kind, (endpoint, AuthData::from_single(api_key)))
        .await
        .map_err(|error| {
            CloudError::BackendNotReady(format!(
                "listing models for cloud provider '{}' failed: {error}",
                provider.id
            ))
        })?;
    Ok(models)
}

/// Normalize a provider `api_base` so genai's `format!("{base_url}models")` produces a valid
/// `/models` URL. Mirrors `slab-app-core`'s `ensure_http_base_url` rule (that helper is
/// `pub(crate)` there, and this crate must not depend on `slab-app-core`).
fn ensure_trailing_slash(api_base: &str) -> Result<String, CloudError> {
    let trimmed = api_base.trim();
    let scheme_end = trimmed.find("://").ok_or_else(|| {
        CloudError::BadRequest(format!("api_base '{api_base}' is not an absolute http URL"))
    })?;
    let scheme = &trimmed[..scheme_end];
    if scheme != "http" && scheme != "https" {
        return Err(CloudError::BadRequest(format!(
            "api_base '{api_base}' must use http or https"
        )));
    }
    Ok(if trimmed.ends_with('/') { trimmed.to_owned() } else { format!("{trimmed}/") })
}

#[cfg(test)]
mod tests {
    use super::*;
    use slab_config::CloudProviderConfig;

    fn provider(id: &str, family: ProviderFamily, api_base: &str) -> CloudProviderConfig {
        CloudProviderConfig {
            id: id.to_owned(),
            family,
            name: id.to_owned(),
            api_base: api_base.to_owned(),
            api_key: Some("sk-test".to_owned()),
            api_key_env: None,
        }
    }

    #[test]
    fn only_openai_compatible_supports_live_discovery() {
        assert!(supports_live_discovery(ProviderFamily::OpenaiCompatible));
        // Curated families and non-OpenAI protocols never hit the discovery path.
        assert!(!supports_live_discovery(ProviderFamily::BigModel));
        assert!(!supports_live_discovery(ProviderFamily::Openai));
        assert!(!supports_live_discovery(ProviderFamily::Ollama));
    }

    #[tokio::test]
    async fn unsupported_family_returns_empty_without_network() {
        let spec = provider("glm", ProviderFamily::BigModel, "https://example.com/v4");
        assert!(list_remote_model_ids(&spec).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn missing_api_base_returns_empty_without_network() {
        let spec = provider("custom", ProviderFamily::OpenaiCompatible, "");
        assert!(list_remote_model_ids(&spec).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn malformed_api_base_is_a_bad_request() {
        let spec = provider("custom", ProviderFamily::OpenaiCompatible, "not-a-url");
        let error = list_remote_model_ids(&spec).await.unwrap_err();
        assert!(error.is_bad_request(), "unexpected error: {error}");
    }

    #[test]
    fn trailing_slash_normalization() {
        assert_eq!(ensure_trailing_slash("https://x.test/v1").unwrap(), "https://x.test/v1/");
        assert_eq!(ensure_trailing_slash("https://x.test/v1/").unwrap(), "https://x.test/v1/");
        assert_eq!(ensure_trailing_slash(" http://x.test ").unwrap(), "http://x.test/");
        assert!(ensure_trailing_slash("ftp://x.test").is_err());
        assert!(ensure_trailing_slash("x.test/v1").is_err());
    }
}
