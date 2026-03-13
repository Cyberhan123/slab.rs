use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::v1::config::schema::{ConfigEntry, SetConfigBody};
use crate::context::{AppState, ModelState};
use crate::error::ServerError;
use crate::services::config::ConfigService;

#[derive(OpenApi)]
#[openapi(
    paths(list_config, get_config_value, set_config_value),
    components(schemas(ConfigEntry, SetConfigBody))
)]
pub struct ConfigApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/config", get(list_config))
        .route("/config/{key}", get(get_config_value).put(set_config_value))
}

#[utoipa::path(
    get,
    path = "/v1/config",
    tag = "config",
    responses(
        (status = 200, description = "List of all configuration entries", body = Vec<ConfigEntry>),
        (status = 401, description = "Unauthorised (management token required)"),
    )
)]
async fn list_config(
    State(state): State<ModelState>,
) -> Result<Json<Vec<ConfigEntry>>, ServerError> {
    let service = ConfigService::new(state);
    Ok(Json(service.list_config().await?))
}

#[utoipa::path(
    get,
    path = "/v1/config/{key}",
    tag = "config",
    responses(
        (status = 200, description = "Get a configuration entry by key", body = ConfigEntry),
        (status = 401, description = "Unauthorised (management token required)"),
        (status = 404, description = "Config key not found"),
    )
)]
async fn get_config_value(
    State(state): State<ModelState>,
    Path(key): Path<String>,
) -> Result<Json<ConfigEntry>, ServerError> {
    let service = ConfigService::new(state);
    Ok(Json(service.get_config_value(key).await?))
}

#[utoipa::path(
    put,
    path = "/v1/config/{key}",
    tag = "config",
    request_body = SetConfigBody,
    responses(
        (status = 200, description = "Set a configuration entry by key", body = ConfigEntry),
        (status = 401, description = "Unauthorised (management token required)"),
        (status = 404, description = "Config key not found"),
    )
)]
async fn set_config_value(
    State(state): State<ModelState>,
    Path(key): Path<String>,
    Json(body): Json<SetConfigBody>,
) -> Result<Json<ConfigEntry>, ServerError> {
    let service = ConfigService::new(state);
    Ok(Json(service.set_config_value(key, body).await?))
}
