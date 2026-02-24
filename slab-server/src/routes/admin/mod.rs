pub mod backend;
pub mod config;

use crate::middleware::auth;
use crate::state::AppState;

use axum::body::Body;
use axum::http::Request;
use axum::middleware;
use axum::middleware::Next;
use axum::Router;
use std::sync::Arc;
use utoipa::OpenApi;

// Routes nested under `/admin` (models, dylib, backend, config).
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .merge(backend::router())
        .merge(config::router())
          .layer(middleware::from_fn_with_state(
            Arc::new(()) as Arc<()>,
            |req: Request<Body>, next: Next| async move {
                auth::check_management_auth(req, next).await
            },
        ))
}

#[derive(OpenApi)]
#[openapi()]
pub struct AdminApi;

pub fn api_docs() -> utoipa::openapi::OpenApi {
    let mut spec = AdminApi::openapi();
    spec.merge(config::ConfigApi::openapi());
    spec.merge(backend::BackendApi::openapi());
    spec
}
