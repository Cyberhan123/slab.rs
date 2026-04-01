use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use log::{error, info, warn};
use tauri::Manager;
use tauri::path::BaseDirectory;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};

/// The TCP address at which the embedded `slab-runtime` gRPC server listens.
pub const RUNTIME_GRPC_BIND: &str = "127.0.0.1:50051";

pub struct RuntimeSidecarState {
    child: Mutex<Option<CommandChild>>,
    terminated: Arc<AtomicBool>,
    shutdown_started: AtomicBool,
}

impl RuntimeSidecarState {
    fn new(child: CommandChild, terminated: Arc<AtomicBool>) -> Self {
        Self {
            child: Mutex::new(Some(child)),
            terminated,
            shutdown_started: AtomicBool::new(false),
        }
    }

    fn trigger_shutdown(&self) {
        if self.shutdown_started.swap(true, Ordering::SeqCst) {
            return;
        }

        let Some(mut child) = self.child.lock().ok().and_then(|mut guard| guard.take()) else {
            return;
        };

        if let Err(e) = child.write(b"shutdown\n") {
            error!("runtime sidecar shutdown signal failed: {e}");
        }

        let terminated = Arc::clone(&self.terminated);
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(8)).await;
            if terminated.load(Ordering::SeqCst) {
                return;
            }
            if let Err(e) = child.kill() {
                error!("runtime sidecar force kill failed after timeout: {e}");
            }
        });
    }
}

pub fn shutdown_runtime_sidecar<R: tauri::Runtime>(app_handle: &tauri::AppHandle<R>) {
    if let Some(state) = app_handle.try_state::<RuntimeSidecarState>() {
        state.trigger_shutdown();
    }
}

/// Launch the embedded `slab-runtime` sidecar with all backends enabled on a
/// single gRPC endpoint ([`RUNTIME_GRPC_BIND`]).
pub fn run_runtime_sidecar(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle();
    let lib_path = app.path().resolve("resources/libs", BaseDirectory::Resource)?;
    let lib_path_str = lib_path.to_str().ok_or("invalid lib path")?;
    let app_log_dir = app.path().app_log_dir()?;
    std::fs::create_dir_all(&app_log_dir)?;
    let runtime_log_path = app_log_dir.join("slab-runtime.log");
    let runtime_log_path_str = runtime_log_path.to_str().ok_or("invalid runtime log path")?;

    let sidecar_command = app_handle.shell().sidecar("slab-runtime")?.args([
        "--grpc-bind",
        RUNTIME_GRPC_BIND,
        "--enabled-backends",
        "llama,whisper,diffusion",
        "--lib-dir",
        lib_path_str,
        "--log-file",
        runtime_log_path_str,
        "--shutdown-on-stdin-close",
    ]);

    let (mut rx, child) = sidecar_command.spawn()?;
    let terminated = Arc::new(AtomicBool::new(false));
    let terminated_for_events = Arc::clone(&terminated);
    let _ = app.manage(RuntimeSidecarState::new(child, terminated));

    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    let msg = String::from_utf8_lossy(&line);
                    info!("runtime stdout: {}", msg.trim_end());
                }
                CommandEvent::Stderr(line) => {
                    let msg = String::from_utf8_lossy(&line);
                    warn!("runtime stderr: {}", msg.trim_end());
                }
                CommandEvent::Error(err) => {
                    error!("runtime process error: {err}");
                }
                CommandEvent::Terminated(payload) => {
                    terminated_for_events.store(true, Ordering::SeqCst);
                    match payload.code {
                        Some(0) => {
                            info!(
                                "runtime terminated: signal {:?} code {:?}",
                                payload.signal, payload.code
                            );
                        }
                        _ => {
                            warn!(
                                "runtime terminated: signal {:?} code {:?}",
                                payload.signal, payload.code
                            );
                        }
                    }
                }
                other => {
                    info!("runtime event: {:?}", other);
                }
            }
        }
    });

    info!(
        "tauri log persistence enabled: log_dir={} runtime_log={}",
        app_log_dir.display(),
        runtime_log_path.display()
    );
    info!(
        "slab-runtime sidecar started (grpc_bind={})",
        RUNTIME_GRPC_BIND
    );
    Ok(())
}

// Keep for backwards compatibility with any code that resolves database paths.
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
