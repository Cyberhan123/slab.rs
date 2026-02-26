//! Health / heartbeat endpoint.

use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::sync::Arc;
use utoipa::OpenApi;

use crate::state::AppState;

#[derive(OpenApi)]
#[openapi(paths(get_health))]
pub struct HealthApi;

/// Register health-check routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/health", get(get_health))
}

/// Heartbeat endpoint.
///
/// Returns `{"status": "ok", "version": "..."}` with HTTP 200.
/// Load-balancers and monitoring systems should poll this endpoint.
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Server is healthy", body = Value)
    )
)]
pub async fn get_health() -> Json<Value> {
    Json(json!({
        "status":  "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn health_response_has_ok_status() {
        let Json(body) = get_health().await;
        assert_eq!(body["status"], "ok");
    }

    #[tokio::test]
    async fn health_response_has_version() {
        let Json(body) = get_health().await;
        assert!(!body["version"].as_str().unwrap_or("").is_empty());
    }
}
