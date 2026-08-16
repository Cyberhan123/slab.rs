pub use slab_gpu_memory_scheduler::{GpuDeviceSnapshot, GpuStatusSnapshot};

#[derive(Debug, Clone)]
pub struct SystemDiagnosticPath {
    pub label: String,
    pub path: String,
    pub exists: bool,
}

#[derive(Debug, Clone)]
pub struct SystemDiagnosticsSnapshot {
    pub status: String,
    pub version: String,
    pub generated_at: String,
    pub transport_mode: String,
    pub swagger_enabled: bool,
    pub admin_token_configured: bool,
    pub cloud_http_trace_enabled: bool,
    pub cors_allowed_origins: Option<String>,
    pub paths: Vec<SystemDiagnosticPath>,
}
