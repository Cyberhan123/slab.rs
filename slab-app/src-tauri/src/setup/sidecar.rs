use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use dirs_next::config_dir;
use log::{error, info, warn};
use tauri::Manager;
use tauri::path::BaseDirectory;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};

pub struct SidecarState {
    child: Mutex<Option<CommandChild>>,
    terminated: Arc<AtomicBool>,
    shutdown_started: AtomicBool,
}

impl SidecarState {
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
            error!("sidecar shutdown signal failed: {e}");
        }

        let terminated = Arc::clone(&self.terminated);
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(8)).await;
            if terminated.load(Ordering::SeqCst) {
                return;
            }
            if let Err(e) = child.kill() {
                error!("sidecar force kill failed after timeout: {e}");
            }
        });
    }
}

pub fn shutdown_server_sidecar<R: tauri::Runtime>(app_handle: &tauri::AppHandle<R>) {
    if let Some(state) = app_handle.try_state::<SidecarState>() {
        state.trigger_shutdown();
    }
}

pub fn run_server_sidecar(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle();
    let lib_path = app.path().resolve("resources/libs", BaseDirectory::Resource)?;
    let lib_path_str = lib_path.to_str().ok_or("invalid lib path")?;
    let config_base_dir = config_dir().unwrap_or_else(|| PathBuf::from(".")).join("Slab");
    std::fs::create_dir_all(&config_base_dir)?;
    let app_log_dir = app.path().app_log_dir()?;
    std::fs::create_dir_all(&app_log_dir)?;
    let settings_path = config_base_dir.join("settings.json");
    let database_path = config_base_dir.join("slab.db");
    let server_log_path = app_log_dir.join("slab-server.log");
    let settings_path_str = settings_path.to_str().ok_or("invalid settings path")?;
    let database_url = sqlite_database_url(&database_path);
    let server_log_path_str = server_log_path.to_str().ok_or("invalid sidecar log path")?;

    let sidecar_command = app_handle.shell().sidecar("slab-server")?.args([
        "--gateway-bind",
        "127.0.0.1:3000",
        "--whisper-bind",
        "127.0.0.1:3001",
        "--llama-bind",
        "127.0.0.1:3002",
        "--runtime-transport",
        "ipc",
        "--lib-dir",
        lib_path_str,
        "--database-url",
        database_url.as_str(),
        "--settings-path",
        settings_path_str,
        "--log-file",
        server_log_path_str,
        "--shutdown-on-stdin-close",
    ]);

    let (mut rx, child) = sidecar_command.spawn()?;
    let terminated = Arc::new(AtomicBool::new(false));
    let terminated_for_events = Arc::clone(&terminated);
    let _ = app.manage(SidecarState::new(child, terminated));

    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    let msg = String::from_utf8_lossy(&line);
                    info!("sidecar stdout: {}", msg.trim_end());
                }
                CommandEvent::Stderr(line) => {
                    let msg = String::from_utf8_lossy(&line);
                    warn!("sidecar stderr: {}", msg.trim_end());
                }
                CommandEvent::Error(err) => {
                    error!("sidecar process error: {err}");
                }
                CommandEvent::Terminated(payload) => {
                    terminated_for_events.store(true, Ordering::SeqCst);
                    match payload.code {
                        Some(0) => {
                            info!(
                                "sidecar terminated: signal {:?} code {:?}",
                                payload.signal, payload.code
                            );
                        }
                        _ => {
                            warn!(
                                "sidecar terminated: signal {:?} code {:?}",
                                payload.signal, payload.code
                            );
                        }
                    }
                }
                other => {
                    info!("sidecar event: {:?}", other);
                }
            }
        }
    });

    info!(
        "tauri log persistence enabled: log_dir={} server_log={}",
        app_log_dir.display(),
        server_log_path.display()
    );
    info!("Slab sidecar started");
    Ok(())
}

fn sqlite_database_url(path: &std::path::Path) -> String {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(path)
    };
    let normalized = absolute.to_string_lossy().replace('\\', "/");
    let prefix = if normalized.starts_with('/') { "sqlite://" } else { "sqlite:///" };
    format!("{prefix}{normalized}?mode=rwc")
}
