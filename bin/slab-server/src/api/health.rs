//! Health / heartbeat endpoint.

use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use std::sync::Arc;
use utoipa::{OpenApi, ToSchema};

use slab_app_core::context::AppState;

#[derive(OpenApi)]
#[openapi(paths(get_health), components(schemas(HealthResponse)))]
pub struct HealthApi;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

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
        (status = 200, description = "Server is healthy", body = HealthResponse)
    )
)]
pub async fn get_health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok".to_owned(), version: env!("CARGO_PKG_VERSION").to_owned() })
}

//  Tests

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn health_response_has_ok_status() {
        let Json(body) = get_health().await;
        assert_eq!(body.status, "ok");
    }

    #[tokio::test]
    async fn health_response_has_version() {
        let Json(body) = get_health().await;
        assert!(!body.version.is_empty());
    }
}
