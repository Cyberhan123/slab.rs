use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path as AxumPath, State};
use axum::response::IntoResponse;
use axum::routing::get;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use slab_utils::pty::{ProcessHandle, TerminalSize, spawn_pty_process};
use tauri::Manager;
use tauri::State as TauriState;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::paths::remove_windows_extended_path_prefix;
use crate::workspace::{WorkspaceState, active_workspace};

const DEFAULT_COLS: u16 = 100;
const DEFAULT_ROWS: u16 = 24;
const WORKSPACE_TERMINAL_ROUTE: &str = "/workspace-terminal";

#[derive(Clone, Default)]
struct TerminalServerInner {
    sessions: Arc<Mutex<HashMap<String, TerminalSessionRequest>>>,
}

impl TerminalServerInner {
    fn insert_session(
        &self,
        root_path: PathBuf,
        shell: WorkspaceTerminalShell,
    ) -> Result<String, String> {
        let id = Uuid::new_v4().to_string();
        let mut sessions =
            self.sessions.lock().map_err(|_| "failed to lock terminal sessions".to_string())?;
        sessions.insert(id.clone(), TerminalSessionRequest { root_path, shell });
        Ok(id)
    }

    fn take_session(&self, id: &str) -> Option<TerminalSessionRequest> {
        self.sessions.lock().ok().and_then(|mut sessions| sessions.remove(id))
    }
}

#[derive(Clone)]
struct TerminalSessionRequest {
    root_path: PathBuf,
    shell: WorkspaceTerminalShell,
}

pub struct WorkspaceTerminalState {
    endpoint_origin: String,
    inner: TerminalServerInner,
}

impl WorkspaceTerminalState {
    fn create_session(
        &self,
        root_path: PathBuf,
        shell: WorkspaceTerminalShell,
    ) -> Result<WorkspaceTerminalSession, String> {
        let session_id = self.inner.insert_session(root_path, shell)?;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkspaceTerminalShell {
    Powershell,
    Cmd,
    Bash,
    Zsh,
}

impl WorkspaceTerminalShell {
    fn parse(value: Option<String>) -> Result<Self, String> {
        match value.as_deref().unwrap_or(default_shell_name()) {
            "powershell" => Ok(Self::Powershell),
            "cmd" => Ok(Self::Cmd),
            "bash" => Ok(Self::Bash),
            "zsh" => Ok(Self::Zsh),
            shell => Err(format!("unsupported workspace terminal shell: {shell}")),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum TerminalClientMessage {
    Input { data: String },
    Resize { cols: u16, rows: u16 },
}

#[tauri::command]
pub fn workspace_terminal_session(
    shell: Option<String>,
    workspace_state: TauriState<'_, WorkspaceState>,
    terminal_state: TauriState<'_, WorkspaceTerminalState>,
) -> Result<WorkspaceTerminalSession, String> {
    workspace_terminal_session_for_state(shell, &workspace_state, &terminal_state)
}

fn workspace_terminal_session_for_state(
    shell: Option<String>,
    workspace_state: &WorkspaceState,
    terminal_state: &WorkspaceTerminalState,
) -> Result<WorkspaceTerminalSession, String> {
    let workspace = active_workspace(workspace_state)?;
    terminal_state
        .create_session(PathBuf::from(workspace.root_path), WorkspaceTerminalShell::parse(shell)?)
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
    let inner = TerminalServerInner::default();
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

    if let Err(error) = run_terminal_session(session.root_path, session.shell, socket).await {
        log::warn!("workspace terminal session ended: {error}");
    }
}

async fn run_terminal_session(
    root_path: PathBuf,
    shell: WorkspaceTerminalShell,
    socket: WebSocket,
) -> Result<(), String> {
    let (program, args) = shell_command(shell)?;
    let mut env: HashMap<String, String> = std::env::vars().collect();
    configure_prompt(&mut env);
    let spawned = spawn_pty_process(
        &program,
        &args,
        &terminal_cwd(&root_path),
        &env,
        &None,
        TerminalSize { rows: DEFAULT_ROWS, cols: DEFAULT_COLS },
    )
    .await
    .map_err(|error| format!("failed to spawn workspace shell: {error}"))?;
    let session = spawned.session;
    let writer = session.writer_sender();
    let mut output_rx = spawned.stdout_rx;
    let mut exit_rx = spawned.exit_rx;

    let (mut socket_sender, mut socket_receiver) = socket.split();
    loop {
        tokio::select! {
            exit = &mut exit_rx => {
                if let Ok(code) = exit {
                    log::debug!("workspace terminal process exited with code {code}");
                }
                break;
            }
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
                            handle_client_message(message, &session, &writer).await;
                        }
                    }
                    Some(Ok(Message::Binary(data))) => {
                        write_process(&writer, data.to_vec()).await;
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

    session.terminate();
    Ok(())
}

async fn handle_client_message(
    message: TerminalClientMessage,
    session: &ProcessHandle,
    writer: &mpsc::Sender<Vec<u8>>,
) {
    match message {
        TerminalClientMessage::Input { data } => {
            write_process(writer, data.into_bytes()).await;
        }
        TerminalClientMessage::Resize { cols, rows } => {
            let cols = cols.max(1);
            let rows = rows.max(1);
            if let Err(error) = session.resize(TerminalSize { rows, cols }) {
                log::warn!("workspace terminal resize failed: {error}");
            }
        }
    }
}

async fn write_process(writer: &mpsc::Sender<Vec<u8>>, data: Vec<u8>) {
    if writer.send(data).await.is_err() {
        log::warn!("workspace terminal input failed: process stdin is closed");
    }
}

#[cfg(windows)]
fn default_shell_name() -> &'static str {
    "powershell"
}

#[cfg(not(windows))]
fn default_shell_name() -> &'static str {
    let shell = std::env::var("SHELL").unwrap_or_default();
    if shell.ends_with("/zsh") { "zsh" } else { "bash" }
}

fn shell_command(shell: WorkspaceTerminalShell) -> Result<(String, Vec<String>), String> {
    match shell {
        WorkspaceTerminalShell::Powershell => {
            #[cfg(windows)]
            {
                Ok(("powershell.exe".to_owned(), vec!["-NoLogo".to_owned()]))
            }
            #[cfg(not(windows))]
            {
                Ok(("pwsh".to_owned(), vec!["-NoLogo".to_owned()]))
            }
        }
        WorkspaceTerminalShell::Cmd => {
            #[cfg(windows)]
            {
                Ok(("cmd.exe".to_owned(), Vec::new()))
            }
            #[cfg(not(windows))]
            {
                Err("cmd shell is only available on Windows".to_owned())
            }
        }
        WorkspaceTerminalShell::Bash => Ok(("bash".to_owned(), Vec::new())),
        WorkspaceTerminalShell::Zsh => Ok(("zsh".to_owned(), Vec::new())),
    }
}

#[cfg(windows)]
fn configure_prompt(_env: &mut HashMap<String, String>) {}

#[cfg(not(windows))]
fn configure_prompt(env: &mut HashMap<String, String>) {
    env.insert(
        "PS1".to_owned(),
        "\\[\\e[36m\\]\\w\\[\\e[0m\\] \\[\\e[32m\\]>\\[\\e[0m\\] ".to_owned(),
    );
}

fn terminal_cwd(path: &Path) -> PathBuf {
    remove_windows_extended_path_prefix(path)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{
        WORKSPACE_TERMINAL_ROUTE, WorkspaceTerminalShell, WorkspaceTerminalState, terminal_cwd,
        workspace_terminal_session_for_state,
    };
    use crate::workspace::{workspace_info_for_test, workspace_state_for_test};

    #[test]
    fn workspace_terminal_session_requires_active_workspace() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace_state = workspace_state_for_test(temp.path().join("recent.json"), None);
        let terminal_state = terminal_state_for_test();

        let error = workspace_terminal_session_for_state(None, &workspace_state, &terminal_state)
            .expect_err("workspace should be required");

        assert_eq!(error, "no workspace is currently open");
    }

    #[test]
    fn workspace_terminal_session_enqueues_active_workspace_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace_state = workspace_state_for_test(
            temp.path().join("recent.json"),
            Some(workspace_info_for_test(temp.path())),
        );
        let terminal_state = terminal_state_for_test();

        let session = workspace_terminal_session_for_state(
            Some("bash".to_owned()),
            &workspace_state,
            &terminal_state,
        )
        .expect("terminal session");

        let prefix = format!("ws://127.0.0.1:3210{WORKSPACE_TERMINAL_ROUTE}/");
        assert!(session.url.starts_with(&prefix));
        let session_id = session.url.strip_prefix(&prefix).expect("session id");
        assert!(!session_id.is_empty());
        let request = terminal_state.inner.take_session(session_id).expect("queued session");
        assert_eq!(request.root_path, PathBuf::from(temp.path()));
        assert_eq!(request.shell, WorkspaceTerminalShell::Bash);
        assert!(terminal_state.inner.take_session(session_id).is_none());
    }

    #[test]
    fn workspace_terminal_rejects_unknown_shell() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace_state = workspace_state_for_test(
            temp.path().join("recent.json"),
            Some(workspace_info_for_test(temp.path())),
        );
        let terminal_state = terminal_state_for_test();

        let error = workspace_terminal_session_for_state(
            Some("fish".to_owned()),
            &workspace_state,
            &terminal_state,
        )
        .expect_err("unsupported shell rejected");

        assert!(error.contains("unsupported workspace terminal shell"));
    }

    #[test]
    fn workspace_terminal_rejects_unknown_session_id() {
        let terminal_state = terminal_state_for_test();

        assert!(terminal_state.inner.take_session("missing-session").is_none());
    }

    #[test]
    fn terminal_cwd_normalizes_shell_working_directory() {
        #[cfg(windows)]
        assert_eq!(
            terminal_cwd(Path::new(r"\\?\C:\Users\example\repo")),
            Path::new(r"C:\Users\example\repo")
        );

        #[cfg(not(windows))]
        assert_eq!(terminal_cwd(Path::new("workspace")), Path::new("workspace"));
    }

    fn terminal_state_for_test() -> WorkspaceTerminalState {
        WorkspaceTerminalState {
            endpoint_origin: "ws://127.0.0.1:3210".to_string(),
            inner: Default::default(),
        }
    }
}
