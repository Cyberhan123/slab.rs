use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Multipart, Path, State};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

const MAX_PLUGIN_PACK_SIZE: usize = 1024 * 1024 * 1024; // 1GB

use crate::api::validation::{ValidatedJson, validate};
use crate::error::ServerError;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::PluginService;
use slab_app_core::schemas::plugin::{
    DeletePluginResponse, InstallPluginRequest, PluginPath, PluginResponse, StopPluginRequest,
};

#[allow(dead_code)]
#[derive(ToSchema)]
struct ImportPluginPackMultipartRequest {
    #[schema(value_type = String, format = Binary)]
    file: Vec<u8>,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        list_plugins,
        get_plugin,
        install_plugin,
        import_plugin_pack,
        plugin_rpc,
        plugin_events,
        enable_plugin,
        disable_plugin,
        start_plugin,
        stop_plugin,
        delete_plugin
    ),
    components(schemas(
        DeletePluginResponse,
        ImportPluginPackMultipartRequest,
        InstallPluginRequest,
        PluginPath,
        PluginResponse,
        StopPluginRequest
    ))
)]
pub struct PluginApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/plugins", get(list_plugins))
        .route("/plugins/install", post(install_plugin))
        .route("/plugins/import-pack", post(import_plugin_pack))
        .route("/plugins/rpc", get(plugin_rpc))
        .route("/plugins/events", get(plugin_events))
        .route("/plugins/{id}", get(get_plugin).delete(delete_plugin))
        .route("/plugins/{id}/enable", post(enable_plugin))
        .route("/plugins/{id}/disable", post(disable_plugin))
        .route("/plugins/{id}/start", post(start_plugin))
        .route("/plugins/{id}/stop", post(stop_plugin))
}

#[utoipa::path(
    get,
    path = "/v1/plugins",
    tag = "plugins",
    responses((status = 200, description = "List discovered plugins", body = [PluginResponse]))
)]
async fn list_plugins(
    State(service): State<PluginService>,
) -> Result<Json<Vec<PluginResponse>>, ServerError> {
    Ok(Json(service.list_plugins().await?.into_iter().map(PluginResponse::from).collect()))
}

#[utoipa::path(
    get,
    path = "/v1/plugins/{id}",
    tag = "plugins",
    params(PluginPath),
    responses((status = 200, description = "Plugin detail", body = PluginResponse))
)]
async fn get_plugin(
    State(service): State<PluginService>,
    Path(params): Path<PluginPath>,
) -> Result<Json<PluginResponse>, ServerError> {
    let params = validate(params)?;
    Ok(Json(service.get_plugin(&params.id).await?.into()))
}

#[utoipa::path(
    post,
    path = "/v1/plugins/install",
    tag = "plugins",
    request_body = InstallPluginRequest,
    responses((status = 200, description = "Installed plugin", body = PluginResponse))
)]
async fn install_plugin(
    State(service): State<PluginService>,
    ValidatedJson(body): ValidatedJson<InstallPluginRequest>,
) -> Result<Json<PluginResponse>, ServerError> {
    Ok(Json(service.install_plugin(body.into()).await?.into()))
}

#[utoipa::path(
    get,
    path = "/v1/plugins/rpc",
    tag = "plugins",
    responses((status = 101, description = "WebSocket upgrade for JSON-RPC 2.0 plugin dispatch"))
)]
async fn plugin_rpc(
    ws: WebSocketUpgrade,
    State(service): State<PluginService>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| plugin_rpc_socket(socket, service))
}

#[utoipa::path(
    get,
    path = "/v1/plugins/events",
    tag = "plugins",
    responses((status = 101, description = "WebSocket upgrade for plugin UI events"))
)]
async fn plugin_events(
    ws: WebSocketUpgrade,
    State(service): State<PluginService>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| plugin_events_socket(socket, service))
}

async fn plugin_events_socket(mut socket: WebSocket, service: PluginService) {
    let mut events = service.subscribe_events();
    loop {
        match events.recv().await {
            Ok(event) => {
                let Ok(payload) = serde_json::to_string(&event) else {
                    continue;
                };
                if socket.send(Message::Text(payload.into())).await.is_err() {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                tracing::warn!(skipped, "plugin event subscriber lagged");
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
}

async fn plugin_rpc_socket(mut socket: WebSocket, service: PluginService) {
    while let Some(message) = socket.next().await {
        let Ok(message) = message else {
            break;
        };

        let Message::Text(payload) = message else {
            continue;
        };

        let response = handle_rpc_payload(&service, &payload).await;
        let _ = socket.send(Message::Text(response.into())).await;
    }
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: serde_json::Value,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

async fn handle_rpc_payload(service: &PluginService, payload: &str) -> String {
    let request: JsonRpcRequest = match serde_json::from_str(payload) {
        Ok(request) => request,
        Err(error) => {
            return serialize_rpc_response(JsonRpcResponse {
                jsonrpc: "2.0",
                id: serde_json::Value::Null,
                result: None,
                error: Some(JsonRpcError {
                    code: -32700,
                    message: format!("invalid json-rpc payload: {error}"),
                }),
            });
        }
    };

    if request.jsonrpc != "2.0" {
        return serialize_rpc_response(JsonRpcResponse {
            jsonrpc: "2.0",
            id: request.id,
            result: None,
            error: Some(JsonRpcError {
                code: -32600,
                message: "jsonrpc must be `2.0`".to_string(),
            }),
        });
    }

    let Some((plugin_id, function_name)) = parse_rpc_method(&request.method) else {
        return serialize_rpc_response(JsonRpcResponse {
            jsonrpc: "2.0",
            id: request.id,
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: "method must use `plugin_id.function_name`".to_string(),
            }),
        });
    };

    match service.dispatch_rpc(plugin_id, function_name, request.params).await {
        Ok(result) => serialize_rpc_response(JsonRpcResponse {
            jsonrpc: "2.0",
            id: request.id,
            result: Some(result),
            error: None,
        }),
        Err(error) => serialize_rpc_response(JsonRpcResponse {
            jsonrpc: "2.0",
            id: request.id,
            result: None,
            error: Some(JsonRpcError { code: -32000, message: error.to_string() }),
        }),
    }
}

fn serialize_rpc_response(response: JsonRpcResponse) -> String {
    serde_json::to_string(&response).unwrap_or_else(|error| {
        format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{{\"code\":-32603,\"message\":\"failed to serialize response: {error}\"}}}}"
        )
    })
}

fn parse_rpc_method(method: &str) -> Option<(&str, &str)> {
    let (plugin_id, function_name) = method.split_once('.')?;
    if plugin_id.trim().is_empty() || function_name.trim().is_empty() {
        return None;
    }
    Some((plugin_id, function_name))
}

#[utoipa::path(
    post,
    path = "/v1/plugins/import-pack",
    tag = "plugins",
    request_body(
        content = ImportPluginPackMultipartRequest,
        content_type = "multipart/form-data",
        description = "Upload a .plugin.slab plugin pack as a multipart file field named `file`."
    ),
    responses((status = 200, description = "Imported plugin", body = PluginResponse))
)]
async fn import_plugin_pack(
    State(service): State<PluginService>,
    mut multipart: Multipart,
) -> Result<Json<PluginResponse>, ServerError> {
    while let Some(field) = multipart.next_field().await.map_err(|error| {
        ServerError::BadRequest(format!("failed to read multipart field: {error}"))
    })? {
        let file_name = field.file_name().map(str::to_owned);
        if file_name.is_none() {
            continue;
        }

        if let Some(file_name) = file_name.as_deref()
            && !file_name.trim().to_ascii_lowercase().ends_with(".plugin.slab")
        {
            return Err(ServerError::BadRequest(format!(
                "uploaded plugin pack must use the .plugin.slab extension: {file_name}"
            )));
        }

        let bytes = field.bytes().await.map_err(|error| {
            ServerError::BadRequest(format!("failed to read plugin pack bytes: {error}"))
        })?;

        if bytes.is_empty() {
            return Err(ServerError::BadRequest("uploaded plugin pack is empty".into()));
        }

        if bytes.len() > MAX_PLUGIN_PACK_SIZE {
            return Err(ServerError::BadRequest(format!(
                "uploaded plugin pack is too large ({} bytes); maximum size is {} bytes (1GB)",
                bytes.len(),
                MAX_PLUGIN_PACK_SIZE
            )));
        }

        return Ok(Json(
            service.import_plugin_pack_bytes(bytes.as_ref(), file_name.as_deref()).await?.into(),
        ));
    }

    Err(ServerError::BadRequest("multipart body must contain a .plugin.slab file field".into()))
}

#[utoipa::path(
    post,
    path = "/v1/plugins/{id}/enable",
    tag = "plugins",
    params(PluginPath),
    responses((status = 200, description = "Enabled plugin", body = PluginResponse))
)]
async fn enable_plugin(
    State(service): State<PluginService>,
    Path(params): Path<PluginPath>,
) -> Result<Json<PluginResponse>, ServerError> {
    let params = validate(params)?;
    Ok(Json(service.enable_plugin(&params.id).await?.into()))
}

#[utoipa::path(
    post,
    path = "/v1/plugins/{id}/disable",
    tag = "plugins",
    params(PluginPath),
    responses((status = 200, description = "Disabled plugin", body = PluginResponse))
)]
async fn disable_plugin(
    State(service): State<PluginService>,
    Path(params): Path<PluginPath>,
) -> Result<Json<PluginResponse>, ServerError> {
    let params = validate(params)?;
    Ok(Json(service.disable_plugin(&params.id).await?.into()))
}

#[utoipa::path(
    post,
    path = "/v1/plugins/{id}/start",
    tag = "plugins",
    params(PluginPath),
    responses((status = 200, description = "Marked plugin as running", body = PluginResponse))
)]
async fn start_plugin(
    State(service): State<PluginService>,
    Path(params): Path<PluginPath>,
) -> Result<Json<PluginResponse>, ServerError> {
    let params = validate(params)?;
    Ok(Json(service.start_plugin(&params.id).await?.into()))
}

#[utoipa::path(
    post,
    path = "/v1/plugins/{id}/stop",
    tag = "plugins",
    params(PluginPath),
    request_body = StopPluginRequest,
    responses((status = 200, description = "Marked plugin as stopped", body = PluginResponse))
)]
async fn stop_plugin(
    State(service): State<PluginService>,
    Path(params): Path<PluginPath>,
    ValidatedJson(body): ValidatedJson<StopPluginRequest>,
) -> Result<Json<PluginResponse>, ServerError> {
    let params = validate(params)?;
    Ok(Json(service.stop_plugin(&params.id, body.last_error).await?.into()))
}

#[utoipa::path(
    delete,
    path = "/v1/plugins/{id}",
    tag = "plugins",
    params(PluginPath),
    responses((status = 200, description = "Deleted plugin", body = DeletePluginResponse))
)]
async fn delete_plugin(
    State(service): State<PluginService>,
    Path(params): Path<PluginPath>,
) -> Result<Json<DeletePluginResponse>, ServerError> {
    let params = validate(params)?;
    service.remove_plugin(&params.id).await?;
    Ok(Json(DeletePluginResponse { id: params.id, deleted: true }))
}

#[cfg(test)]
mod tests {
    use super::{PluginApi, parse_rpc_method};
    use utoipa::OpenApi;

    #[test]
    fn plugin_routes_publish_install_paths_in_openapi() {
        let openapi = serde_json::to_value(PluginApi::openapi()).expect("serialize plugin openapi");
        assert!(openapi["paths"]["/v1/plugins/install"]["post"].is_object());
        assert!(openapi["paths"]["/v1/plugins/import-pack"]["post"].is_object());
        assert!(openapi["paths"]["/v1/plugins/{id}/start"]["post"].is_object());
    }

    #[test]
    fn parses_plugin_rpc_method_shape() {
        assert_eq!(parse_rpc_method("plugin-a.run"), Some(("plugin-a", "run")));
        assert_eq!(parse_rpc_method("plugin-a."), None);
        assert_eq!(parse_rpc_method(".run"), None);
        assert_eq!(parse_rpc_method("plugin-a"), None);
    }
}
