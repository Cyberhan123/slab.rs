use all_smi::AllSmi;
use chrono::Utc;
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct GpuDeviceSnapshot {
    pub id: u32,
    pub name: String,
    pub device_type: String,
    pub utilization_percent: f64,
    pub temperature_celsius: u32,
    pub used_memory_bytes: u64,
    pub total_memory_bytes: u64,
    pub memory_usage_percent: f64,
    pub power_draw_watts: f64,
}

#[derive(Debug, Clone)]
pub struct GpuStatusSnapshot {
    pub available: bool,
    pub backend: String,
    pub updated_at: String,
    pub devices: Vec<GpuDeviceSnapshot>,
    pub error: Option<String>,
}

#[derive(Clone, Default)]
pub struct SystemService;

impl SystemService {
    pub fn new() -> Self {
        Self
    }

    pub async fn gpu_status(&self) -> GpuStatusSnapshot {
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

        GpuStatusSnapshot {
            available,
            backend: "all-smi".to_owned(),
            updated_at: Utc::now().to_rfc3339(),
            devices,
            error,
        }
    }
}

fn memory_usage_percent(used: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    ((used as f64) / (total as f64) * 100.0).clamp(0.0, 100.0)
}

fn collect_gpu_devices() -> Result<Vec<GpuDeviceSnapshot>, String> {
    let all_smi = AllSmi::new().map_err(|err| err.to_string())?;
    let devices = all_smi
        .get_gpu_info()
        .into_iter()
        .enumerate()
        .map(|(index, gpu)| GpuDeviceSnapshot {
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
