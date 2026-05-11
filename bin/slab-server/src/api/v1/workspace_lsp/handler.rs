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
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
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
        provider = %provider.id,
        workspace_root = %workspace_root.display(),
        language = %language,
        "starting workspace LSP session"
    );

    match &provider.transport {
        PluginLanguageServerTransport::Stdio { .. } => {
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

pub async fn read_lsp_stdio_message<R>(reader: &mut R) -> Result<Option<String>, String>
where
    R: AsyncRead + Unpin,
{
    let mut header = Vec::new();
    let mut byte = [0_u8; 1];

    loop {
        match reader.read_exact(&mut byte).await {
            Ok(_) => {
                header.push(byte[0]);
                if header.ends_with(b"\r\n\r\n") {
                    break;
                }
                if header.len() > 8192 {
                    return Err("language server response header is too large".to_owned());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                if header.is_empty() {
                    return Ok(None);
                }
                return Err("language server closed while sending response header".to_owned());
            }
            Err(error) => return Err(format!("failed to read language server header: {error}")),
        }
    }

    let header = String::from_utf8(header)
        .map_err(|_| "language server response header is not UTF-8".to_owned())?;
    let content_length = parse_content_length(&header)?;
    let mut body = vec![0_u8; content_length];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|error| format!("failed to read language server body: {error}"))?;
    String::from_utf8(body)
        .map(Some)
        .map_err(|_| "language server response body is not UTF-8".to_owned())
}

pub async fn write_lsp_stdio_message<W>(writer: &mut W, body: &[u8]) -> Result<(), String>
where
    W: AsyncWrite + Unpin,
{
    writer
        .write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes())
        .await
        .map_err(|error| format!("failed to write language server header: {error}"))?;
    writer
        .write_all(body)
        .await
        .map_err(|error| format!("failed to write language server body: {error}"))?;
    writer
        .flush()
        .await
        .map_err(|error| format!("failed to flush language server message: {error}"))
}

fn parse_content_length(header: &str) -> Result<usize, String> {
    for line in header.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("content-length") {
            return value
                .trim()
                .parse::<usize>()
                .map_err(|_| "invalid language server Content-Length header".to_owned());
        }
    }

    Err("language server response missing Content-Length header".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{parse_content_length, read_lsp_stdio_message, write_lsp_stdio_message};
    use tokio::io::AsyncWriteExt;

    #[test]
    fn parses_case_insensitive_content_length() {
        let length = parse_content_length(
            "content-length: 42\r\ncontent-type: application/vscode-jsonrpc\r\n\r\n",
        )
        .expect("content length");

        assert_eq!(length, 42);
    }

    #[tokio::test]
    async fn reads_stdio_framed_lsp_message() {
        let mut framed = std::io::Cursor::new(
            b"Content-Length: 24\r\nContent-Type: application/vscode-jsonrpc\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1}".to_vec(),
        );

        let message = read_lsp_stdio_message(&mut framed).await.expect("read").expect("message");

        assert_eq!(message, "{\"jsonrpc\":\"2.0\",\"id\":1}");
    }

    #[tokio::test]
    async fn writes_stdio_framed_lsp_message() {
        let (mut writer, mut reader) = tokio::io::duplex(128);
        let write = async move {
            write_lsp_stdio_message(&mut writer, br#"{"jsonrpc":"2.0"}"#).await.expect("write");
            writer.shutdown().await.expect("shutdown");
        };
        let read = async move {
            read_lsp_stdio_message(&mut reader).await.expect("read").expect("message")
        };

        let (_, message) = tokio::join!(write, read);

        assert_eq!(message, r#"{"jsonrpc":"2.0"}"#);
    }
}
