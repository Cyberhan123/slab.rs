use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use base64::Engine;
use extism::{Manifest as ExtismManifest, Plugin, PluginBuilder, UserData, Wasm};
use reqwest::blocking::Client as BlockingHttpClient;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use crate::error::PluginError;
use crate::runtime::PluginBackend;
use crate::types::{
    LoadedPlugin, PluginApiRequest, PluginApiResponse, PluginCallRequest, PluginCallResponse,
    PluginEmitRequest, PluginEventPayload, PluginNetworkMode,
};

const DEFAULT_HTTP_TIMEOUT_MS: u64 = 15_000;
const MAX_HTTP_TIMEOUT_MS: u64 = 60_000;
const MAX_API_RESPONSE_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
struct WasmHostContext {
    plugin_id: String,
    blocking_http_client: BlockingHttpClient,
    api_base_url: Option<String>,
    slab_api_permissions: Vec<String>,
}

extism::host_fn!(slab_api_request(context: WasmHostContext; payload: String) -> String {
    let context = context.get()?;
    let (blocking_http_client, api_base_url, slab_api_permissions) = {
        let guard = context
            .lock()
            .map_err(|_| extism::Error::msg("failed to lock wasm host context"))?;
        (
            guard.blocking_http_client.clone(),
            guard.api_base_url.clone(),
            guard.slab_api_permissions.clone(),
        )
    };

    let base_url = api_base_url.ok_or_else(|| {
        extism::Error::msg("slab.api.request is not available: no API base URL configured")
    })?;

    let api_request: PluginApiRequest = serde_json::from_str(&payload)
        .map_err(|e| extism::Error::msg(format!("invalid API payload: {e}")))?;

    authorize_slab_api_request(&slab_api_permissions, &api_request)
        .map_err(extism::Error::msg)?;

    let response = execute_plugin_api_request_blocking(&blocking_http_client, &base_url, &api_request)
        .map_err(extism::Error::msg)?;

    serde_json::to_string(&response).map_err(extism::Error::from)
});

extism::host_fn!(slab_ui_emit(context: WasmHostContext; payload: String) -> String {
    let context = context.get()?;
    let plugin_id = {
        let guard = context
            .lock()
            .map_err(|_| extism::Error::msg("failed to lock wasm host context"))?;
        guard.plugin_id.clone()
    };

    let emit_request: PluginEmitRequest = serde_json::from_str(&payload)
        .map_err(|e| extism::Error::msg(format!("invalid emit payload: {e}")))?;

    let event_payload = PluginEventPayload {
        plugin_id: plugin_id.clone(),
        topic: emit_request.topic,
        data: emit_request.data,
        ts: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
    };

    // In the server-side context there is no Tauri AppHandle, so the event
    // is not forwarded to any UI. Returning a serialized payload keeps the
    // WASM plugin from failing on this host function call.
    serde_json::to_string(&event_payload).map_err(extism::Error::from)
});

struct RuntimeInstance {
    plugin: Plugin,
}

pub struct WasmPluginBackend {
    instances: Mutex<HashMap<String, RuntimeInstance>>,
    blocking_http_client: BlockingHttpClient,
    api_base_url: Option<String>,
}

impl WasmPluginBackend {
    pub fn new() -> Self {
        let blocking_http_client = BlockingHttpClient::builder()
            .timeout(Duration::from_millis(DEFAULT_HTTP_TIMEOUT_MS))
            .build()
            .unwrap_or_default();
        Self { instances: Mutex::new(HashMap::new()), blocking_http_client, api_base_url: None }
    }

    pub fn with_api_base_url(mut self, url: String) -> Self {
        self.api_base_url = Some(url);
        self
    }
}

impl Default for WasmPluginBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PluginBackend for WasmPluginBackend {
    fn accepts(&self, plugin: &LoadedPlugin) -> bool {
        plugin.wasm_entry_path.is_some()
    }

    async fn call(
        &self,
        plugin: &LoadedPlugin,
        request: &PluginCallRequest,
    ) -> Result<PluginCallResponse, PluginError> {
        let mut guard = self
            .instances
            .lock()
            .map_err(|_| PluginError::Runtime("failed to lock wasm runtime manager".to_string()))?;

        if !guard.contains_key(&plugin.manifest.id) {
            let runtime = RuntimeInstance {
                plugin: build_extism_plugin(
                    plugin,
                    self.blocking_http_client.clone(),
                    self.api_base_url.clone(),
                )?,
            };
            guard.insert(plugin.manifest.id.clone(), runtime);
        }

        let runtime = guard.get_mut(&plugin.manifest.id).ok_or_else(|| {
            PluginError::Runtime("failed to acquire initialized wasm runtime".to_string())
        })?;

        let output = runtime
            .plugin
            .call::<_, Vec<u8>>(request.function.as_str(), request.input.as_bytes())
            .map_err(|error| {
                PluginError::Runtime(format!(
                    "failed to call function `{}` on plugin `{}`: {error}",
                    request.function, request.plugin_id
                ))
            })?;

        Ok(PluginCallResponse {
            output_text: String::from_utf8_lossy(&output).to_string(),
            output_base64: base64::engine::general_purpose::STANDARD.encode(output),
        })
    }
}

fn build_extism_plugin(
    plugin: &LoadedPlugin,
    blocking_http_client: BlockingHttpClient,
    api_base_url: Option<String>,
) -> Result<Plugin, PluginError> {
    let wasm_entry_path = plugin.wasm_entry_path.as_ref().ok_or_else(|| {
        PluginError::Runtime(format!("plugin `{}` has no wasm runtime", plugin.manifest.id))
    })?;

    let mut manifest = ExtismManifest::new([Wasm::file(wasm_entry_path.clone())]);
    manifest = manifest
        .with_allowed_path(plugin.root_dir.to_string_lossy().to_string(), plugin.root_dir.clone());

    if plugin.manifest.permissions.network.mode == PluginNetworkMode::Allowlist {
        if plugin.manifest.permissions.network.allow_hosts.is_empty() {
            return Err(PluginError::Runtime(format!(
                "plugin `{}` uses allowlist network mode but no allowHosts are configured",
                plugin.manifest.id
            )));
        }
        manifest = manifest.with_allowed_hosts(
            plugin.manifest.permissions.network.allow_hosts.clone().into_iter(),
        );
    }

    let context = WasmHostContext {
        plugin_id: plugin.manifest.id.clone(),
        blocking_http_client,
        api_base_url,
        slab_api_permissions: plugin.manifest.permissions.slab_api.clone(),
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
        .map_err(|error| {
            PluginError::Runtime(format!(
                "failed to initialize extism plugin `{}`: {error}",
                plugin.manifest.id
            ))
        })
}

fn authorize_slab_api_request(
    allowed: &[String],
    request: &PluginApiRequest,
) -> Result<(), String> {
    if allowed.is_empty() {
        return Err(format!(
            "plugin has no slab API permissions; request to `{}` is not allowed",
            request.path
        ));
    }
    let path = request.path.trim_start_matches('/');
    let authorized = allowed.iter().any(|prefix| {
        let prefix = prefix.trim_start_matches('/');
        path.starts_with(prefix)
    });
    if !authorized {
        return Err(format!(
            "request to `{}` is not covered by plugin slab API permissions",
            request.path
        ));
    }
    Ok(())
}

fn execute_plugin_api_request_blocking(
    client: &BlockingHttpClient,
    base_url: &str,
    request: &PluginApiRequest,
) -> Result<PluginApiResponse, String> {
    let timeout_ms = request.timeout_ms.unwrap_or(DEFAULT_HTTP_TIMEOUT_MS).min(MAX_HTTP_TIMEOUT_MS);
    let method = reqwest::Method::from_bytes(request.method.as_bytes())
        .map_err(|e| format!("invalid HTTP method `{}`: {e}", request.method))?;
    let url = build_upstream_url(base_url, &request.path)?;
    let headers = sanitize_request_headers(&request.headers)?;
    let mut request_builder =
        client.request(method, url).headers(headers).timeout(Duration::from_millis(timeout_ms));
    if let Some(body) = &request.body {
        request_builder = request_builder.body(body.clone());
    }
    let response =
        request_builder.send().map_err(|e| format!("failed to request local API: {e}"))?;
    let status = response.status().as_u16();
    let resp_headers = collect_response_headers(response.headers());
    let bytes = response.bytes().map_err(|e| format!("failed to read API response body: {e}"))?;
    if bytes.len() > MAX_API_RESPONSE_BYTES {
        return Err(format!("API response body exceeds {MAX_API_RESPONSE_BYTES} bytes limit"));
    }
    Ok(PluginApiResponse {
        status,
        headers: resp_headers,
        body: String::from_utf8_lossy(&bytes).to_string(),
    })
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
        let header_name =
            HeaderName::from_str(name).map_err(|e| format!("invalid header name `{name}`: {e}"))?;
        let header_value = HeaderValue::from_str(value)
            .map_err(|e| format!("invalid header value for `{name}`: {e}"))?;
        clean.insert(header_name, header_value);
    }
    Ok(clean)
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
