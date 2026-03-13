use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use utoipa::OpenApi;
use validator::Validate;

use crate::api::v1::config::schema::{ConfigEntry, SetConfigBody};
use crate::api::validation::{validate, ValidatedJson};
use crate::context::AppState;
use crate::domain::services::ConfigService;
use crate::error::ServerError;

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
    State(service): State<ConfigService>,
) -> Result<Json<Vec<ConfigEntry>>, ServerError> {
    let entries = service
        .list_config()
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    Ok(Json(entries))
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
    State(service): State<ConfigService>,
    Path(params): Path<ConfigKeyPath>,
) -> Result<Json<ConfigEntry>, ServerError> {
    let params = validate(params)?;
    Ok(Json(service.get_config_value(params.key).await?.into()))
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
    State(service): State<ConfigService>,
    Path(params): Path<ConfigKeyPath>,
    ValidatedJson(body): ValidatedJson<SetConfigBody>,
) -> Result<Json<ConfigEntry>, ServerError> {
    let params = validate(params)?;
    Ok(Json(
        service
            .set_config_value(params.key, body.into())
            .await?
            .into(),
    ))
}

#[derive(Debug, Deserialize, Validate)]
struct ConfigKeyPath {
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "key must not be empty"
    ))]
    key: String,
}
