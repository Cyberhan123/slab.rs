use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router, middleware};
use serde::Deserialize;
use utoipa::{IntoParams, OpenApi};
use validator::Validate;

use crate::api::middleware::auth;
use crate::api::validation::validate;
use slab_app_core::context::AppState;
use slab_app_core::domain::models::{
    SettingPropertySchema, SettingPropertyView, SettingValidationErrorData, SettingValueType,
    SettingsDocumentView, SettingsSectionView, SettingsSubsectionView, UpdateSettingCommand,
    UpdateSettingOperation,
};
use slab_app_core::domain::services::SettingsService;
use crate::error::ServerError;

#[derive(Debug, Deserialize, IntoParams, Validate)]
struct SettingPmidPath {
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "pmid must not be empty"
    ))]
    pmid: String,
}

#[derive(OpenApi)]
#[openapi(
    paths(list_settings, get_setting, update_setting),
    components(schemas(
        SettingsDocumentView,
        SettingsSectionView,
        SettingsSubsectionView,
        SettingPropertyView,
        SettingPropertySchema,
        SettingValueType,
        UpdateSettingCommand,
        UpdateSettingOperation,
        SettingValidationErrorData
    ))
)]
pub struct SettingsApi;

pub fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/settings", get(list_settings))
        .route("/settings/{pmid}", get(get_setting).put(update_setting))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth::auth_middleware))
        .with_state(state)
}

#[utoipa::path(
    get,
    path = "/v1/settings",
    tag = "settings",
    responses(
        (status = 200, description = "Full settings document", body = SettingsDocumentView),
        (status = 401, description = "Unauthorised (admin token required)"),
    )
)]
async fn list_settings(
    State(service): State<SettingsService>,
) -> Result<Json<SettingsDocumentView>, ServerError> {
    Ok(Json(service.list_settings().await?))
}

#[utoipa::path(
    get,
    path = "/v1/settings/{pmid}",
    tag = "settings",
    params(SettingPmidPath),
    responses(
        (status = 200, description = "Get a single setting property by PMID", body = SettingPropertyView),
        (status = 401, description = "Unauthorised (admin token required)"),
        (status = 404, description = "Setting not found"),
    )
)]
async fn get_setting(
    State(service): State<SettingsService>,
    Path(params): Path<SettingPmidPath>,
) -> Result<Json<SettingPropertyView>, ServerError> {
    let params = validate(params)?;
    Ok(Json(service.get_setting(&params.pmid).await?))
}

#[utoipa::path(
    put,
    path = "/v1/settings/{pmid}",
    tag = "settings",
    params(SettingPmidPath),
    request_body = UpdateSettingCommand,
    responses(
        (status = 200, description = "Set or unset a setting override", body = SettingPropertyView),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorised (admin token required)"),
        (status = 404, description = "Setting not found"),
    )
)]
async fn update_setting(
    State(service): State<SettingsService>,
    Path(params): Path<SettingPmidPath>,
    Json(body): Json<UpdateSettingCommand>,
) -> Result<Json<SettingPropertyView>, ServerError> {
    let params = validate(params)?;
    Ok(Json(service.update_setting(&params.pmid, body).await?))
}
