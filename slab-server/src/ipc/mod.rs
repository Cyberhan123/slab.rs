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
//!
//! # Security
//!
//! The Unix socket is world-accessible by default when placed in `/tmp`.
//! In production, set `SLAB_IPC_SOCKET` to a path inside a directory with
//! restricted permissions (e.g. `/var/run/slab/server.sock`).

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::Duration;
use tracing::{debug, error, info, warn};

use crate::state::AppState;
use slab_core::api::{Event,Backend};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum line length accepted from an IPC client (1 MiB).
/// Lines exceeding this limit cause the connection to be closed.
const MAX_LINE_BYTES: usize = 1024 * 1024;

/// Timeout for reading a single newline-terminated line from an IPC client.
/// A client that sends no data for this long will be disconnected.
const LINE_READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum prompt length forwarded to any backend (128 KiB).
const MAX_PROMPT_BYTES: usize = 128 * 1024;

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
    _model: String,
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

    // Remove a stale socket file from a previous run, but only if it is
    // actually a socket – to avoid accidentally deleting an unrelated file.
    remove_stale_socket(&socket_path);

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

/// Remove a stale socket file only if it is confirmed to be a socket.
///
/// This prevents accidentally deleting a regular file or directory that
/// happens to exist at the configured socket path.
#[cfg(unix)]
fn remove_stale_socket(path: &str) {
    use std::os::unix::fs::FileTypeExt;
    match std::fs::metadata(path) {
        Err(_) => {} // file does not exist – nothing to do
        Ok(meta) if meta.file_type().is_socket() => {
            if let Err(e) = std::fs::remove_file(path) {
                warn!(path = %path, error = %e, "failed to remove stale IPC socket");
            }
        }
        Ok(_) => {
            warn!(
                path = %path,
                "path exists but is not a socket; refusing to remove it"
            );
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
///
/// Each line read is subject to [`LINE_READ_TIMEOUT`] and [`MAX_LINE_BYTES`]
/// limits to prevent a misbehaving client from holding a connection open
/// indefinitely or causing memory exhaustion.
async fn handle_connection<S>(stream: S, _state: Arc<AppState>)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (reader, mut writer) = tokio::io::split(stream);
    let mut lines = BufReader::new(reader).lines();
    let mut bytes_read_for_line: usize = 0;

    loop {
        // Apply a per-line timeout so a slow/stalled client cannot hold the
        // connection open forever.
        let read_result = tokio::time::timeout(LINE_READ_TIMEOUT, lines.next_line()).await;

        let line = match read_result {
            Err(_elapsed) => {
                warn!("IPC client timed out waiting for newline; closing connection");
                break;
            }
            Ok(Err(e)) => {
                warn!(error = %e, "IPC read error; closing connection");
                break;
            }
            Ok(Ok(None)) => break, // client closed the connection
            Ok(Ok(Some(l))) => l,
        };

        // Reject oversized messages.
        bytes_read_for_line = line.len();
        if bytes_read_for_line > MAX_LINE_BYTES {
            let resp = IpcResponse {
                ok:     false,
                result: None,
                error:  Some(format!("message too large ({bytes_read_for_line} bytes)")),
            };
            let _ = write_response(&mut writer, &resp).await;
            warn!(bytes = bytes_read_for_line, "IPC message too large; closing connection");
            break;
        }

        debug!(line_len = line.len(), "IPC request received");

        let resp = match serde_json::from_str::<IpcRequest>(&line) {
            Err(e) => IpcResponse {
                ok:     false,
                result: None,
                error:  Some(format!("invalid JSON: {e}")),
            },
            Ok(req) => dispatch(req).await,
        };

        if write_response(&mut writer, &resp).await.is_err() {
            break;
        }
    }
}

/// Serialise and write an [`IpcResponse`] followed by a newline.
async fn write_response<W: tokio::io::AsyncWrite + Unpin>(
    writer: &mut W,
    resp: &IpcResponse,
) -> Result<(), std::io::Error> {
    let mut json = serde_json::to_string(resp).unwrap_or_default();
    json.push('\n');
    writer.write_all(json.as_bytes()).await.map_err(|e| {
        warn!(error = %e, "IPC write error; closing connection");
        e
    })
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

/// Route an [`IpcRequest`] to the appropriate slab-core backend.
async fn dispatch(req: IpcRequest) -> IpcResponse {
    // Reject oversized prompts before they reach the backend.
    if req.prompt.len() > MAX_PROMPT_BYTES {
        return IpcResponse {
            ok:     false,
            result: None,
            error:  Some(format!(
                "prompt too large ({} bytes); maximum is {MAX_PROMPT_BYTES} bytes",
                req.prompt.len()
            )),
        };
    }

    let result = match req.op.as_str() {
        "chat" => {
            slab_core::api::backend(Backend::GGMLLama)
                .op(Event::Inference)
                .input(slab_core::Payload::Text(std::sync::Arc::from(
                    req.prompt.as_str(),
                )))
                .run_wait()
                .await
                .map(|b| String::from_utf8_lossy(&b).into_owned())
        }
        "transcribe" => {
            // For IPC transcription, the `prompt` field carries a file path.
            slab_core::api::backend(Backend::GGMLWhisper)
                .op(Event::Inference)
                .input(slab_core::Payload::Text(std::sync::Arc::from(
                    req.prompt.as_str(),
                )))
                .run_wait()
                .await
                .map(|b| String::from_utf8_lossy(&b).into_owned())
        }
        "generate_image" => {
            slab_core::api::backend(Backend::GGMLDiffusion)
                .op(Event::Inference)
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

