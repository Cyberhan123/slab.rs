use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Multipart, Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router, body::Body};
use futures::StreamExt;
use utoipa::{OpenApi, ToSchema};

const MAX_PLUGIN_PACK_SIZE: usize = 1024 * 1024 * 1024; // 1GB

use crate::api::validation::{ValidatedJson, validate};
use crate::error::ServerError;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::PluginService;
use slab_app_core::schemas::plugin::{
    DeletePluginResponse, InstallPluginRequest, PluginApiRequest, PluginApiResponse, PluginPath,
    PluginResponse, StopPluginRequest,
};

use super::rpc;

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
        plugin_api_request,
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
        PluginApiRequest,
        PluginApiResponse,
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
        .route("/plugins/{id}/api-request", post(plugin_api_request))
        .route("/plugins/{id}/ui/{*asset_path}", get(plugin_ui_asset))
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

        let response = rpc::handle_payload(&service, &payload).await;
        let _ = socket.send(Message::Text(response.into())).await;
    }
}

async fn plugin_ui_asset(
    State(service): State<PluginService>,
    Path((id, asset_path)): Path<(String, String)>,
) -> Result<Response, ServerError> {
    let asset = service.plugin_ui_asset(&id, &asset_path).await?;
    let cache_control = if cfg!(debug_assertions) { "no-store" } else { "public, max-age=3600" };
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, asset.content_type)
        .header(header::CACHE_CONTROL, cache_control)
        .header("X-Content-Type-Options", "nosniff");

    if let Some(csp) = asset.csp {
        builder = builder.header("Content-Security-Policy", csp);
    }

    builder.body(Body::from(asset.bytes)).map_err(|error| {
        ServerError::Internal(format!("failed to build plugin asset response: {error}"))
    })
}

#[utoipa::path(
    post,
    path = "/v1/plugins/{id}/api-request",
    tag = "plugins",
    params(PluginPath),
    request_body = PluginApiRequest,
    responses(
        (status = 403, description = "Plugin Slab API bridge is only available through the desktop plugin WebView host")
    )
)]
async fn plugin_api_request(
    Path(params): Path<PluginPath>,
) -> Result<Json<PluginApiResponse>, ServerError> {
    let params = validate(params)?;
    Err(ServerError::Forbidden(format!(
        "plugin Slab API bridge for `{}` is only available through the desktop plugin WebView host",
        params.id
    )))
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
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode, header};
    use chrono::Utc;
    use slab_app_core::infra::db::PluginStateStore;
    use slab_app_core::infra::db::entities::PluginStateRecord;

    use super::PluginApi;
    use crate::api::test_support::{TestResponse, TestServer, response_json};
    use utoipa::OpenApi;

    #[test]
    fn plugin_routes_publish_install_paths_in_openapi() {
        let openapi = serde_json::to_value(PluginApi::openapi()).expect("serialize plugin openapi");
        assert!(openapi["paths"]["/v1/plugins/install"]["post"].is_object());
        assert!(openapi["paths"]["/v1/plugins/import-pack"]["post"].is_object());
        assert!(openapi["paths"]["/v1/plugins/{id}/api-request"]["post"].is_object());
        assert!(openapi["paths"]["/v1/plugins/{id}/start"]["post"].is_object());
    }

    fn stage_dev_plugin(plugins_dir: &std::path::Path, plugin_id: &str) {
        let plugin_root = plugins_dir.join(plugin_id);
        std::fs::create_dir_all(plugin_root.join("ui")).expect("plugin ui dir");
        std::fs::write(plugin_root.join("ui").join("index.html"), "<!doctype html>").expect("ui");
        std::fs::write(
            plugin_root.join("plugin.json"),
            serde_json::json!({
                "manifestVersion": 1,
                "id": plugin_id,
                "name": "Stage Plugin",
                "version": "0.1.0",
                "runtime": { "ui": { "entry": "ui/index.html" } },
                "permissions": { "network": { "mode": "blocked", "allowHosts": [] } }
            })
            .to_string(),
        )
        .expect("plugin manifest");
    }

    #[tokio::test]
    async fn stop_plugin_preserves_prior_failure_diagnostic() {
        let server = TestServer::new().await;
        stage_dev_plugin(&server.plugins_dir(), "stage-plugin");

        // Simulate a plugin that previously failed (e.g. failed to start) and is
        // still enabled. A user-initiated stop must not clear this diagnostic.
        let now = Utc::now();
        server
            .store
            .upsert_plugin_state(PluginStateRecord {
                plugin_id: "stage-plugin".to_owned(),
                source_kind: "dev".to_owned(),
                source_ref: None,
                install_root: Some(
                    server.plugins_dir().join("stage-plugin").to_string_lossy().into_owned(),
                ),
                installed_version: Some("0.1.0".to_owned()),
                manifest_hash: None,
                enabled: true,
                runtime_status: "error".to_owned(),
                last_error: Some("missing runtime dependency".to_owned()),
                installed_at: now,
                updated_at: now,
                last_seen_at: Some(now),
                last_started_at: None,
                last_stopped_at: None,
            })
            .await
            .expect("seed plugin state");

        // The frontend now sends an empty body (no `lastError`) on a manual stop.
        let response =
            server.post_json("/v1/plugins/stage-plugin/stop", serde_json::json!({})).await;

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.body["runtimeStatus"], "stopped");
        assert_eq!(response.body["lastError"], "missing runtime dependency");
    }

    async fn send_plugin_api_request(
        server: &TestServer,
        target: &str,
        caller_header: Option<&str>,
    ) -> TestResponse {
        let body = serde_json::json!({
            "method": "POST",
            "path": "/v1/chat/completions"
        })
        .to_string();
        let uri = format!("/v1/plugins/{target}/api-request");
        let mut builder = Request::builder()
            .method(Method::POST)
            .uri(&uri)
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(caller) = caller_header {
            builder = builder.header("x-slab-plugin-caller", caller);
        }
        let response = server.raw(builder.body(Body::from(body)).expect("test request")).await;
        response_json(response).await
    }

    #[tokio::test]
    async fn plugin_api_request_rejects_public_http_without_caller_header() {
        let server = TestServer::new().await;

        let response = send_plugin_api_request(&server, "stage-plugin", None).await;

        assert_eq!(response.status, StatusCode::FORBIDDEN);
        assert!(
            response.body["message"]
                .as_str()
                .unwrap_or_default()
                .contains("only available through the desktop plugin WebView host")
        );
    }

    #[tokio::test]
    async fn plugin_api_request_rejects_public_http_with_mismatched_caller_header() {
        let server = TestServer::new().await;

        let response = send_plugin_api_request(&server, "stage-plugin", Some("other-plugin")).await;

        assert_eq!(response.status, StatusCode::FORBIDDEN);
        assert!(
            response.body["message"]
                .as_str()
                .unwrap_or_default()
                .contains("only available through the desktop plugin WebView host")
        );
    }

    #[tokio::test]
    async fn plugin_api_request_rejects_public_http_with_matching_caller_header() {
        let server = TestServer::new().await;

        let response = send_plugin_api_request(&server, "stage-plugin", Some("stage-plugin")).await;

        assert_eq!(response.status, StatusCode::FORBIDDEN);
        assert!(
            response.body["message"]
                .as_str()
                .unwrap_or_default()
                .contains("only available through the desktop plugin WebView host")
        );
    }
}
