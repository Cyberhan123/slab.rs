use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use slab_types::{PluginApiRequest, PluginApiResponse};

const DEFAULT_HTTP_TIMEOUT_MS: u64 = 15_000;
const MAX_HTTP_TIMEOUT_MS: u64 = 60_000;
const MAX_API_RESPONSE_BYTES: usize = 1024 * 1024;

pub(super) fn authorize_slab_api_request(
    allowed: &[String],
    request: &PluginApiRequest,
) -> Result<(), String> {
    let Some(required_permission) = required_slab_api_permission(&request.method, &request.path)
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

pub(super) async fn execute_plugin_api_request(
    api_base_url: &str,
    request: &PluginApiRequest,
) -> Result<PluginApiResponse, String> {
    let timeout_ms = request.timeout_ms.unwrap_or(DEFAULT_HTTP_TIMEOUT_MS).min(MAX_HTTP_TIMEOUT_MS);
    let method = reqwest::Method::from_bytes(request.method.as_bytes())
        .map_err(|error| format!("invalid HTTP method `{}`: {error}", request.method))?;
    let url = build_upstream_url(api_base_url, &request.path)?;
    let headers = sanitize_request_headers(&request.headers)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .map_err(|error| format!("failed to create plugin API HTTP client: {error}"))?;

    let mut builder = client.request(method, url).headers(headers);
    if let Some(body) = &request.body {
        builder = builder.body(body.clone());
    }

    let response =
        builder.send().await.map_err(|error| format!("failed to request local API: {error}"))?;
    let status = response.status().as_u16();
    let headers = collect_response_headers(response.headers());
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("failed to read API response body: {error}"))?;
    if bytes.len() > MAX_API_RESPONSE_BYTES {
        return Err(format!("API response body exceeds {MAX_API_RESPONSE_BYTES} bytes limit"));
    }

    Ok(PluginApiResponse { status, headers, body: String::from_utf8_lossy(&bytes).to_string() })
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

fn build_upstream_url(base_url: &str, path: &str) -> Result<String, String> {
    if !path.starts_with('/') {
        return Err("API path must start with `/`".to_string());
    }
    if path.starts_with("//") || path.contains("://") {
        return Err("absolute URLs are not allowed".to_string());
    }
    Ok(format!("{}{}", base_url.trim_end_matches('/'), path))
}

fn sanitize_request_headers(headers: &HashMap<String, String>) -> Result<HeaderMap, String> {
    let mut clean = HeaderMap::new();
    for (name, value) in headers {
        let lower = name.to_ascii_lowercase();
        if matches!(lower.as_str(), "host" | "connection" | "content-length" | "transfer-encoding")
        {
            continue;
        }
        let name = HeaderName::from_str(name)
            .map_err(|error| format!("invalid header name `{name}`: {error}"))?;
        let value = HeaderValue::from_str(value)
            .map_err(|error| format!("invalid header value: {error}"))?;
        clean.insert(name, value);
    }
    Ok(clean)
}

fn collect_response_headers(headers: &HeaderMap) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for (name, value) in headers {
        if matches!(name.as_str(), "connection" | "transfer-encoding") {
            continue;
        }
        if let Ok(value) = value.to_str() {
            result.insert(name.to_string(), value.to_string());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use slab_types::PluginApiRequest;

    use super::authorize_slab_api_request;

    fn request(method: &str, path: &str) -> PluginApiRequest {
        PluginApiRequest {
            method: method.to_owned(),
            path: path.to_owned(),
            headers: HashMap::new(),
            body: None,
            timeout_ms: None,
        }
    }

    #[test]
    fn authorizes_matching_slab_api_permission() {
        let allowed = vec!["models:read".to_owned()];

        let result = authorize_slab_api_request(&allowed, &request("GET", "/v1/models"));

        assert!(result.is_ok());
    }

    #[test]
    fn rejects_slab_api_request_without_matching_permission() {
        let allowed = vec!["models:read".to_owned()];

        let error =
            authorize_slab_api_request(&allowed, &request("POST", "/v1/audio/transcriptions"))
                .unwrap_err();

        assert!(error.contains("audio:transcribe"));
    }
}
