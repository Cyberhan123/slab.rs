use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::v1::system::schema::{GpuDeviceStatus, GpuStatusResponse};
use crate::context::AppState;
use crate::services::system::{GpuDeviceSnapshot, GpuStatusSnapshot, SystemService};

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
async fn gpu_status(
    State(service): State<SystemService>,
) -> Json<GpuStatusResponse> {
    Json(to_gpu_status_response(service.gpu_status().await))
}

fn to_gpu_status_response(snapshot: GpuStatusSnapshot) -> GpuStatusResponse {
    GpuStatusResponse {
        available: snapshot.available,
        backend: snapshot.backend,
        updated_at: snapshot.updated_at,
        devices: snapshot
            .devices
            .into_iter()
            .map(to_gpu_device_status)
            .collect(),
        error: snapshot.error,
    }
}

fn to_gpu_device_status(snapshot: GpuDeviceSnapshot) -> GpuDeviceStatus {
    GpuDeviceStatus {
        id: snapshot.id,
        name: snapshot.name,
        device_type: snapshot.device_type,
        utilization_percent: snapshot.utilization_percent,
        temperature_celsius: snapshot.temperature_celsius,
        used_memory_bytes: snapshot.used_memory_bytes,
        total_memory_bytes: snapshot.total_memory_bytes,
        memory_usage_percent: snapshot.memory_usage_percent,
        power_draw_watts: snapshot.power_draw_watts,
    }
}
