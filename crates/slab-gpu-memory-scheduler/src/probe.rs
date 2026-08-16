//! Hardware abstraction for GPU telemetry collection. The all-smi wrapper is
//! the only place the workspace touches the `all-smi` crate.

use async_trait::async_trait;

use crate::error::GpuMemoryError;
use crate::snapshot::GpuDeviceSnapshot;

/// One round of GPU telemetry collection. Implementations perform their own
/// `spawn_blocking` internally so callers can await safely from async
/// contexts.
#[async_trait]
pub trait GpuProbe: Send + Sync {
    /// Backend identifier surfaced in snapshots (e.g. "all-smi").
    fn backend_name(&self) -> &'static str;

    /// Collect a fresh device snapshot list. An empty list means no devices
    /// were detected; telemetry being unavailable is an error.
    async fn probe(&self) -> Result<Vec<GpuDeviceSnapshot>, GpuMemoryError>;
}

/// Headless default when the `gpu-telemetry` feature is off: telemetry is
/// unavailable and every probe reports [`GpuMemoryError::TelemetryDisabled`].
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopGpuProbe;

#[async_trait]
impl GpuProbe for NoopGpuProbe {
    fn backend_name(&self) -> &'static str {
        "noop"
    }

    async fn probe(&self) -> Result<Vec<GpuDeviceSnapshot>, GpuMemoryError> {
        Err(GpuMemoryError::TelemetryDisabled)
    }
}

/// all-smi-backed probe (feature `gpu-telemetry`). A fresh `AllSmi` instance
/// and full device enumeration per round — same semantics the legacy
/// `SystemService::collect_gpu_devices` had, now concentrated here.
#[cfg(feature = "gpu-telemetry")]
#[derive(Debug, Default, Clone, Copy)]
pub struct AllSmiProbe;

#[cfg(feature = "gpu-telemetry")]
#[async_trait]
impl GpuProbe for AllSmiProbe {
    fn backend_name(&self) -> &'static str {
        "all-smi"
    }

    async fn probe(&self) -> Result<Vec<GpuDeviceSnapshot>, GpuMemoryError> {
        tokio::task::spawn_blocking(collect_devices)
            .await
            .map_err(|_| GpuMemoryError::WorkerPanic)?
    }
}

#[cfg(feature = "gpu-telemetry")]
fn collect_devices() -> Result<Vec<GpuDeviceSnapshot>, GpuMemoryError> {
    let all_smi =
        all_smi::AllSmi::new().map_err(|err| GpuMemoryError::Probe { message: err.to_string() })?;
    let devices = all_smi
        .get_gpu_info()
        .into_iter()
        .enumerate()
        .map(|(index, gpu)| {
            let uuid = (!gpu.uuid.trim().is_empty()).then_some(gpu.uuid);
            GpuDeviceSnapshot {
                id: index as u32,
                uuid,
                name: gpu.name,
                device_type: gpu.device_type,
                utilization_percent: gpu.utilization,
                temperature_celsius: gpu.temperature,
                used_memory_bytes: gpu.used_memory,
                total_memory_bytes: gpu.total_memory,
                memory_usage_percent: memory_usage_percent(gpu.used_memory, gpu.total_memory),
                power_draw_watts: gpu.power_consumption,
            }
        })
        .collect();
    Ok(devices)
}

#[cfg(feature = "gpu-telemetry")]
fn memory_usage_percent(used: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    ((used as f64) / (total as f64) * 100.0).clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn noop_probe_reports_telemetry_disabled() {
        let error = NoopGpuProbe.probe().await.unwrap_err();
        assert!(matches!(error, GpuMemoryError::TelemetryDisabled));
        assert_eq!(
            error.snapshot_error_message(),
            "GPU telemetry backend is disabled in this build"
        );
    }
}
