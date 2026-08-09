//! Gated OS-level isolation tests for the S2b2 elevated sandbox. These are `#[ignore]` AND
//! self-skip unless `SLAB_SANDBOX_ELEVATED=1`, because they require:
//!   - the whole test process to run elevated (so `prepare()` starts the daemon via
//!     `launch_daemon_direct`, no UAC), AND
//!   - a freshly built `slab-sandbox-helper.exe` next to the test binary.
//!
//! Run them manually from an elevated (admin) shell:
//!   `SLAB_SANDBOX_ELEVATED=1 cargo test -p slab-windows-sandbox --test os_isolation -- --ignored --nocapture`
//!
//! The non-elevated `cargo test` run never executes them (they self-skip), so the suite stays green.

#![cfg(target_os = "windows")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use slab_windows_sandbox::{
    ElevatedAclTokenExecutor, ErasedOutputSink, OutputStreamKind, PrepareContext, SpawnRequest,
    WindowsSandboxExecutor,
};

fn elevated_enabled() -> bool {
    std::env::var("SLAB_SANDBOX_ELEVATED").map(|v| v == "1").unwrap_or(false)
}

/// Resolve the helper binary: next to the test binary's parent (target/debug) or beside current_exe.
fn resolve_helper_exe() -> Option<PathBuf> {
    let mut exe = std::env::current_exe().ok()?;
    exe.pop(); // .../deps
    exe.pop(); // target/debug
    exe.push("slab-sandbox-helper.exe");
    if exe.exists() { Some(exe) } else { None }
}

fn make_context(dir: &std::path::Path, workspace: &std::path::Path) -> PrepareContext {
    let helper_exe = resolve_helper_exe().unwrap_or_else(|| {
        panic!(
            "slab-sandbox-helper.exe not found next to the test binary; build it with \
             `cargo build -p slab-sandbox-helper` first"
        )
    });
    PrepareContext {
        workspace_root: Some(workspace.to_path_buf()),
        denied_paths: vec![],
        denied_globs: vec![],
        writable_roots: vec![workspace.to_path_buf()],
        network_blocked: false,
        helper_exe,
        key_path: dir.join("sandbox-helper.key"),
        ipc_dir: dir.join("sandbox-ipc"),
        marker_path: dir.join("sandbox-marker.json"),
    }
}

fn make_request(argv: &[&str], cwd: &std::path::Path) -> SpawnRequest {
    let mut env = HashMap::new();
    if let Ok(sysroot) = std::env::var("SystemRoot") {
        env.insert("SystemRoot".to_string(), sysroot);
    }
    SpawnRequest {
        argv: argv.iter().map(|s| s.to_string()).collect(),
        env,
        cwd: Some(cwd.to_path_buf()),
        denied_paths: vec![],
        denied_globs: vec![],
        writable_roots: vec![],
        workspace_root: None,
        network_blocked: false,
    }
}

/// A capturing sink so the test can assert output relay.
struct Capture(std::sync::Mutex<Vec<u8>>);
impl ErasedOutputSink for Capture {
    fn on_output(&self, stream: OutputStreamKind, delta: &str) {
        if matches!(stream, OutputStreamKind::Stdout) {
            self.0.lock().unwrap().extend_from_slice(delta.as_bytes());
        }
    }
}

#[tokio::test]
#[ignore = "requires SLAB_SANDBOX_ELEVATED=1 + elevated shell; see module docs"]
async fn os_elevated_prepare_and_spawn_relays_stdout() {
    if !elevated_enabled() {
        eprintln!("skip: SLAB_SANDBOX_ELEVATED != 1 (run elevated)");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    let ctx = make_context(dir.path(), &workspace);

    let exec = ElevatedAclTokenExecutor::new(ctx.clone());
    exec.prepare(&ctx).expect("prepare (daemon + ACLs)");

    let cap = Arc::new(Capture(std::sync::Mutex::new(Vec::new())));
    let req = make_request(&["cmd", "/c", "echo slab-os-marker"], &workspace);
    let run = exec
        .spawn_elevated(&req, Some(cap.clone() as Arc<dyn ErasedOutputSink>))
        .expect("spawn_elevated");

    let exit = run.exit_future.await.expect("exit future");
    assert!(!exit.timed_out, "command timed out");
    let stdout = {
        let buf = cap.0.lock().unwrap();
        String::from_utf8_lossy(&buf).into_owned()
    };
    assert!(stdout.contains("slab-os-marker"), "stdout relayed: {stdout}");
}

#[tokio::test]
#[ignore = "requires SLAB_SANDBOX_ELEVATED=1 + elevated shell; see module docs"]
async fn os_low_il_child_writes_inside_workspace() {
    if !elevated_enabled() {
        eprintln!("skip: SLAB_SANDBOX_ELEVATED != 1");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    let ctx = make_context(dir.path(), &workspace);
    let exec = ElevatedAclTokenExecutor::new(ctx.clone());
    exec.prepare(&ctx).expect("prepare");

    let target = workspace.join("out.txt");
    // `cmd /c echo hi > out.txt` — the Low-IL child CAN write inside the lowered workspace.
    let req = make_request(&["cmd", "/c", "echo hi", ">", target.to_str().unwrap()], &workspace);
    let run = exec.spawn_elevated(&req, None).expect("spawn_elevated");
    let exit = run.exit_future.await.expect("exit future");
    assert_eq!(exit.exit_code, 0, "in-workspace write should succeed");
    assert!(target.exists(), "workspace write produced the file");
}

#[tokio::test]
#[ignore = "requires SLAB_SANDBOX_ELEVATED=1 + elevated shell; see module docs"]
async fn os_low_il_child_blocked_outside_workspace() {
    if !elevated_enabled() {
        eprintln!("skip: SLAB_SANDBOX_ELEVATED != 1");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    let ctx = make_context(dir.path(), &workspace);
    let exec = ElevatedAclTokenExecutor::new(ctx.clone());
    exec.prepare(&ctx).expect("prepare");

    // Write OUTSIDE the workspace (into the parent temp dir, which is Medium-IL). The Low-IL
    // child's NO_WRITE_UP must block this: the file is not created.
    let outside = dir.path().join("escape.txt");
    let req = make_request(&["cmd", "/c", "echo bad", ">", outside.to_str().unwrap()], &workspace);
    let run = exec.spawn_elevated(&req, None).expect("spawn_elevated");
    let _exit = run.exit_future.await.expect("exit future");
    assert!(!outside.exists(), "Low-IL child must NOT write outside the workspace");
}
