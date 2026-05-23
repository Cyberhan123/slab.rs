//! Host operations exposed to JS plugins via the QuickJS bridge.
//!
//! These ops mirror the host functions available to WASM plugins:
//! - `slab_api_request`: Make HTTP requests to the slab API
//! - `slab_ui_emit`: Emit events to the host UI

use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const DEFAULT_HTTP_TIMEOUT_MS: u64 = 15_000;
const MAX_HTTP_TIMEOUT_MS: u64 = 60_000;
const MAX_API_RESPONSE_BYTES: usize = 1024 * 1024;

/// Request payload for `slab.api.request` from JS plugins.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginApiRequest {
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// Response payload returned from `slab.api.request`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginApiResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}

/// Request payload for `slab.ui.emit` from JS plugins.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginEmitRequest {
    pub topic: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

/// Response from `slab.ui.emit`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginEventPayload {
    pub plugin_id: String,
    pub topic: String,
    pub data: serde_json::Value,
    pub ts: u64,
}

/// Execute an API request on behalf of a plugin (blocking).
pub fn execute_api_request(
    base_url: &str,
    api_permissions: &[String],
    request: &PluginApiRequest,
) -> Result<PluginApiResponse, String> {
    authorize_api_request(api_permissions, &request.path)?;

    let timeout_ms = request.timeout_ms.unwrap_or(DEFAULT_HTTP_TIMEOUT_MS).min(MAX_HTTP_TIMEOUT_MS);
    let method = reqwest::Method::from_bytes(request.method.as_bytes())
        .map_err(|e| format!("invalid HTTP method `{}`: {e}", request.method))?;
    let url = build_upstream_url(base_url, &request.path)?;

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let mut req_builder = client.request(method, &url);

    for (name, value) in &request.headers {
        let lower = name.to_ascii_lowercase();
        if matches!(lower.as_str(), "host" | "connection" | "content-length" | "transfer-encoding")
        {
            continue;
        }
        req_builder = req_builder.header(name.as_str(), value.as_str());
    }

    if let Some(body) = &request.body {
        req_builder = req_builder.body(body.clone());
    }

    let response = req_builder.send().map_err(|e| format!("API request failed: {e}"))?;
    let status = response.status().as_u16();
    let resp_headers = collect_response_headers(response.headers());
    let bytes = response.bytes().map_err(|e| format!("failed to read response body: {e}"))?;

    if bytes.len() > MAX_API_RESPONSE_BYTES {
        return Err(format!("API response exceeds {MAX_API_RESPONSE_BYTES} byte limit"));
    }

    Ok(PluginApiResponse {
        status,
        headers: resp_headers,
        body: String::from_utf8_lossy(&bytes).to_string(),
    })
}

/// Create an event payload for a UI emit call.
pub fn create_event_payload(plugin_id: &str, request: &PluginEmitRequest) -> PluginEventPayload {
    PluginEventPayload {
        plugin_id: plugin_id.to_string(),
        topic: request.topic.clone(),
        data: request.data.clone(),
        ts: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
    }
}

fn authorize_api_request(allowed: &[String], path: &str) -> Result<(), String> {
    if allowed.is_empty() {
        return Err(format!(
            "plugin has no slab API permissions; request to `{path}` is not allowed"
        ));
    }
    let path = path.trim_start_matches('/');
    let authorized = allowed.iter().any(|prefix| {
        let prefix = prefix.trim_start_matches('/');
        path.starts_with(prefix)
    });
    if !authorized {
        return Err(format!(
            "request to `{path}` is not covered by plugin slab API permissions"
        ));
    }
    Ok(())
}

fn build_upstream_url(base_url: &str, path: &str) -> Result<String, String> {
    if !path.starts_with('/') {
        return Err("API path must start with `/`".to_string());
    }
    if path.starts_with("//") || path.contains("://") {
        return Err("absolute URLs are not allowed".to_string());
    }
    Ok(format!("{}{}", base_url.trim_end_matches('/'), path))
}

fn collect_response_headers(
    headers: &reqwest::header::HeaderMap,
) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for (name, value) in headers {
        if matches!(
            name.as_str().to_ascii_lowercase().as_str(),
            "connection" | "transfer-encoding"
        ) {
            continue;
        }
        if let Ok(v) = value.to_str() {
            result.insert(name.to_string(), v.to_string());
        }
    }
    result
}
