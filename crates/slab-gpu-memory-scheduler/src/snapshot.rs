//! GPU telemetry snapshot types shared by the probe, scheduler, and hosts.

/// Per-device telemetry from one probe round. `uuid` is the backend's stable
/// device key (all-smi fills it where the driver provides one); `id` is only
/// an enumeration index within the snapshot and is not stable across rounds.
#[derive(Debug, Clone)]
pub struct GpuDeviceSnapshot {
    pub id: u32,
    pub uuid: Option<String>,
    pub name: String,
    pub device_type: String,
    pub utilization_percent: f64,
    pub temperature_celsius: u32,
    pub used_memory_bytes: u64,
    pub total_memory_bytes: u64,
    pub memory_usage_percent: f64,
    pub power_draw_watts: f64,
}

/// One probe round over all devices.
#[derive(Debug, Clone)]
pub struct GpuStatusSnapshot {
    pub available: bool,
    pub backend: String,
    pub updated_at: String,
    pub devices: Vec<GpuDeviceSnapshot>,
    pub error: Option<String>,
}

/// Stable per-device memory gauge used by policy math (free = total − used).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceMemoryGauge {
    pub uuid: Option<String>,
    pub used_bytes: u64,
    pub total_bytes: u64,
}

impl DeviceMemoryGauge {
    pub fn free_bytes(&self) -> u64 {
        self.total_bytes.saturating_sub(self.used_bytes)
    }
}
