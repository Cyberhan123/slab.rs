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
    authorize_api_request(api_permissions, request)?;

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

fn authorize_api_request(allowed: &[String], request: &PluginApiRequest) -> Result<(), String> {
    if allowed.is_empty() {
        return Err(format!(
            "plugin has no slab API permissions; request {} {} is not allowed",
            request.method, request.path
        ));
    }

    let Some(required_permission) =
        required_slab_api_permission(request.method.as_str(), request.path.as_str())
    else {
        return Err(format!(
            "plugin API request {} {} is not part of the allowed plugin API surface",
            request.method, request.path
        ));
    };

    if allowed.iter().any(|permission| permission == required_permission) {
        return Ok(());
    }

    Err(format!(
        "plugin API request {} {} requires permissions.slabApi `{required_permission}`",
        request.method, request.path
    ))
}

fn required_slab_api_permission(method: &str, path: &str) -> Option<&'static str> {
    let method = method.to_ascii_uppercase();
    let path = path.split('?').next().unwrap_or(path);

    match method.as_str() {
        "GET" if path_matches(path, "/v1/models") => Some("models:read"),
        "POST" if path == "/v1/models/load" => Some("models:load"),
        "POST" if path == "/v1/ffmpeg/convert" => Some("ffmpeg:convert"),
        "POST" if path == "/v1/audio/transcriptions" => Some("audio:transcribe"),
        "POST" if path == "/v1/subtitles/render" => Some("subtitle:render"),
        "POST" if path == "/v1/chat/completions" => Some("chat:complete"),
        "GET" if path_matches(path, "/v1/tasks") => Some("tasks:read"),
        "POST" if path.starts_with("/v1/tasks/") && path.ends_with("/cancel") => {
            Some("tasks:cancel")
        }
        _ => None,
    }
}

fn path_matches(path: &str, base: &str) -> bool {
    path == base || path.starts_with(&format!("{base}/"))
}

#[cfg(test)]
mod tests {
    use super::{PluginApiRequest, authorize_api_request};

    fn request(method: &str, path: &str) -> PluginApiRequest {
        PluginApiRequest {
            method: method.to_string(),
            path: path.to_string(),
            headers: Default::default(),
            body: None,
            timeout_ms: None,
        }
    }

    #[test]
    fn slab_api_permissions_use_permission_ids() {
        let req = request("POST", "/v1/chat/completions");
        assert!(authorize_api_request(&["chat:complete".to_string()], &req).is_ok());
    }

    #[test]
    fn slab_api_permissions_reject_missing_permission_id() {
        let req = request("POST", "/v1/chat/completions");
        assert!(authorize_api_request(&["models:read".to_string()], &req).is_err());
    }

    #[test]
    fn slab_api_permissions_reject_unknown_surface() {
        let req = request("DELETE", "/v1/chat/completions");
        assert!(authorize_api_request(&["chat:complete".to_string()], &req).is_err());
    }
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

fn collect_response_headers(headers: &reqwest::header::HeaderMap) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for (name, value) in headers {
        if matches!(name.as_str().to_ascii_lowercase().as_str(), "connection" | "transfer-encoding")
        {
            continue;
        }
        if let Ok(v) = value.to_str() {
            result.insert(name.to_string(), v.to_string());
        }
    }
    result
}
