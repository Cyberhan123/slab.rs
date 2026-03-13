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
