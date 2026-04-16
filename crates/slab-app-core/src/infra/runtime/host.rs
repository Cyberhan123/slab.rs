use std::path::Path;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::config::Config;
use crate::domain::services::PmidService;
use crate::error::AppCoreError;
use crate::launch::{LaunchHostPaths, LaunchProfile, ResolvedLaunchSpec};
use slab_types::RuntimeBackendId;

use super::process::{TokioRuntimeSpawner, resolve_runtime_exe};
use super::supervisor::{
    ManagedRuntimeSupervisor, RuntimeSupervisorControlHandle, RuntimeSupervisorOptions,
    RuntimeSupervisorStatus,
};

#[derive(Debug, Clone, Default)]
pub struct ManagedRuntimeHostStartOptions {
    pub log_level: Option<String>,
    pub log_json: bool,
    pub supervisor_options: RuntimeSupervisorOptions,
}

struct ManagedRuntimeHostState {
    supervisor: Option<Arc<ManagedRuntimeSupervisor>>,
    startup_error: Option<String>,
}

pub struct ManagedRuntimeHost {
    launch_spec: ResolvedLaunchSpec,
    start_options: ManagedRuntimeHostStartOptions,
    state: Mutex<ManagedRuntimeHostState>,
    status: Arc<RuntimeSupervisorStatus>,
}

impl ManagedRuntimeHost {
    pub async fn start_server(
        gateway_cfg: &Config,
        options: ManagedRuntimeHostStartOptions,
    ) -> Result<Self, AppCoreError> {
        let runtime_log_dir_fallback = gateway_cfg
            .settings_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::temp_dir().join("Slab"))
            .join("logs");
        let runtime_ipc_dir_fallback = gateway_cfg
            .settings_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::temp_dir().join("Slab"))
            .join("ipc");

        let pmid = PmidService::load_from_path(gateway_cfg.settings_path.clone()).await?;
        let launch_spec = pmid
            .resolve_launch_spec(
                LaunchProfile::Server,
                &LaunchHostPaths {
                    runtime_lib_dir_fallback: gateway_cfg.lib_dir.clone(),
                    runtime_log_dir_fallback,
                    runtime_ipc_dir_fallback,
                    shutdown_on_stdin_close: true,
                },
            )
            .await?;

        let status = Arc::new(RuntimeSupervisorStatus::from_launch_spec(&launch_spec));
        let host = Self {
            launch_spec,
            start_options: options,
            state: Mutex::new(ManagedRuntimeHostState { supervisor: None, startup_error: None }),
            status,
        };

        let _ = host.ensure_supervisor_started().await;
        Ok(host)
    }

    pub fn launch_spec(&self) -> &ResolvedLaunchSpec {
        &self.launch_spec
    }

    pub fn apply_to_config(&self, config: &mut Config) {
        self.launch_spec.apply_to_config(config);
    }

    pub fn status_registry(&self) -> Arc<RuntimeSupervisorStatus> {
        Arc::clone(&self.status)
    }

    pub fn managed_backends(&self) -> Vec<RuntimeBackendId> {
        self.launch_spec.children.iter().map(|child| child.backend).collect()
    }

    pub async fn startup_error(&self) -> Option<String> {
        self.state.lock().await.startup_error.clone()
    }

    pub async fn restart_or_start_backends(
        &self,
        backends: &[RuntimeBackendId],
    ) -> Result<Vec<RuntimeBackendId>, AppCoreError> {
        let requested = self.filter_managed_backends(backends);
        if requested.is_empty() {
            return Ok(Vec::new());
        }

        let activation = self.ensure_supervisor_started().await?;
        match activation {
            SupervisorActivation::Started => Ok(requested),
            SupervisorActivation::Existing(control) => control.restart_backends(&requested),
        }
    }

    pub async fn shutdown(&self) {
        let supervisor = {
            let mut state = self.state.lock().await;
            state.startup_error = None;
            state.supervisor.take()
        };

        if let Some(supervisor) = supervisor {
            supervisor.shutdown().await;
        }
    }

    async fn ensure_supervisor_started(&self) -> Result<SupervisorActivation, AppCoreError> {
        let mut state = self.state.lock().await;

        if let Some(supervisor) = &state.supervisor {
            return Ok(SupervisorActivation::Existing(supervisor.control_handle()));
        }

        match start_runtime_supervisor(&self.launch_spec, &self.start_options).await {
            Ok(supervisor) => {
                let supervisor = Arc::new(supervisor);
                let control = supervisor.control_handle();
                state.supervisor = Some(supervisor);
                state.startup_error = None;
                let _ = control;
                Ok(SupervisorActivation::Started)
            }
            Err(error) => {
                mark_managed_backends_unavailable(&self.status, &self.launch_spec, &error);
                state.startup_error = Some(error.to_string());
                Err(error)
            }
        }
    }

    fn filter_managed_backends(&self, backends: &[RuntimeBackendId]) -> Vec<RuntimeBackendId> {
        backends
            .iter()
            .copied()
            .filter(|backend| {
                self.launch_spec.children.iter().any(|child| child.backend == *backend)
            })
            .collect()
    }
}

enum SupervisorActivation {
    Existing(RuntimeSupervisorControlHandle),
    Started,
}

async fn start_runtime_supervisor(
    launch_spec: &ResolvedLaunchSpec,
    options: &ManagedRuntimeHostStartOptions,
) -> Result<ManagedRuntimeSupervisor, AppCoreError> {
    launch_spec.prepare_filesystem()?;

    let server_exe = std::env::current_exe().map_err(|error| {
        AppCoreError::Internal(format!("failed to resolve current executable path: {error}"))
    })?;
    let runtime_exe = resolve_runtime_exe(&server_exe)?;

    ManagedRuntimeSupervisor::start(
        launch_spec.clone(),
        Arc::new(TokioRuntimeSpawner::new(
            runtime_exe,
            options.log_level.clone(),
            options.log_json,
        )),
        options.supervisor_options.clone(),
    )
    .await
}

fn mark_managed_backends_unavailable(
    status: &RuntimeSupervisorStatus,
    launch_spec: &ResolvedLaunchSpec,
    error: &AppCoreError,
) {
    let detail = format!("managed runtime startup failed: {error}");
    for child_spec in &launch_spec.children {
        status.mark_unavailable(child_spec.backend, detail.clone());
    }
}
