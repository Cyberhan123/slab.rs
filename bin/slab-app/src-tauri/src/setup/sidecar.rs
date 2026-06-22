use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use log::{error, info, warn};
use tauri::Manager;
use tauri::path::BaseDirectory;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};

const SERVER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Clone, Debug, Default)]
pub struct ServerSidecarConfig {
    pub database_url: Option<String>,
    pub settings_path: Option<PathBuf>,
    pub settings_overlay_path: Option<PathBuf>,
    pub workspace_root: Option<PathBuf>,
    pub model_config_dir: Option<PathBuf>,
    pub session_state_dir: Option<PathBuf>,
    pub plugins_dir: Option<PathBuf>,
}

#[derive(Default)]
pub struct ServerSidecarState {
    child: Arc<Mutex<Option<CommandChild>>>,
}

impl ServerSidecarState {
    fn take_child(&self) -> Option<CommandChild> {
        self.child.lock().ok().and_then(|mut guard| guard.take())
    }

    fn set_child(&self, child: CommandChild) -> Result<(), String> {
        let mut guard = self
            .child
            .lock()
            .map_err(|_| "failed to lock slab-server sidecar state".to_string())?;
        *guard = Some(child);
        Ok(())
    }
}

pub fn shutdown_server_sidecar<R: tauri::Runtime>(app_handle: &tauri::AppHandle<R>) {
    if let Some(state) = app_handle.try_state::<ServerSidecarState>()
        && let Some(mut child) = state.take_child()
    {
        tauri::async_runtime::spawn(async move {
            if let Err(error) = child.write(b"shutdown\n") {
                warn!("failed to request slab-server shutdown over stdin: {error}");
            }

            tokio::time::sleep(SERVER_SHUTDOWN_TIMEOUT).await;
            let _ = child.kill();
        });
    }
}

pub fn run_server_sidecar(
    app: &mut tauri::App,
    config: ServerSidecarConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let _ = app.manage(ServerSidecarState::default());
    start_server_sidecar(app.handle(), config).map_err(std::io::Error::other)?;
    Ok(())
}

fn start_server_sidecar<R: tauri::Runtime>(
    app_handle: &tauri::AppHandle<R>,
    config: ServerSidecarConfig,
) -> Result<(), String> {
    let bundled_lib_dir = app_handle
        .path()
        .resolve("resources/libs", BaseDirectory::Resource)
        .map_err(|error| format!("failed to resolve bundled runtime libraries: {error}"))?;
    let log_file = slab_utils::app_home::server_log_file();
    if let Some(log_dir) = log_file.parent() {
        std::fs::create_dir_all(log_dir).map_err(|error| {
            format!("failed to create app log directory {}: {error}", log_dir.display())
        })?;
    }
    let mut args = vec![
        "--shutdown-on-stdin-close".to_owned(),
        "--log-file".to_owned(),
        log_file.display().to_string(),
        "--lib-dir".to_owned(),
        bundled_lib_dir.display().to_string(),
    ];
    if let Some(database_url) = &config.database_url {
        args.push("--database-url".to_owned());
        args.push(database_url.clone());
    }
    if let Some(settings_path) = &config.settings_path {
        args.push("--settings-path".to_owned());
        args.push(settings_path.display().to_string());
    }
    if let Some(settings_overlay_path) = &config.settings_overlay_path {
        args.push("--settings-overlay-path".to_owned());
        args.push(settings_overlay_path.display().to_string());
    }
    if let Some(workspace_root) = &config.workspace_root {
        args.push("--workspace-root".to_owned());
        args.push(workspace_root.display().to_string());
    }
    if let Some(model_config_dir) = &config.model_config_dir {
        args.push("--model-config-dir".to_owned());
        args.push(model_config_dir.display().to_string());
    }
    if let Some(session_state_dir) = &config.session_state_dir {
        args.push("--session-state-dir".to_owned());
        args.push(session_state_dir.display().to_string());
    }
    if let Some(plugins_dir) = &config.plugins_dir {
        args.push("--plugins-dir".to_owned());
        args.push(plugins_dir.display().to_string());
    }

    let command = app_handle
        .shell()
        .sidecar("slab-server")
        .map_err(|error| format!("failed to resolve slab-server sidecar: {error}"))?
        .args(args.clone());

    let (rx, child) =
        command.spawn().map_err(|error| format!("failed to spawn slab-server sidecar: {error}"))?;

    spawn_server_log_task(rx);

    app_handle
        .state::<ServerSidecarState>()
        .set_child(child)
        .map_err(|error| format!("failed to store slab-server sidecar state: {error}"))?;

    info!("slab-server sidecar spawned (log_file={}, args={args:?})", log_file.display());

    Ok(())
}

fn spawn_server_log_task(mut rx: tauri::async_runtime::Receiver<CommandEvent>) {
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    let message = String::from_utf8_lossy(&line);
                    info!("slab-server stdout: {}", message.trim_end());
                }
                CommandEvent::Stderr(line) => {
                    let message = String::from_utf8_lossy(&line);
                    warn!("slab-server stderr: {}", message.trim_end());
                }
                CommandEvent::Error(error) => {
                    error!("slab-server process error: {error}");
                }
                CommandEvent::Terminated(payload) => {
                    let exit_message =
                        format!("code={:?} signal={:?}", payload.code, payload.signal);

                    match payload.code {
                        Some(0) => info!("slab-server terminated cleanly ({exit_message})"),
                        _ => warn!("slab-server terminated unexpectedly ({exit_message})"),
                    }
                }
                other => {
                    info!("slab-server event: {other:?}");
                }
            }
        }
    });
}
