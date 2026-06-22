use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use slab_app_core::context::AppState;
use slab_utils::pty::{ProcessHandle, TerminalSize, spawn_pty_process};
use tokio::sync::mpsc;
use tracing::{debug, warn};

use super::handler::active_workspace_root;
use crate::error::ServerError;

const DEFAULT_COLS: u16 = 100;
const DEFAULT_ROWS: u16 = 24;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkspaceTerminalQuery {
    shell: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkspaceTerminalShell {
    Powershell,
    Cmd,
    Bash,
    Zsh,
}

impl WorkspaceTerminalShell {
    fn parse(value: Option<String>) -> Result<Self, ServerError> {
        match value.as_deref().unwrap_or(default_shell_name()) {
            "powershell" => Ok(Self::Powershell),
            "cmd" => Ok(Self::Cmd),
            "bash" => Ok(Self::Bash),
            "zsh" => Ok(Self::Zsh),
            shell => Err(ServerError::BadRequest(format!(
                "unsupported workspace terminal shell: {shell}"
            ))),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum TerminalClientMessage {
    Input { data: String },
    Resize { cols: u16, rows: u16 },
}

pub(super) async fn upgrade_workspace_terminal(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceTerminalQuery>,
    upgrade: WebSocketUpgrade,
) -> Result<impl IntoResponse, ServerError> {
    let root = active_workspace_root(state.as_ref())?;
    let shell = WorkspaceTerminalShell::parse(query.shell)?;
    Ok(upgrade.on_upgrade(move |socket| handle_terminal_socket(root, shell, socket)))
}

async fn handle_terminal_socket(root: PathBuf, shell: WorkspaceTerminalShell, socket: WebSocket) {
    if let Err(error) = run_terminal_session(root, shell, socket).await {
        warn!(error = %error, "workspace terminal session ended");
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

    debug!(
        workspace_root = %root_path.display(),
        program = %program,
        "starting workspace terminal session"
    );

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
                    debug!("workspace terminal process exited with code {code}");
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
                        warn!("workspace terminal websocket error: {error}");
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
                warn!("workspace terminal resize failed: {error}");
            }
        }
    }
}

async fn write_process(writer: &mpsc::Sender<Vec<u8>>, data: Vec<u8>) {
    if writer.send(data).await.is_err() {
        warn!("workspace terminal input failed: process stdin is closed");
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
    let raw = path.to_string_lossy();
    if let Some(path) = raw.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{path}"));
    }
    if let Some(path) = raw.strip_prefix(r"\\?\") {
        return PathBuf::from(path);
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{WorkspaceTerminalShell, terminal_cwd};

    #[test]
    fn terminal_cwd_normalizes_windows_verbatim_paths() {
        assert_eq!(
            terminal_cwd(Path::new(r"\\?\C:\Users\example\repo")),
            Path::new(r"C:\Users\example\repo")
        );
        assert_eq!(
            terminal_cwd(Path::new(r"\\?\UNC\server\share\repo")),
            Path::new(r"\\server\share\repo")
        );
        assert_eq!(terminal_cwd(Path::new("workspace")), Path::new("workspace"));
    }

    #[test]
    fn workspace_terminal_shell_parse_rejects_unknown_shells() {
        let error = WorkspaceTerminalShell::parse(Some("fish".to_owned()))
            .expect_err("unsupported shell rejected");

        assert!(error.to_string().contains("unsupported workspace terminal shell"));
    }
}
