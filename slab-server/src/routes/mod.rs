//! Axum router construction.
//!
//! [`build`] assembles the complete application router, including:
//! - Middleware layers (CORS, per-request trace-ID injection)
//! - Optional Swagger UI / OpenAPI spec endpoint (disable with `SLAB_ENABLE_SWAGGER=false`)
//! - Health / heartbeat route
//! - OpenAI-compatible `/v1` routes
//! - admin `/admin` routes (optionally protected by bearer token)

mod admin;
pub mod doc;
mod health;
mod v1;
use axum::{
    middleware::{self},
    Router,
};
use crate::middleware::{cors,trace};
use crate::state::AppState;
use std::sync::Arc;
use tower::ServiceBuilder;
use utoipa_swagger_ui::SwaggerUi;
// ── Router builder ────────────────────────────────────────────────────────────

/// Build the complete Axum [`Router`] for the application.
pub fn build(state: Arc<AppState>) -> Router {
    let api_router = Router::new()
        .merge(health::router())
        .nest("/v1", v1::router())
        .nest("/admin", admin::router(state.clone()));

    let mut app = Router::new().merge(api_router);

    // ── Swagger UI ────────────────────────────────────────────────────────────
    // Enabled by default; disable with SLAB_ENABLE_SWAGGER=false in production
    // to avoid exposing the API structure to potential attackers.
    let api_doc = doc::get_docs();

    if state.config.enable_swagger {
        app = app.merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api_doc));
    }

    app
        // Outermost layers execute first on the way in.
        .layer(ServiceBuilder::new().layer(cors::cors_layer(state.clone())))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            trace::trace_middleware,
        ))
        .with_state(state)
}
