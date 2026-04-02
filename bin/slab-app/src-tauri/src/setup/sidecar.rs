use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use log::{error, info, warn};
use slab_app_core::config::Config;
use slab_app_core::domain::services::PmidService;
use slab_app_core::launch::{
    LaunchHostPaths, LaunchProfile, ResolvedLaunchSpec, ResolvedRuntimeChildSpec,
};
use tauri::Manager;
use tauri::path::BaseDirectory;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};

struct ManagedRuntimeChild {
    backend: String,
    bind_address: String,
    child: CommandChild,
    terminated: Arc<AtomicBool>,
}

pub struct RuntimeSidecarState {
    children: Mutex<Vec<ManagedRuntimeChild>>,
    shutdown_started: AtomicBool,
}

impl RuntimeSidecarState {
    fn new(children: Vec<ManagedRuntimeChild>) -> Self {
        Self { children: Mutex::new(children), shutdown_started: AtomicBool::new(false) }
    }

    fn trigger_shutdown(&self) {
        if self.shutdown_started.swap(true, Ordering::SeqCst) {
            return;
        }

        let Some(children) = self.children.lock().ok().map(|mut guard| std::mem::take(&mut *guard))
        else {
            return;
        };

        if children.is_empty() {
            return;
        }

        shutdown_managed_children(children);
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
) -> Result<ResolvedLaunchSpec, Box<dyn std::error::Error>> {
    let cfg = Config::from_env();
    let app_handle = app.handle();
    let lib_path = app.path().resolve("resources/libs", BaseDirectory::Resource)?;
    let app_log_dir = app.path().app_log_dir()?;
    std::fs::create_dir_all(&app_log_dir)?;
    let runtime_log_dir_fallback = app_log_dir.join("runtime");
    let runtime_ipc_dir_fallback = app_log_dir.join("ipc");
    let launch_spec = tauri::async_runtime::block_on(async {
        let pmid = PmidService::load_from_path(cfg.settings_path.clone()).await?;
        let launch_spec = pmid.resolve_launch_spec(
            LaunchProfile::Desktop,
            &LaunchHostPaths {
                runtime_lib_dir_fallback: Some(lib_path),
                runtime_log_dir_fallback,
                runtime_ipc_dir_fallback,
                shutdown_on_stdin_close: true,
            },
        ).await?;
        launch_spec.prepare_filesystem()?;
        Ok::<_, Box<dyn std::error::Error>>(launch_spec)
    })?;

    let mut children = Vec::new();
    for child_spec in &launch_spec.children {
        match spawn_runtime_child(
            app_handle,
            child_spec,
            Some(cfg.log_level.as_str()),
            cfg.log_json,
        ) {
            Ok(child) => children.push(child),
            Err(error) => {
                shutdown_managed_children(children);
                return Err(error);
            }
        }
    }

    let _ = app.manage(RuntimeSidecarState::new(children));

    info!(
        "tauri log persistence enabled: log_dir={} runtime_log_dir={}",
        app_log_dir.display(),
        launch_spec.runtime_log_dir.display()
    );
    info!(
        "slab-runtime supervisor started (transport={}, children={})",
        launch_spec.transport.as_str(),
        launch_spec.children.len()
    );
    Ok(launch_spec)
}

fn spawn_runtime_child<R: tauri::Runtime>(
    app_handle: &tauri::AppHandle<R>,
    child_spec: &ResolvedRuntimeChildSpec,
    log_level: Option<&str>,
    log_json: bool,
) -> Result<ManagedRuntimeChild, Box<dyn std::error::Error>> {
    let args = child_spec.command_args(log_level, log_json);
    let (mut rx, child) = app_handle.shell().sidecar("slab-runtime")?.args(args).spawn()?;
    let terminated = Arc::new(AtomicBool::new(false));
    let terminated_for_events = Arc::clone(&terminated);
    let backend = child_spec.backend.canonical_id().to_owned();
    let bind_address = child_spec.grpc_bind_address.clone();
    let backend_for_events = backend.clone();
    let bind_for_events = bind_address.clone();

    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    let msg = String::from_utf8_lossy(&line);
                    info!(
                        "runtime stdout [{} {}]: {}",
                        backend_for_events,
                        bind_for_events,
                        msg.trim_end()
                    );
                }
                CommandEvent::Stderr(line) => {
                    let msg = String::from_utf8_lossy(&line);
                    warn!(
                        "runtime stderr [{} {}]: {}",
                        backend_for_events,
                        bind_for_events,
                        msg.trim_end()
                    );
                }
                CommandEvent::Error(err) => {
                    error!(
                        "runtime process error [{} {}]: {}",
                        backend_for_events, bind_for_events, err
                    );
                }
                CommandEvent::Terminated(payload) => {
                    terminated_for_events.store(true, Ordering::SeqCst);
                    match payload.code {
                        Some(0) => {
                            info!(
                                "runtime terminated [{} {}]: signal {:?} code {:?}",
                                backend_for_events, bind_for_events, payload.signal, payload.code
                            );
                        }
                        _ => {
                            warn!(
                                "runtime terminated [{} {}]: signal {:?} code {:?}",
                                backend_for_events, bind_for_events, payload.signal, payload.code
                            );
                        }
                    }
                }
                other => {
                    info!(
                        "runtime event [{} {}]: {:?}",
                        backend_for_events, bind_for_events, other
                    );
                }
            }
        }
    });

    info!(
        "spawned slab-runtime child [{}] on {}",
        child_spec.backend.canonical_id(),
        child_spec.grpc_bind_address
    );

    Ok(ManagedRuntimeChild { backend, bind_address, child, terminated })
}

fn shutdown_managed_children(children: Vec<ManagedRuntimeChild>) {
    for mut managed in children {
        let backend = managed.backend.clone();
        let bind_address = managed.bind_address.clone();

        // Tauri shell does not expose an explicit stdin-close API, so desktop
        // falls back to the runtime's legacy text command before a timed kill.
        if let Err(error) = managed.child.write(b"shutdown\n") {
            error!(
                "runtime sidecar shutdown signal failed [{} {}]: {}",
                backend, bind_address, error
            );
        } else {
            info!(
                "requested runtime shutdown [{} {}] via compatibility command",
                backend, bind_address
            );
        }

        let terminated = Arc::clone(&managed.terminated);
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(8)).await;
            if terminated.load(Ordering::SeqCst) {
                return;
            }
            if let Err(error) = managed.child.kill() {
                error!(
                    "runtime sidecar force kill failed after timeout [{} {}]: {}",
                    backend, bind_address, error
                );
            } else {
                warn!("runtime sidecar force killed after timeout [{} {}]", backend, bind_address);
            }
        });
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
