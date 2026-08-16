//! Gated OS-level isolation tests for `slab-linux-sandbox`. These exercise the real kernel
//! mechanisms (bwrap bind mounts, seccomp network filter, landlock path-access) and therefore
//! require Linux with `bwrap` installed and a landlock-capable kernel (≥ 5.13).
//!
//! Each test is `#[ignore]` and self-skips unless `SLAB_SANDBOX_LINUX=1` is set, so a plain
//! `cargo test` never depends on root/bwrap/landlock. Run them explicitly:
//!
//! ```sh
//! SLAB_SANDBOX_LINUX=1 cargo test -p slab-linux-sandbox --test os_isolation -- --ignored --nocapture
//! ```
//!
//! On a non-Linux host this file compiles to nothing (`#![cfg(target_os = "linux")]`).

#![cfg(target_os = "linux")]

use std::path::PathBuf;

use slab_linux_sandbox::{
    CapabilitySnapshot, FsIsolationStrength, LandlockFallbackExecutor, LinuxSandboxExecutor,
    LinuxSetupKind, SandboxPolicyMirror, SpawnRequest, select_executor,
};

/// Gate: only run when the operator explicitly opts in. Avoids accidental CI runs that need
/// root/bwrap/landlock.
fn linux_sandbox_enabled() -> bool {
    std::env::var("SLAB_SANDBOX_LINUX").ok().as_deref() == Some("1")
}

fn skip_unless_enabled() -> bool {
    if !linux_sandbox_enabled() {
        eprintln!(
            "skip: set SLAB_SANDBOX_LINUX=1 (and ensure bwrap + landlock kernel ≥ 5.13) to run"
        );
        return true;
    }
    false
}

fn ws_request(network_blocked: bool) -> SpawnRequest {
    SpawnRequest {
        argv: vec!["/bin/sh".into(), "-c".into(), "true".into()],
        env: Default::default(),
        cwd: None,
        network_blocked,
        managed_proxy_active: false,
        sandbox_policy: SandboxPolicyMirror::WorkspaceWrite,
        workspace_root: Some(PathBuf::from("/tmp")),
        writable_roots: vec![],
        readable_roots: vec![],
        denied_paths: vec![],
        protected_path_names: vec![".git".into()],
    }
}

/// Run `argv` under the given executor and return the WaitStatus-like outcome via the child.
async fn run_argv(
    exec: &dyn LinuxSandboxExecutor,
    argv: Vec<String>,
    req: &SpawnRequest,
) -> Option<i32> {
    let mut spawn_req = req.clone();
    spawn_req.argv = argv;
    let spawned = exec.spawn(&spawn_req).ok()?;
    let mut child = spawned.child;
    // Dropping kill_tree after the child exits is the intended lifecycle.
    let _ = spawned.kill_tree;
    child.wait().await.ok()?.code()
}

#[tokio::test]
#[ignore = "requires SLAB_SANDBOX_LINUX=1 + Linux + bwrap; see module docs"]
async fn os_bwrap_seccomp_child_network_blocked() {
    if skip_unless_enabled() {
        return;
    }
    // bwrap+seccomp with network blocked ⇒ a Python AF_INET socket() is killed (SIGSYS ⇒ exit).
    let exec = select_executor(true, true, false);
    let req = ws_request(true);
    let code = run_argv(
        exec.as_ref(),
        vec![
            "/bin/sh".into(),
            "-c".into(),
            "python3 -c \"import socket; socket.socket(socket.AF_INET).connect(('example.com', 80))\""
                .into(),
        ],
        &req,
    )
    .await;
    // Killed-by-signal processes report no exit code (None) or a non-zero code; either is a pass.
    // A successful connect (exit 0) would be a failure.
    assert!(
        code.map(|c| c != 0).unwrap_or(true),
        "network syscall must be blocked (got exit code {code:?})"
    );
}

#[tokio::test]
#[ignore = "requires SLAB_SANDBOX_LINUX=1 + Linux + bwrap; see module docs"]
async fn os_bwrap_seccomp_af_unix_socket_not_killed() {
    if skip_unless_enabled() {
        return;
    }
    // The AF_UNIX exemption lets libc/init open Unix sockets without being killed.
    let exec = select_executor(true, true, false);
    let req = ws_request(true);
    let code = run_argv(
        exec.as_ref(),
        vec![
            "/bin/sh".into(),
            "-c".into(),
            "python3 -c \"import socket; socket.socket(socket.AF_UNIX)\"".into(),
        ],
        &req,
    )
    .await;
    assert_eq!(code, Some(0), "AF_UNIX socket creation must survive");
}

#[tokio::test]
#[ignore = "requires SLAB_SANDBOX_LINUX=1 + Linux + landlock; see module docs"]
async fn os_landlock_fallback_writes_outside_denied() {
    if skip_unless_enabled() {
        return;
    }
    // Force the landlock path directly (bwrap may also be present). Writable root is /tmp; writing
    // outside it (e.g. /etc) must be denied by the kernel.
    let exec = LandlockFallbackExecutor::new(true, false);
    let mut req = ws_request(true);
    req.workspace_root = Some(PathBuf::from("/tmp"));
    let code = run_argv(
        &exec,
        vec![
            "/bin/sh".into(),
            "-c".into(),
            "echo x > /etc/slab-landlock-test 2>/dev/null; test -f /etc/slab-landlock-test".into(),
        ],
        &req,
    )
    .await;
    // The write should be denied ⇒ the file does not exist ⇒ `test -f` fails (non-zero).
    assert!(
        code.map(|c| c != 0).unwrap_or(true),
        "landlock must deny writes outside the writable root (got {code:?})"
    );
}

#[test]
#[ignore = "requires SLAB_SANDBOX_LINUX=1 + Linux; see module docs"]
fn os_capabilities_report_bwrap_seccomp_when_available() {
    if skip_unless_enabled() {
        return;
    }
    let exec = select_executor(true, true, false);
    let cap = exec.capabilities();
    // With bwrap available, setup_kind must be BwrapSeccomp and fs OsEnforced.
    assert_eq!(cap.filesystem_isolation, FsIsolationStrength::OsEnforced);
    assert_eq!(cap.setup_kind, LinuxSetupKind::BwrapSeccomp);
    assert!(cap.provisioned);
    assert_eq!(cap.network_isolation, FsIsolationStrength::OsEnforced);
}

#[test]
#[ignore = "requires SLAB_SANDBOX_LINUX=1 + Linux; see module docs"]
fn os_capabilities_managed_proxy_degrades_network() {
    if skip_unless_enabled() {
        return;
    }
    // managed_proxy_active ⇒ network not enforced ⇒ Lexical, degraded.
    let exec = select_executor(true, true, true);
    let cap: CapabilitySnapshot = exec.capabilities();
    // Either bwrap or landlock path; both report fs OsEnforced + network Lexical under a proxy.
    assert_eq!(cap.network_isolation, FsIsolationStrength::Lexical);
    assert!(cap.degraded);
    assert_eq!(cap.filesystem_isolation, FsIsolationStrength::OsEnforced);
}
