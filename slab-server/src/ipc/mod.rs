//! Inter-Process Communication (IPC) listener.
//!
//! On Unix, listens on a Unix-domain socket.  On Windows, a named pipe is
//! used (stub only in this iteration).  Each connection receives newline-
//! delimited JSON messages:
//!
//! **Request:**
//! ```json
//! {"op": "chat", "prompt": "Hello!", "model": ""}
//! ```
//!
//! **Response:**
//! ```json
//! {"ok": true, "result": "Hi there!"}
//! ```
//! or on error:
//! ```json
//! {"ok": false, "error": "unknown op: foo"}
//! ```

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info, warn};

use crate::state::AppState;

// ── Protocol ──────────────────────────────────────────────────────────────────

/// Incoming IPC command sent by a client over the socket.
#[derive(Debug, Deserialize)]
struct IpcRequest {
    /// Operation: `"chat"`, `"transcribe"`, or `"generate_image"`.
    op: String,
    /// Text prompt or payload (operation-specific).
    #[serde(default)]
    prompt: String,
    /// Optional backend model override (reserved for future use).
    #[serde(default)]
    model: String,
}

/// IPC response envelope written back to the client.
#[derive(Debug, Serialize)]
struct IpcResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

// ── Entry-point ───────────────────────────────────────────────────────────────

/// Start the IPC listener and handle connections indefinitely.
///
/// - **Unix**: creates a Unix-domain socket at `socket_path`.
/// - **Windows**: `socket_path` is treated as a named-pipe path (stub).
pub async fn serve(socket_path: String, state: Arc<AppState>) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        serve_unix(socket_path, state).await
    }
    #[cfg(windows)]
    {
        serve_windows(socket_path, state).await
    }
}

// ── Unix implementation ───────────────────────────────────────────────────────

#[cfg(unix)]
async fn serve_unix(socket_path: String, state: Arc<AppState>) -> anyhow::Result<()> {
    use tokio::net::UnixListener;

    // Remove a stale socket file left from a previous run.
    let _ = std::fs::remove_file(&socket_path);

    let listener = UnixListener::bind(&socket_path)?;
    info!(socket_path = %socket_path, "IPC Unix-socket listening");

    loop {
        match listener.accept().await {
            Err(e) => error!(error = %e, "IPC accept error"),
            Ok((stream, _addr)) => {
                let state = Arc::clone(&state);
                tokio::spawn(async move {
                    handle_connection(stream, state).await;
                });
            }
        }
    }
}

// ── Windows stub ──────────────────────────────────────────────────────────────

#[cfg(windows)]
async fn serve_windows(_socket_path: String, _state: Arc<AppState>) -> anyhow::Result<()> {
    warn!("IPC named-pipe support is not yet implemented on Windows; IPC disabled");
    // Keep the task alive without busy-looping.
    std::future::pending::<()>().await;
    Ok(())
}

// ── Per-connection handler ────────────────────────────────────────────────────

/// Read newline-delimited JSON requests from `stream` and write responses.
async fn handle_connection<S>(stream: S, _state: Arc<AppState>)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (reader, mut writer) = tokio::io::split(stream);
    let mut lines = BufReader::new(reader).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        debug!(line_len = line.len(), "IPC request received");

        let resp = match serde_json::from_str::<IpcRequest>(&line) {
            Err(e) => IpcResponse {
                ok:     false,
                result: None,
                error:  Some(format!("invalid JSON: {e}")),
            },
            Ok(req) => dispatch(req).await,
        };

        let mut json = serde_json::to_string(&resp).unwrap_or_default();
        json.push('\n');

        if let Err(e) = writer.write_all(json.as_bytes()).await {
            warn!(error = %e, "IPC write error; closing connection");
            break;
        }
    }
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

/// Route an [`IpcRequest`] to the appropriate slab-core backend.
async fn dispatch(req: IpcRequest) -> IpcResponse {
    // Suppress unused-variable warning for `model` field (reserved).
    let _ = &req.model;

    let result = match req.op.as_str() {
        "chat" => {
            slab_core::api::backend("ggml.llama")
                .op("inference")
                .input(slab_core::Payload::Text(std::sync::Arc::from(
                    req.prompt.as_str(),
                )))
                .run_wait()
                .await
                .map(|b| String::from_utf8_lossy(&b).into_owned())
        }
        "transcribe" => {
            // For IPC transcription the `prompt` field carries a file path.
            slab_core::api::backend("ggml.whisper")
                .op("inference")
                .input(slab_core::Payload::Text(std::sync::Arc::from(
                    req.prompt.as_str(),
                )))
                .run_wait()
                .await
                .map(|b| String::from_utf8_lossy(&b).into_owned())
        }
        "generate_image" => {
            slab_core::api::backend("ggml.diffusion")
                .op("inference_image")
                .input(slab_core::Payload::Json(serde_json::json!({
                    "prompt": req.prompt
                })))
                .run_wait()
                .await
                .map(|b| format!("<{} bytes of image data>", b.len()))
        }
        unknown => {
            return IpcResponse {
                ok:     false,
                result: None,
                error:  Some(format!("unknown op: {unknown}")),
            }
        }
    };

    match result {
        Ok(text) => IpcResponse { ok: true,  result: Some(text), error: None },
        Err(e)   => IpcResponse { ok: false, result: None,       error: Some(e.to_string()) },
    }
}
