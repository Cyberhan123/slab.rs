use std::sync::Arc;

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::get;
use futures::{SinkExt, StreamExt};
use slab_app_core::context::AppState;
use slab_app_core::domain::services::WorkspaceLspService;
use slab_types::plugin::PluginLanguageServerTransport;
use slab_utils::lsp::{read_lsp_stdio_message, write_lsp_stdio_message};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;
use tracing::{debug, warn};

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/workspace/lsp/{language}", get(upgrade_workspace_lsp))
}

async fn upgrade_workspace_lsp(
    State(service): State<WorkspaceLspService>,
    Path(language): Path<String>,
    upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    upgrade.on_upgrade(move |socket| handle_workspace_lsp_socket(service, language, socket))
}

async fn handle_workspace_lsp_socket(
    service: WorkspaceLspService,
    language: String,
    socket: WebSocket,
) {
    if let Err(error) = run_workspace_lsp_socket(service, language, socket).await {
        warn!(error = %error, "workspace LSP session ended");
    }
}

async fn run_workspace_lsp_socket(
    service: WorkspaceLspService,
    language: String,
    socket: WebSocket,
) -> Result<(), String> {
    let workspace_root = service.workspace_root().map_err(|error| error.to_string())?;
    let workspace_root = workspace_root
        .canonicalize()
        .map_err(|error| format!("failed to resolve workspace root: {error}"))?;
    if !workspace_root.is_dir() {
        return Err(format!("workspace root {} is not a directory", workspace_root.display()));
    }

    let Some(provider) =
        service.resolve_provider(&language).await.map_err(|error| error.to_string())?
    else {
        return Err(format!("no language server provider for language `{language}`"));
    };
    debug!(
        provider = %provider.contribution.id,
        workspace_root = %workspace_root.display(),
        language = %language,
        "starting workspace LSP session"
    );

    match &provider.contribution.transport {
        PluginLanguageServerTransport::Stdio { .. }
        | PluginLanguageServerTransport::NodePackage { .. } => {
            let mut process = service
                .spawn_stdio_process(&provider, &workspace_root)
                .await
                .map_err(|error| error.to_string())?;
            let (stdin, stdout) = process.io_mut();
            let result = bridge_websocket_to_stdio(socket, stdin, stdout).await;
            process.shutdown().await;
            result
        }
        PluginLanguageServerTransport::WebSocket { url } => {
            bridge_websocket_to_websocket(socket, url).await
        }
    }
}

async fn bridge_websocket_to_stdio<W, R>(
    socket: WebSocket,
    stdin: &mut W,
    stdout: &mut R,
) -> Result<(), String>
where
    W: AsyncWrite + Unpin,
    R: AsyncRead + Unpin,
{
    let (mut socket_sender, mut socket_receiver) = socket.split();

    loop {
        tokio::select! {
            message = read_lsp_stdio_message(stdout) => {
                match message? {
                    Some(message) => {
                        if socket_sender.send(Message::Text(message.into())).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
            message = socket_receiver.next() => {
                match message {
                    Some(Ok(Message::Text(text))) => {
                        write_lsp_stdio_message(stdin, text.as_bytes()).await?;
                    }
                    Some(Ok(Message::Binary(data))) => {
                        write_lsp_stdio_message(stdin, &data).await?;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(error)) => return Err(format!("websocket receive failed: {error}")),
                }
            }
        }
    }

    Ok(())
}

async fn bridge_websocket_to_websocket(
    socket: WebSocket,
    provider_url: &str,
) -> Result<(), String> {
    let (provider_socket, _) = connect_async(provider_url)
        .await
        .map_err(|error| format!("failed to connect language server websocket: {error}"))?;
    let (mut client_sender, mut client_receiver) = socket.split();
    let (mut provider_sender, mut provider_receiver) = provider_socket.split();

    loop {
        tokio::select! {
            message = client_receiver.next() => {
                match message {
                    Some(Ok(Message::Text(text))) => {
                        provider_sender
                            .send(TungsteniteMessage::Text(text.to_string().into()))
                            .await
                            .map_err(|error| format!("provider websocket send failed: {error}"))?;
                    }
                    Some(Ok(Message::Binary(data))) => {
                        provider_sender
                            .send(TungsteniteMessage::Binary(data.to_vec().into()))
                            .await
                            .map_err(|error| format!("provider websocket send failed: {error}"))?;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(error)) => return Err(format!("client websocket receive failed: {error}")),
                }
            }
            message = provider_receiver.next() => {
                match message {
                    Some(Ok(TungsteniteMessage::Text(text))) => {
                        if client_sender.send(Message::Text(text.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(TungsteniteMessage::Binary(data))) => {
                        if client_sender.send(Message::Binary(data.to_vec().into())).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(TungsteniteMessage::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(error)) => return Err(format!("provider websocket receive failed: {error}")),
                }
            }
        }
    }

    Ok(())
}
