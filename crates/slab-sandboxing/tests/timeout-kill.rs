//! Timeout tree-kill regressions.
//!
//! Reproduces the production shell path (`bash -lc "<cmd>"` through the real
//! platform driver) with a timeout shorter than the command's runtime. The
//! contract under test:
//!
//! 1. On timeout the whole process tree dies — a `sleep` that outlives the
//!    deadline must never let a *later* command in the same shell invocation
//!    run and leak its output into the captured stdout (the
//!    `slow_command; echo marker` escape hatch).
//! 2. Output produced BEFORE the deadline (already flowing through the pipe)
//!    is still captured.
//!
//! Skips when no POSIX shell (incl. Git Bash on Windows) is on PATH.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use slab_sandboxing::{
    SandboxDriver, SandboxEnvironment, SandboxPolicy, SandboxedCommand, create_platform_driver,
};
use tempfile::TempDir;

fn timeout_workspace() -> Option<(TempDir, std::sync::Arc<dyn SandboxDriver>)> {
    let workspace = tempfile::tempdir().expect("temp workspace");
    let env = SandboxEnvironment::new(
        Some(workspace.path().to_path_buf()),
        SandboxPolicy::WorkspaceWrite,
    );
    let driver = create_platform_driver(env).expect("platform sandbox driver");
    let status = driver.setup_status();
    if !status.available {
        if std::env::var("SLAB_SANDBOX_SMOKE_ALLOW_SKIP").ok().as_deref() == Some("1") {
            eprintln!("skipping timeout-kill: {}", status.details);
            return None;
        }
        panic!("{}", status.details);
    }
    Some((workspace, driver))
}

/// A command that finishes well after the timeout, followed by an `echo` whose
/// output must never be captured: if the shell survives the timeout kill, the
/// marker proves it (this is the exact `slow_command; dangerous_command` shape
/// the timeout mechanism must cut off).
#[tokio::test]
async fn timeout_kills_tree_before_post_deadline_echo_runs() {
    let Some(bash) = find_posix_shell() else {
        eprintln!("skipping: no bash/sh found on PATH");
        return;
    };
    let Some((workspace, driver)) = timeout_workspace() else {
        return;
    };

    let cmd = SandboxedCommand {
        argv: vec![
            bash.to_string_lossy().into_owned(),
            "-lc".to_string(),
            "sleep 45; echo SLAB_TIMEOUT_LEAK".to_string(),
        ],
        env: HashMap::new(),
        cwd: Some(workspace.path().to_path_buf()),
        timeout: Some(Duration::from_secs(5)),
        output_sink: None,
    };

    let started = std::time::Instant::now();
    let output = tokio::time::timeout(Duration::from_secs(20), driver.run(cmd))
        .await
        .expect("driver.run hung past 20s (timeout kill deadlock?)")
        .expect("run");
    let elapsed = started.elapsed();
    let stdout = String::from_utf8_lossy(&output.stdout);
    eprintln!("timeout-kill elapsed={elapsed:?} exit={} stdout={stdout:?}", output.exit_code);

    assert!(output.timed_out, "expected timed_out=true");
    assert_eq!(output.exit_code, 124, "expected the fixed timeout exit code");
    assert!(
        !stdout.contains("SLAB_TIMEOUT_LEAK"),
        "post-deadline output leaked into stdout — the shell outlived the timeout kill"
    );
    // 5s deadline + kill + bounded grace; anything near 45s means the kill
    // never landed and we merely waited the command out.
    assert!(elapsed < Duration::from_secs(20), "run took {elapsed:?} — tree kill did not land?");
}

/// Output written before the deadline must survive the timeout kill (the
/// partial output is part of the contract: the model gets what the command
/// produced before being cut off).
#[tokio::test]
async fn timeout_preserves_pre_deadline_output() {
    let Some(bash) = find_posix_shell() else {
        eprintln!("skipping: no bash/sh found on PATH");
        return;
    };
    let Some((workspace, driver)) = timeout_workspace() else {
        return;
    };

    let cmd = SandboxedCommand {
        argv: vec![
            bash.to_string_lossy().into_owned(),
            "-lc".to_string(),
            "echo slab-early-marker; sleep 45".to_string(),
        ],
        env: HashMap::new(),
        cwd: Some(workspace.path().to_path_buf()),
        timeout: Some(Duration::from_secs(2)),
        output_sink: None,
    };

    let output = tokio::time::timeout(Duration::from_secs(20), driver.run(cmd))
        .await
        .expect("driver.run hung past 20s")
        .expect("run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    eprintln!("timeout-early exit={} stdout={stdout:?}", output.exit_code);

    assert!(output.timed_out, "expected timed_out=true");
    assert_eq!(output.exit_code, 124, "expected the fixed timeout exit code");
    assert!(stdout.contains("slab-early-marker"), "pre-deadline output was dropped: {stdout:?}");
}

fn find_posix_shell() -> Option<PathBuf> {
    for name in ["bash", "sh"] {
        if let Some(p) = which(name) {
            return Some(p);
        }
    }
    None
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        let exe = dir.join(format!("{name}.exe"));
        if exe.is_file() {
            return Some(exe);
        }
    }
    None
}
