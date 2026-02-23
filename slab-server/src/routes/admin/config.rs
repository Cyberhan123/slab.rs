//! Config management endpoints.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::entities::ConfigStore;
use crate::error::ServerError;
use crate::state::AppState;
use crate::schemas::admin::config::{ConfigEntry, SetConfigBody};

#[derive(OpenApi)]
#[openapi(
    paths(list_config, get_config_value, set_config_value),
    components(schemas(
        ConfigEntry,
        SetConfigBody,
        
    ))
)]
pub struct ConfigApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/config",       get(list_config))
        .route("/config/{key}", get(get_config_value).put(set_config_value))
}
#[utoipa::path(
    get,
    path = "/admin/config",
    tag = "management",
    responses(
        (status = 200, description = "List of all configuration entries", body = Vec<ConfigEntry>),
        (status = 401, description = "Unauthorised (management token required)"),
    )
)]  
pub async fn list_config(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ConfigEntry>>, ServerError> {
    let entries = state.store.list_config_values().await?;
    Ok(Json(
        entries
            .into_iter()
            .map(|(key, value)| ConfigEntry { key, value })
            .collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/admin/config/{key}",
    tag = "management",
    responses(
        (status = 200, description = "Get a configuration entry by key", body = ConfigEntry),
        (status = 401, description = "Unauthorised (management token required)"),
        (status = 404, description = "Config key not found"),
    )
)]
pub async fn get_config_value(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> Result<Json<ConfigEntry>, ServerError> {
    let value = state
        .store
        .get_config_value(&key)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("config key '{key}' not found")))?;
    Ok(Json(ConfigEntry { key, value }))
}

#[utoipa::path(
    put,
    path = "/admin/config/{key}",
    tag = "management",
    request_body = SetConfigBody,
    responses(
        (status = 200, description = "Set a configuration entry by key", body = ConfigEntry),
        (status = 401, description = "Unauthorised (management token required)"),
        (status = 404, description = "Config key not found"),
    )
)]
pub async fn set_config_value(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
    Json(body): Json<SetConfigBody>,
) -> Result<Json<ConfigEntry>, ServerError> {
    state.store.set_config_value(&key, &body.value).await?;
    Ok(Json(ConfigEntry { key, value: body.value }))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn config_entry_fields() {
        let e = ConfigEntry { key: "foo".into(), value: "bar".into() };
        assert_eq!(e.key, "foo");
        assert_eq!(e.value, "bar");
    }
}
