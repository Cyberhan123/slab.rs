use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{middleware, Json, Router};
use serde::Deserialize;
use utoipa::OpenApi;
use validator::Validate;

use crate::api::middleware::auth;
use crate::api::validation::{validate, ValidatedJson};
use crate::api::v1::settings::schema::{
    SettingResponse, SettingsSystemResponse, UpdateSettingRequest,
};
use crate::context::AppState;
use crate::domain::services::SettingsService;
use crate::error::ServerError;

#[derive(OpenApi)]
#[openapi(
    paths(list_settings, get_setting, update_setting, system_info),
    components(schemas(
        SettingResponse,
        SettingsSystemResponse,
        crate::domain::models::SettingCategory,
        crate::domain::models::SettingControl,
        crate::domain::models::SettingValidation,
        UpdateSettingRequest
    ))
)]
pub struct SettingsApi;

pub fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/settings", get(list_settings))
        .route("/settings/system", get(system_info))
        .route("/settings/{key}", get(get_setting).put(update_setting))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ))
        .with_state(state)
}

#[utoipa::path(
    get,
    path = "/v1/settings",
    tag = "settings",
    responses(
        (status = 200, description = "List settings metadata and current values", body = [SettingResponse]),
        (status = 401, description = "Unauthorised (admin token required)"),
    )
)]
async fn list_settings(
    State(service): State<SettingsService>,
) -> Result<Json<Vec<SettingResponse>>, ServerError> {
    let items = service
        .list_settings()
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    Ok(Json(items))
}

#[utoipa::path(
    get,
    path = "/v1/settings/{key}",
    tag = "settings",
    responses(
        (status = 200, description = "Get a setting by key", body = SettingResponse),
        (status = 401, description = "Unauthorised (admin token required)"),
        (status = 404, description = "Setting not found"),
    )
)]
async fn get_setting(
    State(service): State<SettingsService>,
    Path(params): Path<SettingKeyPath>,
) -> Result<Json<SettingResponse>, ServerError> {
    let params = validate(params)?;
    Ok(Json(service.get_setting(&params.key).await?.into()))
}

#[utoipa::path(
    put,
    path = "/v1/settings/{key}",
    tag = "settings",
    request_body = UpdateSettingRequest,
    responses(
        (status = 200, description = "Update a setting", body = SettingResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorised (admin token required)"),
        (status = 404, description = "Setting not found"),
    )
)]
async fn update_setting(
    State(service): State<SettingsService>,
    Path(params): Path<SettingKeyPath>,
    ValidatedJson(body): ValidatedJson<UpdateSettingRequest>,
) -> Result<Json<SettingResponse>, ServerError> {
    let params = validate(params)?;
    Ok(Json(service.update_setting(&params.key, body.into()).await?.into()))
}

#[utoipa::path(
    get,
    path = "/v1/settings/system",
    tag = "settings",
    responses(
        (status = 200, description = "Read-only system and backend facts used by Settings UI", body = SettingsSystemResponse),
        (status = 401, description = "Unauthorised (admin token required)"),
    )
)]
async fn system_info(
    State(service): State<SettingsService>,
) -> Result<Json<SettingsSystemResponse>, ServerError> {
    Ok(Json(service.system_info().await?.into()))
}

#[derive(Debug, Deserialize, Validate)]
struct SettingKeyPath {
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "key must not be empty"
    ))]
    key: String,
}
