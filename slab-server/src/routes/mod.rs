//! Axum router construction.
//!
//! [`build`] assembles the complete application router, including:
//! - Middleware layers (CORS, per-request trace-ID injection)
//! - Optional Swagger UI / OpenAPI spec endpoint (disable with `SLAB_ENABLE_SWAGGER=false`)
//! - Health / heartbeat route
//! - OpenAI-compatible `/v1` routes
//! - admin `/admin` routes (optionally protected by bearer token)


mod health;
mod v1;
mod admin;
pub mod doc;

use std::sync::Arc;
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use utoipa_swagger_ui::SwaggerUi;
use crate::middleware::TraceLayer;
use crate::state::AppState;

// ── Router builder ────────────────────────────────────────────────────────────

/// Build the complete Axum [`Router`] for the application.
pub fn build(state: Arc<AppState>) -> Router {
    // ── CORS ─────────────────────────────────────────────────────────────────
    // Default allows all origins.  In production, restrict via SLAB_CORS_ORIGINS.
    let cors = if let Some(origins_str) = &state.config.cors_allowed_origins {
        // Parse the comma-separated origin list and build a restrictive layer.
        let origins: Vec<axum::http::HeaderValue> = origins_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if origins.is_empty() {
            CorsLayer::new().allow_origin(Any).allow_headers(Any).allow_methods(Any)
        } else {
            CorsLayer::new()
                .allow_origin(origins)
                .allow_headers(Any)
                .allow_methods(Any)
        }
    } else {
        // Wildcard – suitable for development; set SLAB_CORS_ORIGINS in production.
        CorsLayer::new().allow_origin(Any).allow_headers(Any).allow_methods(Any)
    };

    let api_router = Router::new()
        .merge(health::router())
        .nest("/v1",  v1::router())
        .nest("/admin", admin::router());

    let mut app = Router::new().merge(api_router);

    // ── Swagger UI ────────────────────────────────────────────────────────────
    // Enabled by default; disable with SLAB_ENABLE_SWAGGER=false in production
    // to avoid exposing the API structure to potential attackers.
    let api_doc = doc::get_docs();

    if state.config.enable_swagger {
        app = app.merge(
            SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", api_doc),
        );
    }

    app
        // Outermost layers execute first on the way in.
        .layer(TraceLayer::new(Arc::clone(&state)))
        .layer(cors)
        .with_state(state)
}


