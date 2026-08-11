//! Named-pipe protocol between the non-elevated orchestrator (slab-server) and the long-lived
//! elevated daemon (`slab-sandbox-helper --serve`). Framing is length-prefixed JSON (4-byte
//! big-endian length + JSON bytes) so both sides share one self-describing wire type.
//!
//! S2b1 ships only `Ping`/`Pong` (daemon liveness + reconnect). S2b2 adds `Provision`/`Spawn`/
//! `Output`/`Exited`/`Kill` for the real Low-IL restricted-token child.
//!
//! Why a daemon at all: UAC-per-command is unusable, and a non-elevated orchestrator cannot
//! `CreateProcessAsUserW` a Low-integrity token it did not create. So elevation happens once at
//! daemon start; each command is a non-interactive pipe round-trip.
//!
//! Frame integrity: `Provision`/`Spawn`/`Kill` carry an HMAC `tag` (ring SHA-256, key =
//! DPAPI-sealed, same-user). The daemon verifies before acting. This is defense-in-depth — the
//! pipe name is derived from the key fingerprint (unguessable to other users) and the SACL labels
//! are the real boundary — but the daemon is elevated, so we gate every mutating command on the
//! tag. `spawn.env` is a HashMap, so the tag is computed over a CANONICAL form (sorted keys) via
//! `serde_json::to_value` (BTreeMap-backed; the workspace does not enable `preserve_order`).

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::windows::named_pipe::ClientOptions;

use crate::error::WindowsSandboxError;
use crate::ipc::SetupMarker;
use crate::mac;
use crate::request::SpawnRequest;

/// Maximum frame size (64 MiB) — guards against a runaway/peer feeding a bogus length prefix.
const MAX_FRAME_LEN: u32 = 64 * 1024 * 1024;

/// Which child stream an [`PipeFrame::Output`] chunk belongs to.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputStreamKind {
    Stdout,
    Stderr,
}

/// A single framed pipe message. Tagged so variants can be added without breaking the wire.
///
/// Note: `Eq` is intentionally NOT derived — `Spawn` embeds `SpawnRequest` whose `env` is a
/// `HashMap` (not `Eq`). `PartialEq` suffices for tests.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PipeFrame {
    /// Client liveness probe; daemon echoes the nonce in `Pong`.
    Ping { nonce: String },
    /// Daemon reply to `Ping`.
    Pong { nonce: String },

    /// Orchestrator → daemon: apply integrity-label ACLs + write the marker (once per session).
    /// `tag` = HMAC over the canonical form of the signed fields.
    Provision {
        denied_paths: Vec<PathBuf>,
        writable_roots: Vec<PathBuf>,
        workspace_root: Option<PathBuf>,
        key_fingerprint: String,
        tag: String,
    },
    /// Daemon → orchestrator: provisioning completed; carries the written marker.
    ProvisionOk { marker: SetupMarker },

    /// Orchestrator → daemon: spawn a Low-IL restricted child. `job_token` is generated
    /// orchestrator-side and echoed in the subsequent `Output`/`Exited` frames for correlation.
    Spawn { job_token: String, spawn: SpawnRequest, tag: String },
    /// Daemon → orchestrator: child launched; relay begins.
    SpawnAccepted { job_token: String },

    /// Daemon → orchestrator: incremental child output (raw bytes, binary-safe).
    Output { job_token: String, stream: OutputStreamKind, bytes: Vec<u8> },
    /// Daemon → orchestrator: child exited.
    Exited { job_token: String, code: i32, timed_out: bool },

    /// Orchestrator → daemon: tear down a job (drops the Job ⇒ `KILL_ON_JOB_CLOSE`).
    Kill { job_token: String, tag: String },
}

/// Canonical (sorted-key) JSON bytes for a value, so a HashMap field serializes identically on
/// both sides of the pipe. See module docs on why `preserve_order` is off.
fn canonical_bytes<T: Serialize>(v: &T) -> Result<Vec<u8>, WindowsSandboxError> {
    let value = serde_json::to_value(v)
        .map_err(|e| WindowsSandboxError::SetupFailed(format!("canonical to_value: {e}")))?;
    serde_json::to_vec(&value)
        .map_err(|e| WindowsSandboxError::SetupFailed(format!("canonical to_vec: {e}")))
}

/// HMAC-SHA256 tag (hex) over the canonical form of `v`.
pub(crate) fn compute_tag<T: Serialize>(key: &[u8], v: &T) -> Result<String, WindowsSandboxError> {
    Ok(hex::encode(mac::sign(key, &canonical_bytes(v)?)))
}

/// Verify a hex tag against the canonical form of `v`. False on any decode/serialization failure.
pub(crate) fn tag_matches<T: Serialize>(key: &[u8], v: &T, given_hex: &str) -> bool {
    let Ok(body) = canonical_bytes(v) else {
        return false;
    };
    let Ok(given) = hex::decode(given_hex) else {
        return false;
    };
    mac::verify(key, &body, &given)
}

/// Tag for a `Provision` frame, over `(denied_paths, writable_roots, workspace_root, key_fingerprint)`.
pub(crate) fn provision_tag(
    key: &[u8],
    denied_paths: &[PathBuf],
    writable_roots: &[PathBuf],
    workspace_root: Option<&PathBuf>,
    key_fingerprint: &str,
) -> Result<String, WindowsSandboxError> {
    compute_tag(key, &(denied_paths, writable_roots, workspace_root, key_fingerprint))
}

/// Tag for a `Spawn` frame, over `(job_token, spawn)`.
pub(crate) fn spawn_tag(
    key: &[u8],
    job_token: &str,
    spawn: &SpawnRequest,
) -> Result<String, WindowsSandboxError> {
    compute_tag(key, &(job_token, spawn))
}

// Note: there is no `kill_tag` — the orchestrator tears a job down by DROPPING its daemon
// connection (the daemon's `KILL_ON_JOB_CLOSE` fires on disconnect) rather than sending an explicit
// `Kill` frame, because `kill_tree` is a synchronous `FnOnce` and cannot `await` a pipe write. The
// daemon still handles `Kill` defensively (verifying its tag via `tag_matches`).

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

    fn key() -> Vec<u8> {
        b"pipe-test-key-32-bytes-_____________"[..32].to_vec()
    }

    fn sample_spawn() -> SpawnRequest {
        let mut env = std::collections::HashMap::new();
        env.insert("Z_LAST".to_string(), "1".to_string());
        env.insert("A_FIRST".to_string(), "2".to_string());
        SpawnRequest {
            argv: vec!["cmd".into(), "/c".into(), "echo hi".into()],
            env,
            cwd: Some(PathBuf::from(r"C:\tmp")),
            denied_paths: vec![PathBuf::from(r"C:\secret")],
            denied_globs: vec!["**/.git/config".into()],
            writable_roots: vec![PathBuf::from(r"C:\ws")],
            workspace_root: Some(PathBuf::from(r"C:\ws")),
            network_blocked: false,
            use_conpty: false,
            diagnostic_plain_spawn: false,
            diagnostic_no_low_il_token: false,
            diagnostic_new_console: false,
            diagnostic_bare_spawn: false,
        }
    }

    #[tokio::test]
    async fn ping_pong_roundtrip() {
        let name = test_pipe_name("pp");
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("helper.key").to_path_buf();
        let marker_path = dir.path().join("marker.json").to_path_buf();
        let daemon_task = tokio::spawn(run_daemon(name.clone(), key_path, marker_path));

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

    #[tokio::test]
    async fn new_variants_round_trip_through_memory() {
        let spawn = sample_spawn();
        let st = spawn_tag(&key(), "job-1", &spawn).unwrap();
        let frames = vec![
            PipeFrame::Spawn { job_token: "job-1".into(), spawn: spawn.clone(), tag: st.clone() },
            PipeFrame::SpawnAccepted { job_token: "job-1".into() },
            PipeFrame::Output {
                job_token: "job-1".into(),
                stream: OutputStreamKind::Stdout,
                bytes: b"hello\n".to_vec(),
            },
            PipeFrame::Exited { job_token: "job-1".into(), code: 0, timed_out: false },
            PipeFrame::Kill {
                job_token: "job-1".into(),
                tag: compute_tag(&key(), &"job-1").unwrap(),
            },
        ];
        for frame in frames {
            let mut buf = Vec::<u8>::new();
            write_frame(&mut buf, &frame).await.unwrap();
            let back = read_frame(&mut std::io::Cursor::new(buf)).await.unwrap();
            assert_eq!(back, frame);
        }
    }

    #[test]
    fn tag_round_trips_and_rejects_tamper() {
        let key = key();
        let spawn = sample_spawn();

        // Spawn tag verifies against the same fields.
        let st = spawn_tag(&key, "job-1", &spawn).unwrap();
        assert!(tag_matches(&key, &("job-1", &spawn), &st));
        // Tampered job_token must fail.
        assert!(!tag_matches(&key, &("job-2", &spawn), &st));
        // Tampered spawn must fail.
        let mut spawn2 = spawn.clone();
        spawn2.argv.push("extra".into());
        assert!(!tag_matches(&key, &("job-1", &spawn2), &st));

        // Provision tag is stable despite HashMap ordering in spawn (no HashMap here, but exercise it).
        let denied = vec![PathBuf::from(r"C:\x")];
        let writable = vec![PathBuf::from(r"C:\ws")];
        let pt =
            provision_tag(&key, &denied, &writable, Some(&PathBuf::from(r"C:\ws")), "fp").unwrap();
        assert!(tag_matches(
            &key,
            &(denied.as_slice(), writable.as_slice(), Some(&PathBuf::from(r"C:\ws")), "fp"),
            &pt,
        ));

        // Kill/provision verify round-trip via compute_tag + tag_matches.
        let kt = compute_tag(&key, &"job-9").unwrap();
        assert!(tag_matches(&key, &"job-9", &kt));
        assert!(!tag_matches(&key, &"job-8", &kt));
    }

    #[test]
    fn spawn_tag_is_independent_of_env_map_iteration_order() {
        // The tag must be identical regardless of HashMap iteration order (canonical serialization).
        let key = key();
        let mut env_a = std::collections::HashMap::new();
        env_a.insert("PATH".into(), "x".into());
        env_a.insert("HOME".into(), "y".into());
        let mut env_b = std::collections::HashMap::new();
        // Insert in opposite order; HashMap may still differ in layout.
        env_b.insert("HOME".into(), "y".into());
        env_b.insert("PATH".into(), "x".into());
        let sa = SpawnRequest {
            argv: vec!["a".into()],
            env: env_a,
            cwd: None,
            denied_paths: vec![],
            denied_globs: vec![],
            writable_roots: vec![],
            workspace_root: None,
            network_blocked: false,
            use_conpty: false,
            diagnostic_plain_spawn: false,
            diagnostic_no_low_il_token: false,
            diagnostic_new_console: false,
            diagnostic_bare_spawn: false,
        };
        let sb = SpawnRequest { env: env_b, ..sa.clone() };
        let ta = spawn_tag(&key, "t", &sa).unwrap();
        let tb = spawn_tag(&key, "t", &sb).unwrap();
        assert_eq!(ta, tb, "tag must be canonical (env-key-order independent)");
    }
}
