use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use log::{error, info, warn};
use tauri::Manager;
use tauri::path::BaseDirectory;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};

const SERVER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(8);

pub struct ServerSidecarState {
    child: Arc<Mutex<Option<CommandChild>>>,
}

impl ServerSidecarState {
    fn new(child: Arc<Mutex<Option<CommandChild>>>) -> Self {
        Self { child }
    }

    fn take_child(&self) -> Option<CommandChild> {
        self.child.lock().ok().and_then(|mut guard| guard.take())
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
    plugins_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let bundled_lib_dir = app.path().resolve("resources/libs", BaseDirectory::Resource)?;
    let bundled_plugin_market_path = app
        .path()
        .resolve("resources/plugins/dist/plugin-market.json", BaseDirectory::Resource)
        .ok()
        .filter(|path| path.is_file());
    let log_dir = app.path().app_log_dir()?;
    std::fs::create_dir_all(&log_dir)?;
    let log_file = log_dir.join("slab-server.log");
    let args = vec![
        "--shutdown-on-stdin-close".to_owned(),
        "--log-file".to_owned(),
        log_file.display().to_string(),
        "--lib-dir".to_owned(),
        bundled_lib_dir.display().to_string(),
    ];

    let mut command = app
        .shell()
        .sidecar("slab-server")
        .map_err(|error| {
            std::io::Error::other(format!("failed to resolve slab-server sidecar: {error}"))
        })?
        .args(args.clone())
        .env("SLAB_PLUGINS_DIR", plugins_dir.to_string_lossy().into_owned());

    if std::env::var_os("SLAB_PLUGIN_MARKET_URL").is_none()
        && let Some(path) = bundled_plugin_market_path
    {
        command = command.env("SLAB_PLUGIN_MARKET_URL", path.to_string_lossy().into_owned());
    }

    let (rx, child) = command
        .spawn()
        .map_err(|error| {
            std::io::Error::other(format!("failed to spawn slab-server sidecar: {error}"))
        })?;

    let child = Arc::new(Mutex::new(Some(child)));
    spawn_server_log_task(rx);

    let _ = app.manage(ServerSidecarState::new(Arc::clone(&child)));

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
