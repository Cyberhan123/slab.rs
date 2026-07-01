use std::path::Path;

use crate::context::ModelState;
use crate::domain::models::{
    GpuDeviceSnapshot, GpuStatusSnapshot, SystemDiagnosticPath, SystemDiagnosticsSnapshot,
};
use crate::error::AppCoreError;
use crate::schemas::system::AgentDiagnosticsResponse;
#[cfg(feature = "gpu-telemetry")]
use all_smi::AllSmi;
use chrono::Utc;
use tracing::{debug, warn};

#[derive(Clone, Default)]
pub struct SystemService {
    model_state: Option<ModelState>,
}

impl SystemService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with_model_state(model_state: ModelState) -> Self {
        Self { model_state: Some(model_state) }
    }

    pub async fn gpu_status(&self) -> GpuStatusSnapshot {
        let snapshot = tokio::task::spawn_blocking(collect_gpu_devices).await;

        let (available, devices, error) = match snapshot {
            Ok(Ok(devices)) if devices.is_empty() => {
                (false, Vec::new(), Some("No GPU device detected by all-smi".to_owned()))
            }
            Ok(Ok(devices)) => (true, devices, None),
            Ok(Err(err)) => {
                warn!(error = %err, "failed to refresh gpu telemetry");
                (false, Vec::new(), Some(format!("GPU telemetry unavailable: {err}")))
            }
            Err(err) => {
                warn!(error = %err, "gpu telemetry worker panicked");
                (false, Vec::new(), Some("GPU telemetry worker failed".to_owned()))
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

    pub async fn diagnostics(&self) -> Result<SystemDiagnosticsSnapshot, AppCoreError> {
        let model_state = self.model_state.as_ref().ok_or_else(|| {
            AppCoreError::Internal("system diagnostics require app state".to_owned())
        })?;
        let config = model_state.config();
        let settings = model_state.pmid().config();

        let mut paths = vec![
            diagnostic_path("settings_file", &config.settings_path),
            diagnostic_path("model_config_dir", &config.model_config_dir),
            diagnostic_path("plugin_install_dir", &config.plugins_dir),
            diagnostic_path("session_state_dir", Path::new(&config.session_state_dir)),
            diagnostic_path("exec_rules_dir", &config.exec_rules_dir),
            diagnostic_path("logs_dir", &slab_utils::app_home::logs_dir()),
        ];
        if let Some(path) = config.settings_overlay_path.as_ref() {
            paths.push(diagnostic_path("settings_overlay_file", path));
        }
        if let Some(path) = config.log_file.as_ref() {
            paths.push(diagnostic_path("server_log_file", path));
        }
        if let Some(path) = settings.runtime.model_cache_dir.as_deref() {
            paths.push(diagnostic_path("model_cache_dir", Path::new(path)));
        }
        if let Some(path) = settings.setup.backends.dir.as_deref() {
            paths.push(diagnostic_path("setup_backend_dir", Path::new(path)));
        }
        if let Some(path) = settings.setup.ffmpeg.dir.as_deref() {
            paths.push(diagnostic_path("setup_ffmpeg_dir", Path::new(path)));
        }
        if let Some(path) = config.lib_dir.as_ref() {
            paths.push(diagnostic_path("runtime_lib_dir", path));
        }
        if let Some(path) = config.workspace_root.as_ref() {
            paths.push(diagnostic_path("workspace_root", path));
        }

        Ok(SystemDiagnosticsSnapshot {
            status: "ok".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            generated_at: Utc::now().to_rfc3339(),
            transport_mode: config.transport_mode.clone(),
            swagger_enabled: settings.server.swagger.enabled,
            admin_token_configured: settings
                .server
                .admin
                .token
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
            cloud_http_trace_enabled: settings.server.cloud_http_trace,
            cors_allowed_origins: (!settings.server.cors.allowed_origins.is_empty())
                .then(|| settings.server.cors.allowed_origins.join(",")),
            paths,
        })
    }

    /// Aggregate recent agent thread stats + failed tool calls for diagnostics
    /// (INFRA-08). Thread stats carry only whitelist-safe fields (no message
    /// content); failed tool calls carry tool name + error only (no arguments).
    /// The reason field is populated from `completion_text` for non-completed
    /// threads (where it stores the termination reason) and left `None` for
    /// completed threads (where it stores the final answer, not a reason).
    pub async fn agent_diagnostics(&self) -> Result<AgentDiagnosticsResponse, AppCoreError> {
        let model_state = self.model_state.as_ref().ok_or_else(|| {
            AppCoreError::Internal("agent diagnostics require app state".to_owned())
        })?;
        let store = model_state.store();
        const LIMIT: i64 = 50;

        let thread_rows = store.list_recent_agent_thread_stats(LIMIT).await?;
        let failed_rows = store.list_recent_failed_tool_calls(LIMIT).await?;

        let threads = thread_rows
            .into_iter()
            .map(|row| {
                let reason = if row.status != "completed" {
                    row.completion_text.filter(|value| !value.trim().is_empty())
                } else {
                    None
                };
                slab_utils::diagnostics::ThreadStat {
                    thread_id: row.id,
                    status: row.status,
                    turn_index: row.turn_index,
                    depth: row.depth,
                    reason,
                }
            })
            .map(Into::into)
            .collect();

        let failed_tool_calls = failed_rows
            .into_iter()
            .map(|row| slab_utils::diagnostics::FailedToolCall {
                tool_name: row.tool_name,
                error: row.output.unwrap_or_default(),
            })
            .map(Into::into)
            .collect();

        Ok(AgentDiagnosticsResponse { threads, failed_tool_calls })
    }
}

fn diagnostic_path(label: &str, path: &Path) -> SystemDiagnosticPath {
    SystemDiagnosticPath {
        label: label.to_owned(),
        path: path.display().to_string(),
        exists: path.exists(),
    }
}

#[cfg(feature = "gpu-telemetry")]
fn memory_usage_percent(used: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    ((used as f64) / (total as f64) * 100.0).clamp(0.0, 100.0)
}

#[cfg(feature = "gpu-telemetry")]
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

#[cfg(not(feature = "gpu-telemetry"))]
fn collect_gpu_devices() -> Result<Vec<GpuDeviceSnapshot>, String> {
    Err("GPU telemetry backend is disabled in this build".to_owned())
}
