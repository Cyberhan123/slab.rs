use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router, middleware};
use utoipa::OpenApi;

use crate::api::middleware::auth;
use crate::api::v1::backend::schema::{
    BackendListResponse, BackendStatusResponse, BackendTypeQuery,
};
use crate::api::validation::ValidatedQuery;
use crate::error::ServerError;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::BackendService;

#[derive(OpenApi)]
#[openapi(
    paths(backend_status, list_backends),
    components(schemas(BackendTypeQuery, BackendStatusResponse, BackendListResponse,))
)]
pub struct BackendApi;

pub fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/backends", get(list_backends))
        .route("/backends/status", get(backend_status))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth::auth_middleware))
        .with_state(state)
}

#[utoipa::path(
    get,
    path = "/v1/backends/status",
    tag = "backends",
    params(BackendTypeQuery),
    responses(
        (status = 200, description = "Backend worker is running", body = BackendStatusResponse),
        (status = 400, description = "Unknown model type"),
        (status = 401, description = "Unauthorised (admin token required)"),
    )
)]
async fn backend_status(
    State(service): State<BackendService>,
    ValidatedQuery(query): ValidatedQuery<BackendTypeQuery>,
) -> Result<Json<BackendStatusResponse>, ServerError> {
    Ok(Json(service.backend_status(query.into()).await?.into()))
}

#[utoipa::path(
    get,
    path = "/v1/backends",
    tag = "backends",
    responses(
        (status = 200, description = "List of all registered backends", body = BackendListResponse),
        (status = 401, description = "Unauthorised (admin token required)"),
    )
)]
async fn list_backends(
    State(service): State<BackendService>,
) -> Result<Json<BackendListResponse>, ServerError> {
    let backends = service.list_backends().await?.into_iter().map(Into::into).collect();
    Ok(Json(BackendListResponse { backends }))
}
