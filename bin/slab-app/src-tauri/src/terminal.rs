use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path as AxumPath, State};
use axum::response::IntoResponse;
use axum::routing::get;
use futures::{SinkExt, StreamExt};
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use tauri::Manager;
use tauri::State as TauriState;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::workspace::{WorkspaceState, active_workspace};

const DEFAULT_COLS: u16 = 100;
const DEFAULT_ROWS: u16 = 24;
const WORKSPACE_TERMINAL_ROUTE: &str = "/workspace-terminal";

#[derive(Clone)]
struct TerminalServerInner {
    sessions: Arc<Mutex<HashMap<String, TerminalSessionRequest>>>,
}

impl TerminalServerInner {
    fn insert_session(&self, root_path: PathBuf) -> Result<String, String> {
        let id = Uuid::new_v4().to_string();
        let mut sessions =
            self.sessions.lock().map_err(|_| "failed to lock terminal sessions".to_string())?;
        sessions.insert(id.clone(), TerminalSessionRequest { root_path });
        Ok(id)
    }

    fn take_session(&self, id: &str) -> Option<TerminalSessionRequest> {
        self.sessions.lock().ok().and_then(|mut sessions| sessions.remove(id))
    }
}

#[derive(Clone)]
struct TerminalSessionRequest {
    root_path: PathBuf,
}

pub struct WorkspaceTerminalState {
    endpoint_origin: String,
    inner: TerminalServerInner,
}

impl WorkspaceTerminalState {
    fn create_session(&self, root_path: PathBuf) -> Result<WorkspaceTerminalSession, String> {
        let session_id = self.inner.insert_session(root_path)?;
        Ok(WorkspaceTerminalSession {
            url: format!("{}{}/{}", self.endpoint_origin, WORKSPACE_TERMINAL_ROUTE, session_id),
        })
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTerminalSession {
    pub url: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum TerminalClientMessage {
    Input { data: String },
    Resize { cols: u16, rows: u16 },
}

#[tauri::command]
pub fn workspace_terminal_session(
    workspace_state: TauriState<'_, WorkspaceState>,
    terminal_state: TauriState<'_, WorkspaceTerminalState>,
) -> Result<WorkspaceTerminalSession, String> {
    let workspace = active_workspace(&workspace_state)?;
    terminal_state.create_session(PathBuf::from(workspace.root_path))
}

pub fn init<R: tauri::Runtime>(app: &mut tauri::App<R>) -> Result<(), String> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("failed to bind workspace terminal server: {error}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("failed to configure workspace terminal server: {error}"))?;
    let local_addr = listener
        .local_addr()
        .map_err(|error| format!("failed to read workspace terminal server address: {error}"))?;
    let inner = TerminalServerInner { sessions: Arc::new(Mutex::new(HashMap::new())) };
    app.manage(WorkspaceTerminalState {
        endpoint_origin: format!("ws://{}", local_addr),
        inner: inner.clone(),
    });

    tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(error) => {
                log::error!("failed to start workspace terminal server listener: {error}");
                return;
            }
        };
        let router = Router::new()
            .route(&format!("{WORKSPACE_TERMINAL_ROUTE}/{{session_id}}"), get(upgrade_terminal))
            .with_state(inner);
        if let Err(error) = axum::serve(listener, router).await {
            log::error!("workspace terminal server stopped: {error}");
        }
    });

    log::info!("workspace terminal server listening on {local_addr}");
    Ok(())
}

async fn upgrade_terminal(
    State(state): State<TerminalServerInner>,
    AxumPath(session_id): AxumPath<String>,
    upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    upgrade.on_upgrade(move |socket| handle_terminal_socket(state, session_id, socket))
}

async fn handle_terminal_socket(state: TerminalServerInner, session_id: String, socket: WebSocket) {
    let Some(session) = state.take_session(&session_id) else {
        log::warn!("workspace terminal rejected unknown session {session_id}");
        return;
    };

    if let Err(error) = run_terminal_session(session.root_path, socket).await {
        log::warn!("workspace terminal session ended: {error}");
    }
}

async fn run_terminal_session(root_path: PathBuf, socket: WebSocket) -> Result<(), String> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: DEFAULT_ROWS,
            cols: DEFAULT_COLS,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("failed to open workspace terminal pty: {error}"))?;
    let mut command = shell_command();
    command.cwd(terminal_cwd(&root_path));
    configure_prompt(&mut command);
    let mut child = pair
        .slave
        .spawn_command(command)
        .map_err(|error| format!("failed to spawn workspace shell: {error}"))?;
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| format!("failed to open terminal reader: {error}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|error| format!("failed to open terminal writer: {error}"))?;
    let master = Arc::new(Mutex::new(pair.master));
    let writer = Arc::new(Mutex::new(writer));
    let (output_tx, mut output_rx) = mpsc::unbounded_channel();
    spawn_pty_reader(reader, output_tx);

    let (mut socket_sender, mut socket_receiver) = socket.split();
    loop {
        tokio::select! {
            output = output_rx.recv() => {
                let Some(output) = output else {
                    break;
                };
                if socket_sender.send(Message::Binary(output.into())).await.is_err() {
                    break;
                }
            }
            message = socket_receiver.next() => {
                match message {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(message) = serde_json::from_str::<TerminalClientMessage>(text.as_str()) {
                            handle_client_message(message, Arc::clone(&master), Arc::clone(&writer)).await;
                        }
                    }
                    Some(Ok(Message::Binary(data))) => {
                        write_pty(Arc::clone(&writer), data.to_vec()).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(error)) => {
                        log::warn!("workspace terminal websocket error: {error}");
                        break;
                    }
                }
            }
        }
    }

    let _ = child.kill();
    Ok(())
}

async fn handle_client_message(
    message: TerminalClientMessage,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
) {
    match message {
        TerminalClientMessage::Input { data } => {
            write_pty(writer, data.into_bytes()).await;
        }
        TerminalClientMessage::Resize { cols, rows } => {
            let cols = cols.max(1);
            let rows = rows.max(1);
            let result = tokio::task::spawn_blocking(move || {
                let master = master.lock().map_err(|_| "failed to lock terminal pty")?;
                master
                    .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
                    .map_err(|error| error.to_string())
            })
            .await;
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => log::warn!("workspace terminal resize failed: {error}"),
                Err(error) => log::warn!("workspace terminal resize task failed: {error}"),
            }
        }
    }
}

async fn write_pty(writer: Arc<Mutex<Box<dyn Write + Send>>>, data: Vec<u8>) {
    let result = tokio::task::spawn_blocking(move || {
        let mut writer = writer.lock().map_err(|_| "failed to lock terminal writer")?;
        writer.write_all(&data).map_err(|error| error.to_string())?;
        writer.flush().map_err(|error| error.to_string())
    })
    .await;
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => log::warn!("workspace terminal input failed: {error}"),
        Err(error) => log::warn!("workspace terminal input task failed: {error}"),
    }
}

fn spawn_pty_reader(mut reader: Box<dyn Read + Send>, output_tx: mpsc::UnboundedSender<Vec<u8>>) {
    std::thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(size) => {
                    if output_tx.send(buffer[..size].to_vec()).is_err() {
                        break;
                    }
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
    });
}

#[cfg(windows)]
fn shell_command() -> CommandBuilder {
    let mut command = CommandBuilder::new("powershell.exe");
    command.arg("-NoLogo");
    command
}

#[cfg(not(windows))]
fn shell_command() -> CommandBuilder {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".to_string());
    CommandBuilder::new(shell)
}

#[cfg(windows)]
fn configure_prompt(_command: &mut CommandBuilder) {}

#[cfg(not(windows))]
fn configure_prompt(command: &mut CommandBuilder) {
    command.env("PS1", "\\[\\e[36m\\]\\w\\[\\e[0m\\] \\[\\e[32m\\]>\\[\\e[0m\\] ");
}

#[cfg(windows)]
fn terminal_cwd(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    if let Some(path) = raw.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{path}"));
    }
    if let Some(path) = raw.strip_prefix(r"\\?\") {
        return PathBuf::from(path);
    }
    path.to_path_buf()
}

#[cfg(not(windows))]
fn terminal_cwd(path: &Path) -> PathBuf {
    path.to_path_buf()
}

#[cfg(all(test, windows))]
mod tests {
    use super::terminal_cwd;
    use std::path::Path;

    #[test]
    fn terminal_cwd_strips_windows_extended_path_prefix() {
        assert_eq!(
            terminal_cwd(Path::new(r"\\?\C:\Users\example\repo")),
            Path::new(r"C:\Users\example\repo")
        );
        assert_eq!(
            terminal_cwd(Path::new(r"\\?\UNC\server\share\repo")),
            Path::new(r"\\server\share\repo")
        );
    }
}
