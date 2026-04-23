use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::validation::{ValidatedJson, validate};
use crate::error::ServerError;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::PluginService;
use slab_app_core::schemas::plugin::{
    DeletePluginResponse, InstallPluginRequest, PluginMarketResponse, PluginPath, PluginResponse,
    StopPluginRequest,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        list_plugins,
        get_plugin,
        list_market_plugins,
        install_plugin,
        enable_plugin,
        disable_plugin,
        start_plugin,
        stop_plugin,
        delete_plugin
    ),
    components(schemas(
        DeletePluginResponse,
        InstallPluginRequest,
        PluginMarketResponse,
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
        .route("/plugins/market", get(list_market_plugins))
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
    get,
    path = "/v1/plugins/market",
    tag = "plugins",
    responses((status = 200, description = "Remote market catalog", body = [PluginMarketResponse]))
)]
async fn list_market_plugins(
    State(service): State<PluginService>,
) -> Result<Json<Vec<PluginMarketResponse>>, ServerError> {
    Ok(Json(service.list_market().await?.into_iter().map(PluginMarketResponse::from).collect()))
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
    use super::PluginApi;
    use utoipa::OpenApi;

    #[test]
    fn plugin_routes_publish_install_and_market_paths_in_openapi() {
        let openapi = serde_json::to_value(PluginApi::openapi()).expect("serialize plugin openapi");
        assert!(openapi["paths"]["/v1/plugins/install"]["post"].is_object());
        assert!(openapi["paths"]["/v1/plugins/market"]["get"].is_object());
        assert!(openapi["paths"]["/v1/plugins/{id}/start"]["post"].is_object());
    }
}
