//! Responses API capability probe for custom OpenAI-compatible endpoints.
//!
//! Third-party endpoints increasingly expose the OpenAI Responses API
//! (`POST {api_base}/responses`) alongside Chat Completions, but support is
//! uneven — the probe's answer routes chat traffic (see
//! [`crate::adapter_kind::resolve_adapter_kind`]) and must never break the
//! request path: any transport failure, timeout, or malformed `api_base`
//! resolves to "unsupported", so the caller silently stays on
//! chat/completions. Callers own caching (success TTL / failure backoff) and
//! timeouts, mirroring the live-discovery contract in [`crate::discovery`].

use std::time::Duration;

use reqwest::StatusCode;
use serde_json::json;
use slab_config::CloudProviderConfig;
use tracing::debug;

use crate::error::CloudError;

/// Probe payload model when no remote model id is known at probe time. Route
/// existence — not request validity — is what the probe measures, so a 4xx
/// rejection of this model still counts as "supported".
const PROBE_FALLBACK_MODEL: &str = "gpt-4.1-mini";

/// Whether an HTTP status from `POST {api_base}/responses` proves the route
/// exists. 404/405 mean the route is absent; every other status (including
/// auth/validity/server errors) means the route is there — the probe's minimal
/// payload is expected to be rejected on route-bearing endpoints that don't
/// know the model. Unknown-model 404s are misread as "unsupported" in the
/// worst case, which only keeps traffic on chat/completions — the safe
/// direction.
pub fn responses_probe_status_to_supports(status: StatusCode) -> bool {
    !matches!(status.as_u16(), 404 | 405)
}

/// Probe whether the provider's endpoint exposes the Responses API. Returns
/// `false` (never an error) when the endpoint cannot be reached, the request
/// times out, or `api_base` is not an absolute http(s) URL.
pub async fn probe_responses_support(
    provider: &CloudProviderConfig,
    model: Option<&str>,
    timeout: Duration,
) -> bool {
    probe_responses_support_raw(
        &provider.api_base,
        crate::provider::resolve_api_key(provider).ok().as_deref(),
        model,
        timeout,
    )
    .await
}

/// Transport-level probe beneath [`probe_responses_support`] (also the
/// no-network test entry point for invalid bases).
async fn probe_responses_support_raw(
    api_base: &str,
    api_key: Option<&str>,
    model: Option<&str>,
    timeout: Duration,
) -> bool {
    let base = match ensure_trailing_slash(api_base) {
        Ok(base) => base,
        Err(error) => {
            debug!(%error, "responses probe skipped: invalid api_base");
            return false;
        }
    };
    let client = match reqwest::Client::builder().timeout(timeout).build() {
        Ok(client) => client,
        Err(error) => {
            debug!(%error, "responses probe skipped: client build failed");
            return false;
        }
    };
    let body = json!({
        "model": model.unwrap_or(PROBE_FALLBACK_MODEL),
        "input": "ping",
    });
    let mut request = client
        .post(format!("{base}responses"))
        .header("content-type", "application/json")
        .json(&body);
    if let Some(api_key) = api_key {
        request = request.bearer_auth(api_key);
    }
    match request.send().await {
        Ok(response) => {
            let supports = responses_probe_status_to_supports(response.status());
            debug!(status = response.status().as_u16(), supports, "responses probe completed");
            supports
        }
        Err(error) => {
            debug!(%error, "responses probe failed: treating endpoint as unsupported");
            false
        }
    }
}

/// Same normalization rule as [`crate::discovery`] (kept private there): the
/// probe URL is `{api_base}/responses`, so the base must be absolute http(s)
/// with a trailing slash.
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

    #[test]
    fn route_absence_means_unsupported() {
        assert!(!responses_probe_status_to_supports(StatusCode::NOT_FOUND));
        assert!(!responses_probe_status_to_supports(StatusCode::METHOD_NOT_ALLOWED));
    }

    #[test]
    fn route_presence_is_supported_even_when_the_request_is_rejected() {
        // Minimal probe payloads are expected to be rejected on route-bearing
        // endpoints (unknown model, missing scopes, rate limits, upstream
        // errors) — the route still exists.
        assert!(responses_probe_status_to_supports(StatusCode::OK));
        assert!(responses_probe_status_to_supports(StatusCode::BAD_REQUEST));
        assert!(responses_probe_status_to_supports(StatusCode::UNAUTHORIZED));
        assert!(responses_probe_status_to_supports(StatusCode::UNPROCESSABLE_ENTITY));
        assert!(responses_probe_status_to_supports(StatusCode::TOO_MANY_REQUESTS));
        assert!(responses_probe_status_to_supports(StatusCode::BAD_GATEWAY));
    }

    #[tokio::test]
    async fn invalid_api_base_is_unsupported_without_network() {
        assert!(
            !probe_responses_support_raw("not-a-url", None, None, Duration::from_secs(1)).await
        );
        assert!(
            !probe_responses_support_raw("ftp://x.test", None, None, Duration::from_secs(1)).await
        );
    }
}
