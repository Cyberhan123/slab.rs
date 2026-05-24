use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Runtime};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

use super::types::{PluginCallRequest, PluginCallResponse, PluginEventPayload};
use crate::setup::ApiEndpointConfig;

pub struct PluginRpcWsClient {
    endpoint: String,
    connection: Mutex<Option<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>>>,
    request_id: AtomicU64,
}

pub fn spawn_plugin_event_listener<R: Runtime>(
    app_handle: AppHandle<R>,
    api_endpoint: ApiEndpointConfig,
) {
    let endpoint = format!("{}/v1/plugins/events", to_ws_origin(&api_endpoint.api_origin));
    tauri::async_runtime::spawn(async move {
        loop {
            match connect_async(&endpoint).await {
                Ok((mut socket, _)) => {
                    while let Some(frame) = socket.next().await {
                        let Ok(frame) = frame else {
                            break;
                        };
                        let Message::Text(text) = frame else {
                            continue;
                        };
                        match serde_json::from_str::<PluginEventPayload>(&text) {
                            Ok(payload) => {
                                let event_name = format!("plugin://{}/event", payload.plugin_id);
                                if let Err(error) = app_handle.emit(&event_name, payload) {
                                    log::warn!("failed to emit plugin event to UI: {error}");
                                }
                            }
                            Err(error) => {
                                log::warn!("failed to parse plugin event payload: {error}");
                            }
                        }
                    }
                }
                Err(error) => {
                    log::debug!("plugin event websocket unavailable: {error}");
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });
}

impl PluginRpcWsClient {
    pub fn new(api_endpoint: ApiEndpointConfig) -> Self {
        let endpoint = format!("{}/v1/plugins/rpc", to_ws_origin(&api_endpoint.api_origin));
        Self { endpoint, connection: Mutex::new(None), request_id: AtomicU64::new(1) }
    }

    pub async fn call(&self, request: &PluginCallRequest) -> Result<PluginCallResponse, String> {
        let params = if request.input.trim().is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&request.input)
                .map_err(|error| format!("failed to parse plugin call input as JSON: {error}"))?
        };

        let method = format!("{}.{}", request.plugin_id, request.function);
        let id = self.request_id.fetch_add(1, Ordering::Relaxed);
        let rpc_request = RpcRequest { jsonrpc: "2.0", id: Value::from(id), method, params };

        let payload = serde_json::to_string(&rpc_request)
            .map_err(|error| format!("failed to serialize plugin RPC request: {error}"))?;

        let mut connection = self.connection.lock().await;
        let socket = if let Some(socket) = connection.as_mut() {
            socket
        } else {
            let (socket, _) = connect_async(&self.endpoint)
                .await
                .map_err(|error| format!("failed to connect plugin RPC websocket: {error}"))?;
            connection.insert(socket)
        };

        socket
            .send(Message::Text(payload.into()))
            .await
            .map_err(|error| format!("failed to send plugin RPC frame: {error}"))?;

        while let Some(frame) = socket.next().await {
            let frame =
                frame.map_err(|error| format!("failed to read plugin RPC frame: {error}"))?;
            let Message::Text(text) = frame else {
                continue;
            };

            let response: RpcResponse = serde_json::from_str(&text)
                .map_err(|error| format!("failed to parse plugin RPC response: {error}"))?;
            if response.id.as_u64() != Some(id) {
                continue;
            }

            if let Some(error) = response.error {
                return Err(format!("plugin RPC error {}: {}", error.code, error.message));
            }

            let result = response.result.unwrap_or(Value::Null);
            let output_bytes = serde_json::to_vec(&result)
                .map_err(|error| format!("failed to serialize plugin RPC result: {error}"))?;
            return Ok(PluginCallResponse {
                output_text: String::from_utf8_lossy(&output_bytes).to_string(),
                output_base64: base64::engine::general_purpose::STANDARD.encode(output_bytes),
            });
        }

        *connection = None;
        Err("plugin RPC websocket closed unexpectedly".to_string())
    }
}

#[derive(Debug, Serialize)]
struct RpcRequest {
    jsonrpc: &'static str,
    id: Value,
    method: String,
    params: Value,
}

#[derive(Debug, Deserialize)]
struct RpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Value,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

fn to_ws_origin(origin: &str) -> String {
    if origin.starts_with("https://") {
        return origin.replacen("https://", "wss://", 1);
    }
    origin.replacen("http://", "ws://", 1)
}

#[cfg(test)]
mod tests {
    use super::to_ws_origin;

    #[test]
    fn converts_http_and_https_origins_to_websocket() {
        assert_eq!(to_ws_origin("http://127.0.0.1:11435"), "ws://127.0.0.1:11435");
        assert_eq!(to_ws_origin("https://api.slab.local"), "wss://api.slab.local");
    }
}
