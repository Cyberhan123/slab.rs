use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

use reqwest::Client as AsyncHttpClient;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use super::types::{PluginApiRequest, PluginApiResponse};
use crate::setup::ApiEndpointConfig;

const DEFAULT_HTTP_TIMEOUT_MS: u64 = 15_000;
const MAX_HTTP_TIMEOUT_MS: u64 = 60_000;
const MAX_API_RESPONSE_BYTES: usize = 1024 * 1024;

pub async fn execute_plugin_api_request_async(
    api_endpoint: &ApiEndpointConfig,
    request: &PluginApiRequest,
) -> Result<PluginApiResponse, String> {
    let method = reqwest::Method::from_bytes(request.method.as_bytes())
        .map_err(|error| format!("invalid HTTP method `{}`: {error}", request.method))?;
    let url = build_upstream_url(api_endpoint, &request.path)?;
    let timeout_ms = request.timeout_ms.unwrap_or(DEFAULT_HTTP_TIMEOUT_MS).min(MAX_HTTP_TIMEOUT_MS);

    let client = AsyncHttpClient::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .map_err(|error| format!("failed to initialize async HTTP client: {error}"))?;

    let headers = sanitize_request_headers(&request.headers)?;
    let mut request_builder = client.request(method, url).headers(headers);
    if let Some(body) = &request.body {
        request_builder = request_builder.body(body.clone());
    }

    let response = request_builder
        .send()
        .await
        .map_err(|error| format!("failed to request local API: {error}"))?;
    response_to_plugin_api_response_async(response).await
}

fn build_upstream_url(api_endpoint: &ApiEndpointConfig, path: &str) -> Result<String, String> {
    if !path.starts_with('/') {
        return Err("API path must start with `/`".to_string());
    }

    if path.starts_with("//") || path.contains("://") {
        return Err("absolute URLs are not allowed".to_string());
    }

    Ok(format!("{}{}", api_endpoint.api_base_url().trim_end_matches('/'), path))
}

fn sanitize_request_headers(headers: &HashMap<String, String>) -> Result<HeaderMap, String> {
    let mut clean = HeaderMap::new();
    for (name, value) in headers {
        let lower = name.to_ascii_lowercase();
        if matches!(lower.as_str(), "host" | "connection" | "content-length" | "transfer-encoding")
        {
            continue;
        }

        let header_name = HeaderName::from_str(name)
            .map_err(|error| format!("invalid header name `{name}`: {error}"))?;
        let header_value = HeaderValue::from_str(value)
            .map_err(|error| format!("invalid header value for `{name}`: {error}"))?;
        clean.insert(header_name, header_value);
    }
    Ok(clean)
}

async fn response_to_plugin_api_response_async(
    response: reqwest::Response,
) -> Result<PluginApiResponse, String> {
    let status = response.status().as_u16();
    let headers = collect_response_headers(response.headers());
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("failed to read API response body: {error}"))?;
    if bytes.len() > MAX_API_RESPONSE_BYTES {
        return Err(format!("API response body exceeds {} bytes limit", MAX_API_RESPONSE_BYTES));
    }

    Ok(PluginApiResponse { status, headers, body: String::from_utf8_lossy(&bytes).to_string() })
}

fn collect_response_headers(headers: &HeaderMap) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for (name, value) in headers {
        if matches!(name.as_str().to_ascii_lowercase().as_str(), "connection" | "transfer-encoding")
        {
            continue;
        }

        if let Ok(value) = value.to_str() {
            result.insert(name.to_string(), value.to_string());
        }
    }
    result
}
