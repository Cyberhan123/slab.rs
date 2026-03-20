//! Axum router construction.
//!
//! [`build`] assembles the complete application router, including:
//! - Middleware layers (CORS, per-request trace-ID injection)
//! - Optional Swagger UI / OpenAPI spec endpoint (disable with `SLAB_ENABLE_SWAGGER=false`)
//! - Health / heartbeat route
//! - OpenAI-compatible `/v1` routes

pub mod doc;
pub mod health;
mod middleware;
pub(crate) mod v1;
pub mod validation;
use crate::api::middleware::{cors, trace};
use crate::context::AppState;
use axum::{middleware as axum_middleware, Router};
use std::sync::Arc;
use tower::ServiceBuilder;
use utoipa_swagger_ui::SwaggerUi;
// ── Router builder ────────────────────────────────────────────────────────────

/// Build the HTTP gateway router used by supervisor mode.
pub fn build(state: Arc<AppState>) -> Router {
    let api_router = Router::new().merge(health::router()).nest("/v1", v1::router(state.clone()));

    let mut app = Router::new().merge(api_router);
    let api_doc = doc::get_docs();

    if state.context.config.enable_swagger {
        app = app.merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api_doc));
    }

    app.layer(ServiceBuilder::new().layer(cors::cors_layer(state.clone())))
        .layer(axum_middleware::from_fn_with_state(state.clone(), trace::trace_middleware))
        .with_state(state)
}
