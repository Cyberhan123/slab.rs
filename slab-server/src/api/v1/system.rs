//! System telemetry routes.

use std::sync::Arc;

use all_smi::AllSmi;
use axum::routing::get;
use axum::{Json, Router};
use chrono::Utc;
use tracing::{debug, warn};
use utoipa::OpenApi;

use crate::schemas::v1::system::{GpuDeviceStatus, GpuStatusResponse};
use crate::context::AppState;

#[derive(OpenApi)]
#[openapi(
    paths(gpu_status),
    components(schemas(GpuStatusResponse, GpuDeviceStatus))
)]
pub struct SystemApi;

/// Register system telemetry routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/system/gpu", get(gpu_status))
}

fn memory_usage_percent(used: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    ((used as f64) / (total as f64) * 100.0).clamp(0.0, 100.0)
}

fn collect_gpu_devices() -> Result<Vec<GpuDeviceStatus>, String> {
    let all_smi = AllSmi::new().map_err(|err| err.to_string())?;
    let devices = all_smi
        .get_gpu_info()
        .into_iter()
        .enumerate()
        .map(|(index, gpu)| GpuDeviceStatus {
            id: index as u32,
            name: gpu.name,
            device_type: gpu.device_type,
            utilization_percent: gpu.utilization,
            temperature_celsius: gpu.temperature,
            used_memory_bytes: gpu.used_memory,
            total_memory_bytes: gpu.total_memory,
            memory_usage_percent: memory_usage_percent(gpu.used_memory, gpu.total_memory),
            power_draw_watts: gpu.power_consumption,
        })
        .collect();

    Ok(devices)
}

#[utoipa::path(
    get,
    path = "/v1/system/gpu",
    tag = "system",
    responses(
        (status = 200, description = "Current GPU telemetry snapshot", body = GpuStatusResponse),
    )
)]
pub async fn gpu_status() -> Json<GpuStatusResponse> {
    let snapshot = tokio::task::spawn_blocking(collect_gpu_devices).await;

    let (available, devices, error) = match snapshot {
        Ok(Ok(devices)) if devices.is_empty() => (
            false,
            Vec::new(),
            Some("No GPU device detected by all-smi".to_owned()),
        ),
        Ok(Ok(devices)) => (true, devices, None),
        Ok(Err(err)) => {
            warn!(error = %err, "failed to refresh gpu telemetry");
            (
                false,
                Vec::new(),
                Some(format!("GPU telemetry unavailable: {err}")),
            )
        }
        Err(err) => {
            warn!(error = %err, "gpu telemetry worker panicked");
            (
                false,
                Vec::new(),
                Some("GPU telemetry worker failed".to_owned()),
            )
        }
    };

    if available {
        debug!(device_count = devices.len(), "gpu telemetry snapshot ready");
    }

    Json(GpuStatusResponse {
        available,
        backend: "all-smi".to_owned(),
        updated_at: Utc::now().to_rfc3339(),
        devices,
        error,
    })
}

