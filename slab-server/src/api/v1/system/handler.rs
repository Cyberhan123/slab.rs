use std::sync::Arc;

use axum::routing::get;
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::v1::system::schema::{GpuDeviceStatus, GpuStatusResponse};
use crate::context::AppState;
use crate::services::system::SystemService;

#[derive(OpenApi)]
#[openapi(
    paths(gpu_status),
    components(schemas(GpuStatusResponse, GpuDeviceStatus))
)]
pub struct SystemApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/system/gpu", get(gpu_status))
}

#[utoipa::path(
    get,
    path = "/v1/system/gpu",
    tag = "system",
    responses(
        (status = 200, description = "Current GPU telemetry snapshot", body = GpuStatusResponse),
    )
)]
async fn gpu_status() -> Json<GpuStatusResponse> {
    Json(SystemService::gpu_status().await)
}
