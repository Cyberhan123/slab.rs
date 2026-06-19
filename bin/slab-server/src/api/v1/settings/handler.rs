use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router, middleware};
use serde::Deserialize;
use utoipa::{IntoParams, OpenApi};
use validator::Validate;

use crate::api::middleware::auth;
use crate::api::validation::validate;
use crate::error::ServerError;
use slab_app_core::context::AppState;
use slab_app_core::domain::models::{
    SettingChangeEffect, SettingOverrideSource, SettingPropertySchema, SettingPropertyView,
    SettingValidationErrorData, SettingValueType, SettingsDocumentView, SettingsSectionView,
    SettingsSubsectionView, UpdateSettingCommand, UpdateSettingOperation,
};
use slab_app_core::domain::services::SettingsService;

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
        SettingChangeEffect,
        SettingOverrideSource,
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

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;

    use crate::api::test_support::{TestServer, TestServerOptions};

    #[tokio::test]
    async fn settings_routes_allow_loopback_without_admin_token() {
        let server = TestServer::new().await;

        let response = server.get("/v1/settings").await;

        assert_eq!(response.status, StatusCode::OK);
        assert!(response.body["sections"].as_array().is_some());
    }

    #[tokio::test]
    async fn settings_routes_require_configured_admin_token() {
        let server = TestServer::new_with(TestServerOptions {
            bind_address: Some("0.0.0.0:0".to_owned()),
            admin_api_token: Some("test-admin-token".to_owned()),
            workspace_root: None,
        })
        .await;

        let missing = server.get("/v1/settings").await;
        assert_eq!(missing.status, StatusCode::UNAUTHORIZED);

        let allowed = server.get_with_token("/v1/settings", "test-admin-token").await;
        assert_eq!(allowed.status, StatusCode::OK);
    }

    #[tokio::test]
    async fn settings_admin_token_update_changes_next_request_authorization() {
        let server = TestServer::new().await;

        let token_update = server
            .put_json(
                "/v1/settings/server.admin.token",
                json!({
                    "op": "set",
                    "value": "next-admin-token"
                }),
            )
            .await;
        assert_eq!(token_update.status, StatusCode::OK);

        let address_update = server
            .put_json_with_token(
                "/v1/settings/server.address",
                json!({
                    "op": "set",
                    "value": "0.0.0.0:0"
                }),
                "next-admin-token",
            )
            .await;
        assert_eq!(address_update.status, StatusCode::OK);

        let missing = server.get("/v1/settings").await;
        assert_eq!(missing.status, StatusCode::UNAUTHORIZED);

        let allowed = server.get_with_token("/v1/settings", "next-admin-token").await;
        assert_eq!(allowed.status, StatusCode::OK);
    }

    #[tokio::test]
    async fn settings_path_validation_rejects_blank_pmid() {
        let server = TestServer::new().await;

        let response = server.get("/v1/settings/%20").await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST);
        assert!(response.body["message"].as_str().unwrap_or_default().contains("pmid"));
        assert_eq!(
            response.body["i18n"]["message"]["key"],
            "server.errors.requestValidationFailed"
        );
    }

    #[tokio::test]
    async fn settings_missing_pmid_maps_to_not_found() {
        let server = TestServer::new().await;

        let response = server.get("/v1/settings/missing.setting").await;

        assert_eq!(response.status, StatusCode::NOT_FOUND);
        assert_eq!(response.body["i18n"]["message"]["key"], "server.errors.notFound");
    }
}
