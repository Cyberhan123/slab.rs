//! Named-pipe protocol between the non-elevated orchestrator (slab-server) and the long-lived
//! elevated daemon (`slab-sandbox-helper --serve`). Framing is length-prefixed JSON (4-byte
//! big-endian length + JSON bytes) so both sides share one self-describing wire type.
//!
//! S2b1 ships only `Ping`/`Pong` (daemon liveness + reconnect). S2b2 adds `Spawn`/`Output`/
//! `Exited`/`Kill` for the real Low-IL restricted-token child.
//!
//! Why a daemon at all: UAC-per-command is unusable, and a non-elevated orchestrator cannot
//! `CreateProcessAsUserW` a Low-integrity token it did not create. So elevation happens once at
//! daemon start; each command is a non-interactive pipe round-trip.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::windows::named_pipe::ClientOptions;

use crate::error::WindowsSandboxError;

/// Maximum frame size (64 MiB) — guards against a runaway/peer feeding a bogus length prefix.
const MAX_FRAME_LEN: u32 = 64 * 1024 * 1024;

/// A single framed pipe message. Tagged so S2b2 can add variants without breaking the wire.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PipeFrame {
    /// Client liveness probe; daemon echoes the nonce in `Pong`.
    Ping { nonce: String },
    /// Daemon reply to `Ping`.
    Pong { nonce: String },
}

/// Write a length-prefixed JSON frame.
pub async fn write_frame<W: AsyncWrite + Unpin>(
    w: &mut W,
    frame: &PipeFrame,
) -> Result<(), WindowsSandboxError> {
    let bytes = serde_json::to_vec(frame)
        .map_err(|e| WindowsSandboxError::SetupFailed(format!("pipe frame encode: {e}")))?;
    let len = u32::try_from(bytes.len()).map_err(|_| {
        WindowsSandboxError::SetupFailed("pipe frame exceeds u32 length".to_string())
    })?;
    if len > MAX_FRAME_LEN {
        return Err(WindowsSandboxError::SetupFailed(format!(
            "pipe frame too large ({len} bytes)"
        )));
    }
    w.write_all(&len.to_be_bytes()).await.map_err(io_err)?;
    w.write_all(&bytes).await.map_err(io_err)?;
    w.flush().await.map_err(io_err)?;
    Ok(())
}

/// Read a length-prefixed JSON frame.
pub async fn read_frame<R: AsyncRead + Unpin>(r: &mut R) -> Result<PipeFrame, WindowsSandboxError> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            WindowsSandboxError::SetupFailed("pipe closed mid-frame-length".to_string())
        } else {
            io_err(e)
        }
    })?;
    let len = u32::from_be_bytes(len_buf);
    if len == 0 || len > MAX_FRAME_LEN {
        return Err(WindowsSandboxError::SetupFailed(format!("invalid pipe frame length ({len})")));
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf).await.map_err(|e| {
        WindowsSandboxError::SetupFailed(format!("pipe closed mid-frame-body: {e}"))
    })?;
    serde_json::from_slice(&buf)
        .map_err(|e| WindowsSandboxError::SetupFailed(format!("pipe frame decode: {e}")))
}

fn io_err(e: std::io::Error) -> WindowsSandboxError {
    WindowsSandboxError::WindowsApi(e.to_string())
}

/// Connect to the daemon's named pipe and Ping it; returns the echoed nonce on success. Used by
/// the orchestrator both to confirm a daemon is alive (reconnect) and to wait for a freshly
/// started daemon to become ready. The connect retries until `timeout` (the daemon may still be
/// starting up, or all instances momentarily busy).
pub async fn ping(pipe_name: &str, nonce: &str) -> Result<String, WindowsSandboxError> {
    ping_with_timeout(pipe_name, nonce, Duration::from_secs(15)).await
}

pub async fn ping_with_timeout(
    pipe_name: &str,
    nonce: &str,
    timeout: Duration,
) -> Result<String, WindowsSandboxError> {
    let started = std::time::Instant::now();
    let client = loop {
        match ClientOptions::new().open(pipe_name) {
            Ok(c) => break c,
            Err(_) if started.elapsed() < timeout => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(e) => {
                return Err(WindowsSandboxError::WindowsApi(format!(
                    "pipe connect timed out: {e}"
                )));
            }
        }
    };
    let (mut reader, mut writer) = tokio::io::split(client);
    write_frame(&mut writer, &PipeFrame::Ping { nonce: nonce.to_string() }).await?;
    let reply = tokio::time::timeout(timeout, read_frame(&mut reader)).await.map_err(|_| {
        WindowsSandboxError::ElevationFailed("timed out waiting for daemon pong".into())
    })??;
    match reply {
        PipeFrame::Pong { nonce: echoed } => Ok(echoed),
        other => Err(WindowsSandboxError::SetupFailed(format!(
            "daemon replied with unexpected frame: {other:?}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::run_daemon;

    /// Each test needs a unique pipe name so concurrent tests don't collide.
    fn test_pipe_name(label: &str) -> String {
        // Unique-ish per process + label (no randomness needed).
        format!(r"\\.\pipe\slab-sandbox-test-{}-{}", std::process::id(), label)
    }

    #[tokio::test]
    async fn ping_pong_roundtrip() {
        let name = test_pipe_name("pp");
        let daemon_task = tokio::spawn(run_daemon(name.clone()));

        // `ping` retries the connect until the daemon has created its first instance.
        let echoed = ping(&name, "hello-daemon").await.expect("ping ok");
        assert_eq!(echoed, "hello-daemon");

        daemon_task.abort();
    }

    #[tokio::test]
    async fn frame_round_trip_through_memory() {
        // Round-trip the codec without a real pipe (pure framing sanity).
        let frame = PipeFrame::Ping { nonce: "n".into() };
        let mut buf = Vec::<u8>::new();
        write_frame(&mut buf, &frame).await.unwrap();
        let mut cursor = std::io::Cursor::new(buf);
        let back = read_frame(&mut cursor).await.unwrap();
        assert_eq!(back, frame);
    }
}
