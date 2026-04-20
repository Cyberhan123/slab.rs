use std::sync::Arc;

use axum::http::HeaderValue;
use slab_app_core::context::AppState;
use slab_types::desktop_dev_allowed_origins;
use tower_http::cors::{Any, CorsLayer};
use tracing::warn;

pub fn cors_layer(state: Arc<AppState>) -> CorsLayer {
    let origins = resolve_allowed_origins(state.context.config.cors_allowed_origins.as_deref());
    cors_layer_with_origins(origins)
}

fn cors_layer_with_origins(origins: Vec<HeaderValue>) -> CorsLayer {
    let mut layer = CorsLayer::new().allow_headers(Any).allow_methods(Any);
    if !origins.is_empty() {
        layer = layer.allow_origin(origins);
    }
    layer
}

fn resolve_allowed_origins(configured_origins: Option<&str>) -> Vec<HeaderValue> {
    match configured_origins {
        Some(origins_str) => parse_configured_origins(origins_str).unwrap_or_else(|| {
            warn!(
                configured = %origins_str,
                "SLAB_CORS_ORIGINS resolved to an empty allowlist; rejecting cross-origin requests"
            );
            Vec::new()
        }),
        None => default_allowed_origins(),
    }
}

fn parse_configured_origins(origins_str: &str) -> Option<Vec<HeaderValue>> {
    let mut origins = Vec::new();

    for raw_origin in origins_str.split(',') {
        let origin = raw_origin.trim();
        if origin.is_empty() {
            continue;
        }

        match origin.parse::<HeaderValue>() {
            Ok(origin) => origins.push(origin),
            Err(error) => warn!(origin = %origin, error = %error, "ignoring invalid CORS origin"),
        }
    }

    (!origins.is_empty()).then_some(origins)
}

fn default_allowed_origins() -> Vec<HeaderValue> {
    desktop_dev_allowed_origins().iter().map(|origin| HeaderValue::from_static(origin)).collect()
}

#[cfg(test)]
mod tests {
    use super::resolve_allowed_origins;
    use axum::Router;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use tower::ServiceExt;

    use crate::api::middleware::cors::cors_layer_with_origins;

    async fn preflight_allow_origin(
        configured_origins: Option<&str>,
        origin: &str,
    ) -> Option<String> {
        let app = Router::new()
            .route("/status", get(|| async { "ok" }))
            .layer(cors_layer_with_origins(resolve_allowed_origins(configured_origins)));

        let response = app
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/status")
                    .header("Origin", origin)
                    .header("Access-Control-Request-Method", "GET")
                    .header("Access-Control-Request-Headers", "Authorization")
                    .body(Body::empty())
                    .expect("preflight request"),
            )
            .await
            .expect("CORS middleware response");

        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned)
    }

    #[tokio::test]
    async fn default_allowlist_allows_vite_dev_origin() {
        let allow_origin = preflight_allow_origin(None, "http://localhost:1420").await;

        assert_eq!(allow_origin.as_deref(), Some("http://localhost:1420"));
    }

    #[tokio::test]
    async fn default_allowlist_rejects_unrelated_origin() {
        let allow_origin = preflight_allow_origin(None, "https://example.com").await;

        assert_eq!(allow_origin, None);
    }

    #[tokio::test]
    async fn explicit_cors_origins_override_default_allowlist() {
        let explicit_allow_origin =
            preflight_allow_origin(Some("https://example.com"), "https://example.com").await;
        let default_allow_origin =
            preflight_allow_origin(Some("https://example.com"), "http://localhost:1420").await;

        assert_eq!(explicit_allow_origin.as_deref(), Some("https://example.com"));
        assert_eq!(default_allow_origin, None);
    }

    #[tokio::test]
    async fn empty_configured_cors_origin_list_rejects_cross_origin_requests() {
        let allow_origin = preflight_allow_origin(Some(" , "), "http://localhost:1420").await;

        assert_eq!(allow_origin, None);
    }
}
