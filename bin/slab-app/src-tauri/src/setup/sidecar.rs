use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use log::{error, info, warn};
use slab_app_core::config::{Config, default_app_dir, default_runtime_ipc_dir, default_runtime_log_dir};
use slab_app_core::domain::services::PmidService;
use slab_app_core::launch::{LaunchHostPaths, LaunchProfile};
use slab_app_core::runtime_supervisor::{
    ManagedRuntimeSupervisor, RuntimeChildExit, RuntimeChildHandle, RuntimeChildSpawner,
    RuntimeSupervisorOptions,
};
use tauri::Manager;
use tauri::path::BaseDirectory;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tokio::sync::oneshot;

pub struct RuntimeSidecarState {
    supervisor: Arc<ManagedRuntimeSupervisor>,
}

impl RuntimeSidecarState {
    fn new(supervisor: Arc<ManagedRuntimeSupervisor>) -> Self {
        Self { supervisor }
    }

    fn trigger_shutdown(&self) {
        self.supervisor.trigger_shutdown();
        let supervisor = Arc::clone(&self.supervisor);
        tauri::async_runtime::spawn(async move {
            supervisor.shutdown().await;
        });
    }
}

pub fn shutdown_runtime_sidecar<R: tauri::Runtime>(app_handle: &tauri::AppHandle<R>) {
    if let Some(state) = app_handle.try_state::<RuntimeSidecarState>() {
        state.trigger_shutdown();
    }
}

/// Launch the embedded `slab-runtime` supervisor using the shared desktop
/// launch profile from `slab-app-core`.
pub fn run_runtime_sidecar(
    app: &mut tauri::App,
) -> Result<Arc<ManagedRuntimeSupervisor>, Box<dyn std::error::Error>> {
    let cfg = Config::from_env();
    let app_handle = app.handle().clone();
    let lib_path = app.path().resolve("resources/libs", BaseDirectory::Resource)?;
    let tauri_log_dir = app.path().app_log_dir()?;
    let app_data_dir = default_app_dir();
    std::fs::create_dir_all(&tauri_log_dir)?;
    std::fs::create_dir_all(&app_data_dir)?;
    let runtime_log_dir_fallback = default_runtime_log_dir();
    let runtime_ipc_dir_fallback = default_runtime_ipc_dir();
    let log_level = Some(cfg.log_level.clone());
    let log_json = cfg.log_json;
    let app_handle_for_spawn = app_handle.clone();

    let supervisor = tauri::async_runtime::block_on(async move {
        let pmid = PmidService::load_from_path(cfg.settings_path.clone()).await?;
        let launch_spec = pmid
            .resolve_launch_spec(
                LaunchProfile::Desktop,
                &LaunchHostPaths {
                    runtime_lib_dir_fallback: Some(lib_path),
                    runtime_log_dir_fallback,
                    runtime_ipc_dir_fallback,
                    shutdown_on_stdin_close: true,
                },
            )
            .await?;
        launch_spec.prepare_filesystem()?;

        let supervisor = Arc::new(
            ManagedRuntimeSupervisor::start(
                launch_spec,
                Arc::new(TauriRuntimeSpawner::new(app_handle_for_spawn, log_level, log_json)),
                RuntimeSupervisorOptions {
                    graceful_shutdown_timeout: Duration::from_secs(8),
                    force_shutdown_timeout: Duration::from_secs(8),
                    ..RuntimeSupervisorOptions::default()
                },
            )
            .await?,
        );

        Ok::<_, Box<dyn std::error::Error>>(supervisor)
    })?;

    let _ = app.manage(RuntimeSidecarState::new(Arc::clone(&supervisor)));

    info!(
        "tauri log persistence enabled: tauri_log_dir={} app_data_dir={} runtime_log_dir={}",
        tauri_log_dir.display(),
        app_data_dir.display(),
        supervisor.launch_spec().runtime_log_dir.display()
    );
    info!(
        "slab-runtime supervisor started (transport={}, children={})",
        supervisor.launch_spec().transport.as_str(),
        supervisor.launch_spec().children.len()
    );

    Ok(supervisor)
}

type MainThreadClosure = Box<dyn FnOnce() + Send + Sync + 'static>;
type RunOnMainThread =
    Arc<dyn Fn(MainThreadClosure) -> Result<(), tauri::Error> + Send + Sync + 'static>;
type SpawnRuntimeSidecar = Arc<
    dyn Fn(Vec<String>) -> Result<SpawnedRuntimeSidecar, slab_app_core::error::AppCoreError>
        + Send
        + Sync
        + 'static,
>;

struct SpawnedRuntimeSidecar {
    rx: tauri::async_runtime::Receiver<CommandEvent>,
    child: CommandChild,
}

struct TauriRuntimeSpawner {
    run_on_main_thread: RunOnMainThread,
    spawn_sidecar: SpawnRuntimeSidecar,
    log_level: Option<String>,
    log_json: bool,
}

impl TauriRuntimeSpawner {
    fn new<R: tauri::Runtime>(
        app_handle: tauri::AppHandle<R>,
        log_level: Option<String>,
        log_json: bool,
    ) -> Self {
        let app_for_main_thread = app_handle.clone();
        let run_on_main_thread: RunOnMainThread =
            Arc::new(move |f| app_for_main_thread.run_on_main_thread(f));

        let app_for_spawn = app_handle.clone();
        let spawn_sidecar: SpawnRuntimeSidecar = Arc::new(move |args| {
            let (rx, child) = app_for_spawn
                .shell()
                .sidecar("slab-runtime")
                .map_err(|error| {
                    slab_app_core::error::AppCoreError::Internal(format!(
                        "failed to resolve slab-runtime sidecar: {error}"
                    ))
                })?
                .args(args)
                .spawn()
                .map_err(|error| {
                    slab_app_core::error::AppCoreError::Internal(format!(
                        "failed to spawn slab-runtime sidecar: {error}"
                    ))
                })?;
            Ok(SpawnedRuntimeSidecar { rx, child })
        });

        Self { run_on_main_thread, spawn_sidecar, log_level, log_json }
    }
}

struct TauriRuntimeChildHandle {
    child: Option<CommandChild>,
    exit_rx: Option<oneshot::Receiver<RuntimeChildExit>>,
}

#[async_trait]
impl RuntimeChildHandle for TauriRuntimeChildHandle {
    async fn wait_for_exit(
        &mut self,
    ) -> Result<RuntimeChildExit, slab_app_core::error::AppCoreError> {
        let exit = self
            .exit_rx
            .as_mut()
            .ok_or_else(|| {
                slab_app_core::error::AppCoreError::Internal(
                    "runtime exit receiver missing for Tauri sidecar".to_owned(),
                )
            })?
            .await
            .map_err(|_| {
                slab_app_core::error::AppCoreError::Internal(
                    "runtime exit receiver dropped for Tauri sidecar".to_owned(),
                )
            })?;
        self.exit_rx = None;
        Ok(exit)
    }

    async fn request_graceful_shutdown(
        &mut self,
    ) -> Result<(), slab_app_core::error::AppCoreError> {
        self.child
            .as_mut()
            .ok_or_else(|| {
                slab_app_core::error::AppCoreError::Internal(
                    "runtime sidecar child handle missing during graceful shutdown".to_owned(),
                )
            })?
            .write(b"shutdown\n")
            .map_err(|error| {
                slab_app_core::error::AppCoreError::Internal(format!(
                    "runtime sidecar shutdown signal failed: {error}"
                ))
            })
    }

    async fn force_kill(&mut self) -> Result<(), slab_app_core::error::AppCoreError> {
        self.child
            .take()
            .ok_or_else(|| {
                slab_app_core::error::AppCoreError::Internal(
                    "runtime sidecar child handle missing during force kill".to_owned(),
                )
            })?
            .kill()
            .map_err(|error| {
                slab_app_core::error::AppCoreError::Internal(format!(
                    "runtime sidecar force kill failed: {error}"
                ))
            })
    }
}

#[async_trait]
impl RuntimeChildSpawner for TauriRuntimeSpawner {
    async fn spawn_child(
        &self,
        child_spec: &slab_app_core::launch::ResolvedRuntimeChildSpec,
    ) -> Result<Box<dyn RuntimeChildHandle>, slab_app_core::error::AppCoreError> {
        let args = child_spec.command_args(self.log_level.as_deref(), self.log_json);
        let log_file = child_spec.log_file.display().to_string();
        info!(
            "spawning slab-runtime child [{} {}] transport={} queue_capacity={} backend_capacity={} shutdown_on_stdin_close={} log_file={} args={:?}",
            child_spec.backend.canonical_id(),
            child_spec.grpc_bind_address,
            child_spec.transport.as_str(),
            child_spec.queue_capacity,
            child_spec.backend_capacity,
            child_spec.shutdown_on_stdin_close,
            log_file,
            args
        );
        let run_on_main_thread = Arc::clone(&self.run_on_main_thread);
        let spawn_sidecar = Arc::clone(&self.spawn_sidecar);
        let (spawn_tx, spawn_rx) = oneshot::channel();
        run_on_main_thread(Box::new(move || {
            let result = spawn_sidecar(args);
            let _ = spawn_tx.send(result);
        }))
        .map_err(|error| {
            slab_app_core::error::AppCoreError::Internal(format!(
                "failed to schedule slab-runtime spawn on main thread: {error}"
            ))
        })?;

        let SpawnedRuntimeSidecar { mut rx, child } = spawn_rx.await.map_err(|_| {
            slab_app_core::error::AppCoreError::Internal(
                "main-thread slab-runtime spawn response dropped unexpectedly".to_owned(),
            )
        })??;

        let backend = child_spec.backend.canonical_id().to_owned();
        let bind_address = child_spec.grpc_bind_address.clone();
        let log_file_for_events = log_file.clone();
        let backend_for_events = backend.clone();
        let bind_for_events = bind_address.clone();
        let (exit_tx, exit_rx) = oneshot::channel();

        tauri::async_runtime::spawn(async move {
            let mut exit_tx = Some(exit_tx);
            while let Some(event) = rx.recv().await {
                match event {
                    CommandEvent::Stdout(line) => {
                        let msg = String::from_utf8_lossy(&line);
                        info!(
                            "runtime stdout [{} {} {}]: {}",
                            backend_for_events,
                            bind_for_events,
                            log_file_for_events,
                            msg.trim_end()
                        );
                    }
                    CommandEvent::Stderr(line) => {
                        let msg = String::from_utf8_lossy(&line);
                        warn!(
                            "runtime stderr [{} {} {}]: {}",
                            backend_for_events,
                            bind_for_events,
                            log_file_for_events,
                            msg.trim_end()
                        );
                    }
                    CommandEvent::Error(err) => {
                        error!(
                            "runtime process error [{} {} {}]: {}",
                            backend_for_events, bind_for_events, log_file_for_events, err
                        );
                    }
                    CommandEvent::Terminated(payload) => {
                        match payload.code {
                            Some(0) => {
                                info!(
                                    "runtime terminated [{} {} {}]: signal {:?} code {:?}",
                                    backend_for_events,
                                    bind_for_events,
                                    log_file_for_events,
                                    payload.signal,
                                    payload.code
                                );
                            }
                            _ => {
                                warn!(
                                    "runtime terminated [{} {} {}]: signal {:?} code {:?}",
                                    backend_for_events,
                                    bind_for_events,
                                    log_file_for_events,
                                    payload.signal,
                                    payload.code
                                );
                            }
                        }

                        if let Some(sender) = exit_tx.take() {
                            let _ = sender.send(RuntimeChildExit {
                                code: payload.code,
                                signal: payload.signal,
                                message: None,
                            });
                        }
                    }
                    other => {
                        info!(
                            "runtime event [{} {} {}]: {:?}",
                            backend_for_events, bind_for_events, log_file_for_events, other
                        );
                    }
                }
            }

            if let Some(sender) = exit_tx.take() {
                warn!(
                    "runtime event stream closed [{} {} {}]",
                    backend_for_events, bind_for_events, log_file_for_events
                );
                let _ = sender.send(RuntimeChildExit {
                    code: None,
                    signal: None,
                    message: Some("runtime event stream closed".to_owned()),
                });
            }
        });

        info!(
            "spawned slab-runtime child [{}] on {} with log_file={}",
            child_spec.backend.canonical_id(),
            child_spec.grpc_bind_address,
            child_spec.log_file.display()
        );

        Ok(Box::new(TauriRuntimeChildHandle { child: Some(child), exit_rx: Some(exit_rx) }))
    }
}

// Keep for backwards compatibility with any code that resolves database paths.
#[allow(dead_code)]
pub fn sqlite_database_url(path: &std::path::Path) -> String {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(path)
    };
    let normalized = absolute.to_string_lossy().replace('\\', "/");
    let prefix = if normalized.starts_with('/') { "sqlite://" } else { "sqlite:///" };
    format!("{prefix}{normalized}?mode=rwc")
}
