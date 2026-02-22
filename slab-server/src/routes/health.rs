//! Health / heartbeat endpoint.

use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::state::AppState;

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
