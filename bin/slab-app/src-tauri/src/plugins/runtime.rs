use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use extism::{Manifest as ExtismManifest, Plugin, PluginBuilder, UserData, Wasm};
use reqwest::Client as AsyncHttpClient;
use reqwest::blocking::Client as BlockingHttpClient;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use tauri::{AppHandle, Emitter};

use super::registry::LoadedPlugin;
use super::types::{
    PluginApiRequest, PluginApiResponse, PluginCallRequest, PluginCallResponse, PluginEmitRequest,
    PluginEventPayload, PluginNetworkMode,
};
use crate::setup::ApiEndpointConfig;

const DEFAULT_HTTP_TIMEOUT_MS: u64 = 15_000;
const MAX_HTTP_TIMEOUT_MS: u64 = 60_000;
const MAX_API_RESPONSE_BYTES: usize = 1024 * 1024;

struct RuntimeInstance {
    plugin: Plugin,
}

pub struct PluginRuntimeManager {
    instances: Mutex<HashMap<String, RuntimeInstance>>,
    blocking_http_client: BlockingHttpClient,
    api_endpoint: ApiEndpointConfig,
}

impl PluginRuntimeManager {
    pub fn new(api_endpoint: ApiEndpointConfig) -> Result<Self, String> {
        let blocking_http_client = BlockingHttpClient::builder()
            .timeout(Duration::from_millis(DEFAULT_HTTP_TIMEOUT_MS))
            .build()
            .map_err(|e| format!("failed to build blocking HTTP client: {e}"))?;

        Ok(Self { instances: Mutex::new(HashMap::new()), blocking_http_client, api_endpoint })
    }

    pub fn call_plugin(
        &self,
        app_handle: &AppHandle,
        plugin: &LoadedPlugin,
        request: &PluginCallRequest,
    ) -> Result<PluginCallResponse, String> {
        let mut guard = self
            .instances
            .lock()
            .map_err(|_| "failed to lock plugin runtime manager".to_string())?;

        if !guard.contains_key(&plugin.manifest.id) {
            let instance = RuntimeInstance {
                plugin: build_extism_plugin(
                    app_handle,
                    plugin,
                    self.blocking_http_client.clone(),
                    self.api_endpoint.clone(),
                )?,
            };
            guard.insert(plugin.manifest.id.clone(), instance);
        }

        let instance = guard
            .get_mut(&plugin.manifest.id)
            .ok_or_else(|| "failed to acquire initialized plugin runtime".to_string())?;

        let output = instance
            .plugin
            .call::<_, Vec<u8>>(request.function.as_str(), request.input.as_bytes())
            .map_err(|e| {
                format!(
                    "failed to call function `{}` on plugin `{}`: {e}",
                    request.function, request.plugin_id
                )
            })?;

        Ok(PluginCallResponse {
            output_text: String::from_utf8_lossy(&output).to_string(),
            output_base64: base64::engine::general_purpose::STANDARD.encode(output),
        })
    }
}

#[derive(Clone)]
struct HostFunctionContext {
    plugin_id: String,
    app_handle: AppHandle,
    blocking_http_client: BlockingHttpClient,
    api_endpoint: ApiEndpointConfig,
}

extism::host_fn!(slab_api_request(context: HostFunctionContext; payload: String) -> String {
    let context = context.get()?;
    let blocking_http_client = {
        let guard = context
            .lock()
            .map_err(|_| extism::Error::msg("failed to lock extism host context"))?;
        guard.blocking_http_client.clone()
    };

    let api_request: PluginApiRequest =
        serde_json::from_str(&payload).map_err(|e| extism::Error::msg(format!("invalid API payload: {e}")))?;
    let api_endpoint = {
        let guard = context
            .lock()
            .map_err(|_| extism::Error::msg("failed to lock extism host context"))?;
        guard.api_endpoint.clone()
    };
    let response = execute_plugin_api_request_blocking(&blocking_http_client, &api_endpoint, &api_request)
        .map_err(extism::Error::msg)?;
    serde_json::to_string(&response).map_err(extism::Error::from)
});

extism::host_fn!(slab_ui_emit(context: HostFunctionContext; payload: String) -> String {
    let context = context.get()?;
    let (plugin_id, app_handle) = {
        let guard = context
            .lock()
            .map_err(|_| extism::Error::msg("failed to lock extism host context"))?;
        (guard.plugin_id.clone(), guard.app_handle.clone())
    };

    let emit_request: PluginEmitRequest =
        serde_json::from_str(&payload).map_err(|e| extism::Error::msg(format!("invalid emit payload: {e}")))?;

    let event_payload = PluginEventPayload {
        plugin_id: plugin_id.clone(),
        topic: emit_request.topic,
        data: emit_request.data,
        ts: now_millis(),
    };

    app_handle
        .emit(&plugin_event_name(&plugin_id), &event_payload)
        .map_err(extism::Error::from)?;
    serde_json::to_string(&event_payload).map_err(extism::Error::from)
});

fn build_extism_plugin(
    app_handle: &AppHandle,
    plugin: &LoadedPlugin,
    blocking_http_client: BlockingHttpClient,
    api_endpoint: ApiEndpointConfig,
) -> Result<Plugin, String> {
    let wasm_entry_path = plugin
        .wasm_entry_path
        .as_ref()
        .ok_or_else(|| format!("plugin `{}` has no wasm runtime", plugin.manifest.id))?;
    let mut manifest = ExtismManifest::new([Wasm::file(wasm_entry_path.clone())]);
    manifest = manifest
        .with_allowed_path(plugin.root_dir.to_string_lossy().to_string(), plugin.root_dir.clone());

    if plugin.manifest.permissions.network.mode == PluginNetworkMode::Allowlist
        && !plugin.manifest.permissions.network.allow_hosts.is_empty()
    {
        manifest =
            manifest.with_allowed_hosts(
                plugin.manifest.permissions.network.allow_hosts.clone().into_iter(),
            );
    }

    let context = HostFunctionContext {
        plugin_id: plugin.manifest.id.clone(),
        app_handle: app_handle.clone(),
        blocking_http_client,
        api_endpoint,
    };
    let user_data = UserData::new(context);

    PluginBuilder::new(manifest)
        .with_wasi(true)
        .with_function(
            "slab.api.request",
            [extism::PTR],
            [extism::PTR],
            user_data.clone(),
            slab_api_request,
        )
        .with_function("slab.ui.emit", [extism::PTR], [extism::PTR], user_data, slab_ui_emit)
        .build()
        .map_err(|e| format!("failed to initialize extism plugin `{}`: {e}", plugin.manifest.id))
}

pub async fn execute_plugin_api_request_async(
    api_endpoint: &ApiEndpointConfig,
    request: &PluginApiRequest,
) -> Result<PluginApiResponse, String> {
    let method = reqwest::Method::from_bytes(request.method.as_bytes())
        .map_err(|e| format!("invalid HTTP method `{}`: {e}", request.method))?;
    let url = build_upstream_url(api_endpoint, &request.path)?;
    let timeout_ms = request.timeout_ms.unwrap_or(DEFAULT_HTTP_TIMEOUT_MS).min(MAX_HTTP_TIMEOUT_MS);

    let client = AsyncHttpClient::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .map_err(|e| format!("failed to initialize async HTTP client: {e}"))?;

    let headers = sanitize_request_headers(&request.headers)?;
    let mut request_builder = client.request(method, url).headers(headers);
    if let Some(body) = &request.body {
        request_builder = request_builder.body(body.clone());
    }

    let response =
        request_builder.send().await.map_err(|e| format!("failed to request local API: {e}"))?;
    response_to_plugin_api_response_async(response).await
}

fn execute_plugin_api_request_blocking(
    client: &BlockingHttpClient,
    api_endpoint: &ApiEndpointConfig,
    request: &PluginApiRequest,
) -> Result<PluginApiResponse, String> {
    let method = reqwest::Method::from_bytes(request.method.as_bytes())
        .map_err(|e| format!("invalid HTTP method `{}`: {e}", request.method))?;
    let url = build_upstream_url(api_endpoint, &request.path)?;
    let headers = sanitize_request_headers(&request.headers)?;
    let mut request_builder = client.request(method, url).headers(headers);

    if let Some(body) = &request.body {
        request_builder = request_builder.body(body.clone());
    }

    let response =
        request_builder.send().map_err(|e| format!("failed to request local API: {e}"))?;
    response_to_plugin_api_response_blocking(response)
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

        let header_name =
            HeaderName::from_str(name).map_err(|e| format!("invalid header name `{name}`: {e}"))?;
        let header_value = HeaderValue::from_str(value)
            .map_err(|e| format!("invalid header value for `{name}`: {e}"))?;
        clean.insert(header_name, header_value);
    }
    Ok(clean)
}

async fn response_to_plugin_api_response_async(
    response: reqwest::Response,
) -> Result<PluginApiResponse, String> {
    let status = response.status().as_u16();
    let headers = collect_response_headers(response.headers());
    let bytes =
        response.bytes().await.map_err(|e| format!("failed to read API response body: {e}"))?;
    if bytes.len() > MAX_API_RESPONSE_BYTES {
        return Err(format!("API response body exceeds {} bytes limit", MAX_API_RESPONSE_BYTES));
    }

    Ok(PluginApiResponse { status, headers, body: String::from_utf8_lossy(&bytes).to_string() })
}

fn response_to_plugin_api_response_blocking(
    response: reqwest::blocking::Response,
) -> Result<PluginApiResponse, String> {
    let status = response.status().as_u16();
    let headers = collect_response_headers(response.headers());
    let bytes = response.bytes().map_err(|e| format!("failed to read API response body: {e}"))?;

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

fn now_millis() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

pub fn plugin_event_name(plugin_id: &str) -> String {
    format!("plugin://{plugin_id}/event")
}
