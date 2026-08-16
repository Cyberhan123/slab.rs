use std::path::Path;

use crate::context::ModelState;
use crate::domain::models::{GpuStatusSnapshot, SystemDiagnosticPath, SystemDiagnosticsSnapshot};
use crate::error::AppCoreError;
use crate::schemas::system::{AgentDiagnosticsResponse, GpuLedgerResponse};
use chrono::Utc;

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

    /// GPU telemetry via the process-wide scheduler (cache + periodic
    /// refresh). Bare `SystemService::new()` has no scheduler and reports
    /// telemetry as unavailable.
    pub async fn gpu_status(&self) -> GpuStatusSnapshot {
        let Some(model_state) = self.model_state.as_ref() else {
            return GpuStatusSnapshot {
                available: false,
                backend: "none".to_owned(),
                updated_at: Utc::now().to_rfc3339(),
                devices: Vec::new(),
                error: Some("GPU telemetry unavailable: scheduler not initialized".to_owned()),
            };
        };
        model_state.gpu_scheduler().gpu_status().await
    }

    /// Resident-model memory ledger (diagnostics-only): per-device gauge +
    /// resident entries with engine-resolved context lengths. Reads pure
    /// bookkeeping — no probe, no cache-freshness interaction. Empty when no
    /// app state is attached.
    pub async fn gpu_ledger(&self) -> GpuLedgerResponse {
        let Some(model_state) = self.model_state.as_ref() else {
            return GpuLedgerResponse { devices: Vec::new() };
        };
        GpuLedgerResponse {
            devices: model_state
                .gpu_scheduler()
                .ledger()
                .snapshot()
                .await
                .into_iter()
                .map(Into::into)
                .collect(),
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

    /// Aggregate recent agent thread stats for diagnostics (INFRA-08). Thread
    /// stats carry only whitelist-safe fields (no message content). The reason
    /// field is populated from `completion_text` for non-completed threads
    /// (where it stores the termination reason) and left `None` for completed
    /// threads (where it stores the final answer, not a reason).
    ///
    /// The `agent_tool_calls` audit table was dropped, so the
    /// failed-tool-call list is no longer populated here (the response field is
    /// retained for API stability and returned empty). Tool failures are now
    /// captured by the rollout `TurnItem` stream; a rollout-native diagnostics
    /// reader is not yet implemented.
    pub async fn agent_diagnostics(&self) -> Result<AgentDiagnosticsResponse, AppCoreError> {
        let model_state = self.model_state.as_ref().ok_or_else(|| {
            AppCoreError::Internal("agent diagnostics require app state".to_owned())
        })?;
        let store = model_state.store();
        const LIMIT: i64 = 50;

        let thread_rows = store.list_recent_agent_thread_stats(LIMIT).await?;

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

        // agent_tool_calls was dropped; failed-tool-call diagnostics
        // are not populated until a rollout-native reader exists.
        let failed_tool_calls = Vec::new();

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
