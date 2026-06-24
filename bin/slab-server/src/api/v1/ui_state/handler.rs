use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use utoipa::OpenApi;

use crate::api::v1::ui_state::schema::{
    UiStateBatchResponse, UiStateDeleteResponse, UiStateKeyPath, UiStateValueResponse,
    UpdateUiStateRequest,
};
use crate::api::validation::{ValidatedJson, validate};
use crate::error::ServerError;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::UiStateService;

#[derive(OpenApi)]
#[openapi(
    paths(get_ui_state, get_ui_state_batch, update_ui_state, delete_ui_state),
    components(schemas(
        UiStateBatchResponse,
        UiStateDeleteResponse,
        UiStateKeyPath,
        UiStateValueResponse,
        UpdateUiStateRequest
    ))
)]
pub struct UiStateApi;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UiStateBatchQuery {
    keys: String,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ui-state", get(get_ui_state_batch))
        .route("/ui-state/{key}", get(get_ui_state).put(update_ui_state).delete(delete_ui_state))
}

#[utoipa::path(
    get,
    path = "/v1/ui-state",
    tag = "ui-state",
    params(
        ("keys" = String, Query, description = "Comma-separated UI state keys (max 32). Keys must not contain commas.")
    ),
    responses(
        (status = 200, description = "Batched UI state values; absent keys have null value/updatedAt", body = UiStateBatchResponse),
        (status = 400, description = "Bad request"),
    )
)]
async fn get_ui_state_batch(
    State(service): State<UiStateService>,
    Query(query): Query<UiStateBatchQuery>,
) -> Result<Json<UiStateBatchResponse>, ServerError> {
    let keys: Vec<String> = query
        .keys
        .split(',')
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_owned)
        .collect();
    if keys.is_empty() {
        return Err(ServerError::BadRequest("at least one ui state key is required".to_owned()));
    }
    Ok(Json(service.get_ui_state_batch(&keys).await?.into()))
}

#[utoipa::path(
    get,
    path = "/v1/ui-state/{key}",
    tag = "ui-state",
    params(UiStateKeyPath),
    responses(
        (status = 200, description = "UI state value", body = UiStateValueResponse),
        (status = 404, description = "UI state not found"),
    )
)]
async fn get_ui_state(
    State(service): State<UiStateService>,
    Path(params): Path<UiStateKeyPath>,
) -> Result<Json<UiStateValueResponse>, ServerError> {
    let params = validate(params)?;
    Ok(Json(service.get_ui_state(&params.key).await?.into()))
}

#[utoipa::path(
    put,
    path = "/v1/ui-state/{key}",
    tag = "ui-state",
    params(UiStateKeyPath),
    request_body = UpdateUiStateRequest,
    responses(
        (status = 200, description = "Updated UI state value", body = UiStateValueResponse),
        (status = 400, description = "Bad request"),
    )
)]
async fn update_ui_state(
    State(service): State<UiStateService>,
    Path(params): Path<UiStateKeyPath>,
    ValidatedJson(body): ValidatedJson<UpdateUiStateRequest>,
) -> Result<Json<UiStateValueResponse>, ServerError> {
    let params = validate(params)?;
    Ok(Json(service.update_ui_state(&params.key, body.into()).await?.into()))
}

#[utoipa::path(
    delete,
    path = "/v1/ui-state/{key}",
    tag = "ui-state",
    params(UiStateKeyPath),
    responses(
        (status = 200, description = "Deleted UI state value", body = UiStateDeleteResponse),
        (status = 400, description = "Bad request"),
    )
)]
async fn delete_ui_state(
    State(service): State<UiStateService>,
    Path(params): Path<UiStateKeyPath>,
) -> Result<Json<UiStateDeleteResponse>, ServerError> {
    let params = validate(params)?;
    Ok(Json(service.delete_ui_state(&params.key).await?.into()))
}

#[cfg(test)]
mod route_tests {
    use axum::http::StatusCode;
    use chrono::Utc;
    use serde_json::Value;
    use slab_app_core::infra::db::{UiStateRecord, UiStateStore};
    use utoipa::OpenApi;

    use super::UiStateApi;
    use crate::api::test_support::TestServer;

    fn entry_value(entries: &Value, key: &str) -> Option<Value> {
        entries
            .as_array()
            .and_then(|items| {
                items.iter().find(|entry| entry["key"] == Value::String(key.to_owned()))
            })
            .map(|entry| entry["value"].clone())
    }

    #[test]
    fn ui_state_batch_route_published_in_openapi() {
        let openapi =
            serde_json::to_value(UiStateApi::openapi()).expect("serialize ui-state openapi");
        assert!(openapi["paths"]["/v1/ui-state"]["get"].is_object(), "missing GET /v1/ui-state");
    }

    #[tokio::test]
    async fn ui_state_batch_returns_present_and_absent_keys() {
        let server = TestServer::new().await;
        let now = Utc::now();
        server
            .store
            .upsert_ui_state(UiStateRecord {
                key: "zustand:assistant-ui".to_owned(),
                value: "assistant-blob".to_owned(),
                updated_at: now,
            })
            .await
            .expect("seed assistant ui state");

        let response =
            server.get("/v1/ui-state?keys=zustand:assistant-ui,zustand:missing-ui").await;

        assert_eq!(response.status, StatusCode::OK);
        let entries = &response.body["entries"];
        assert_eq!(
            entry_value(entries, "zustand:assistant-ui"),
            Some(Value::String("assistant-blob".to_owned()))
        );
        assert_eq!(entry_value(entries, "zustand:missing-ui"), Some(Value::Null));
    }

    #[tokio::test]
    async fn ui_state_batch_rejects_empty_keys() {
        let server = TestServer::new().await;

        let response = server.get("/v1/ui-state?keys=").await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST);
    }
}
