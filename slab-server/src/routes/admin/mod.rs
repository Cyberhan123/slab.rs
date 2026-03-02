pub mod backend;
pub mod config;
pub mod models;

use crate::state::AppState;

use axum::Router;
use std::sync::Arc;
use utoipa::OpenApi;

// Routes nested under `/admin` (models, dylib, backend, config).
pub fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .merge(backend::router())
        .merge(config::router())
        .merge(models::router())
        // .route_layer(middleware::from_fn_with_state(
        //     state.clone(),
        //     auth::auth_middleware,
        // ))
        .with_state(state.clone())
}

#[derive(OpenApi)]
#[openapi()]
pub struct AdminApi;

pub fn api_docs() -> utoipa::openapi::OpenApi {
    let mut spec = AdminApi::openapi();
    spec.merge(config::ConfigApi::openapi());
    spec.merge(backend::BackendApi::openapi());
    spec.merge(models::ModelsAdminApi::openapi());
    spec
}
