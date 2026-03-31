use crate::context::AppState;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

pub fn cors_layer(state: Arc<AppState>) -> CorsLayer {
    if let Some(origins_str) = &state.context.config.cors_allowed_origins {
        // Parse the comma-separated origin list and build a restrictive layer.
        let origins: Vec<axum::http::HeaderValue> = origins_str
            .split(',')
            .filter_map(|s| s.trim().parse::<axum::http::HeaderValue>().ok())
            .collect();
        if origins.is_empty() {
            CorsLayer::new().allow_origin(Any).allow_headers(Any).allow_methods(Any)
        } else {
            CorsLayer::new().allow_origin(origins).allow_headers(Any).allow_methods(Any)
        }
    } else {
        // Wildcard – suitable for development; set SLAB_CORS_ORIGINS in production.
        CorsLayer::new().allow_origin(Any).allow_headers(Any).allow_methods(Any)
    }
}
