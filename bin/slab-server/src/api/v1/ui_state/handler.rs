use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::v1::ui_state::schema::{
    UiStateDeleteResponse, UiStateKeyPath, UiStateValueResponse, UpdateUiStateRequest,
};
use crate::api::validation::{ValidatedJson, validate};
use crate::error::ServerError;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::UiStateService;

#[derive(OpenApi)]
#[openapi(
    paths(get_ui_state, update_ui_state, delete_ui_state),
    components(schemas(
        UiStateDeleteResponse,
        UiStateKeyPath,
        UiStateValueResponse,
        UpdateUiStateRequest
    ))
)]
pub struct UiStateApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ui-state/{key}", get(get_ui_state).put(update_ui_state).delete(delete_ui_state))
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
