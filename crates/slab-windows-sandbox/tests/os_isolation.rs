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
    ElevatedAclTokenExecutor, ErasedOutputSink, FsIsolationStrength, OutputStreamKind,
    PrepareContext, SpawnRequest, WindowsSandboxError, WindowsSandboxExecutor, WindowsSetupKind,
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

/// Wrap `prepare` so that on failure we print the daemon's captured stderr/stdout log (written by
/// `launch_daemon_direct` next to the marker). The daemon otherwise hides its output behind
/// `CREATE_NO_WINDOW`, leaving provisioning failures opaque.
fn prepare_with_diag(
    exec: &ElevatedAclTokenExecutor,
    ctx: &PrepareContext,
) -> Result<(), WindowsSandboxError> {
    match exec.prepare(ctx) {
        Ok(()) => Ok(()),
        Err(e) => {
            let log = ctx.marker_path.with_file_name("daemon-error.log");
            match std::fs::read_to_string(&log) {
                Ok(contents) if !contents.is_empty() => {
                    eprintln!(
                        "=== daemon-error log ({}) ===\n{contents}=== end log ===",
                        log.display()
                    );
                }
                _ => eprintln!("(no daemon-error log at {})", log.display()),
            }
            Err(e)
        }
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
        use_conpty: false,
        diagnostic_plain_spawn: false,
        diagnostic_no_low_il_token: false,
        diagnostic_new_console: false,
    }
}

/// A capturing sink that records stdout + stderr separately so a failing test can print the
/// child's stderr (where `cmd` writes its actual error) instead of a bare empty-stdout panic.
struct Capture {
    stdout: std::sync::Mutex<Vec<u8>>,
    stderr: std::sync::Mutex<Vec<u8>>,
}
impl Capture {
    fn new() -> Self {
        Self {
            stdout: std::sync::Mutex::new(Vec::new()),
            stderr: std::sync::Mutex::new(Vec::new()),
        }
    }
    fn stdout_string(&self) -> String {
        String::from_utf8_lossy(&self.stdout.lock().unwrap()).into_owned()
    }
    fn stderr_string(&self) -> String {
        String::from_utf8_lossy(&self.stderr.lock().unwrap()).into_owned()
    }
}
impl ErasedOutputSink for Capture {
    fn on_output(&self, stream: OutputStreamKind, delta: &str) {
        let buf = match stream {
            OutputStreamKind::Stdout => &self.stdout,
            OutputStreamKind::Stderr => &self.stderr,
        };
        buf.lock().unwrap().extend_from_slice(delta.as_bytes());
    }
}

/// Regression (non-elevated, always runs when the helper is built): `launch_daemon_direct` must
/// construct a command line the helper's clap accepts — positional `serve <PIPE>`, not the
/// `--serve --pipe` flag form clap rejects (which silently exits the helper before it creates the
/// pipe, surfacing downstream as a "pipe not found" timeout). Launching the daemon + a Ping/Pong
/// handshake need NO admin (only Provision's ACL/WFP does). Self-skips if the helper binary is not
/// built next to the test binary.
#[tokio::test]
async fn launch_daemon_direct_starts_helper_and_pings() {
    let helper_exe = match resolve_helper_exe() {
        Some(p) => p,
        None => {
            eprintln!(
                "skip: slab-sandbox-helper.exe not built next to test binary \
                 (run `cargo build -p slab-sandbox-helper`)"
            );
            return;
        }
    };
    let pipe = format!(r"\\.\pipe\slab-sandbox-launch-regression-{}", std::process::id());
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join("helper.key");
    let marker_path = dir.path().join("marker.json");
    slab_windows_sandbox::launch_daemon_direct(&helper_exe, &pipe, &key_path, &marker_path)
        .expect("launch_daemon_direct spawns the helper");

    let echoed = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        slab_windows_sandbox::ping(&pipe, "regression-nonce"),
    )
    .await
    .expect(
        "timed out waiting for the daemon's pipe — the helper likely rejected the command line \
         (clap parse) and exited before creating the pipe",
    )
    .expect("ping handshake failed");
    assert_eq!(echoed, "regression-nonce");
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
    prepare_with_diag(&exec, &ctx).expect("prepare (daemon + ACLs)");

    let cap = Arc::new(Capture::new());
    let req = make_request(&["cmd", "/c", "echo slab-os-marker"], &workspace);
    let run = exec
        .spawn_elevated(&req, Some(cap.clone() as Arc<dyn ErasedOutputSink>))
        .expect("spawn_elevated");

    let exit = run.exit_future.await.expect("exit future");
    assert!(!exit.timed_out, "command timed out");
    let stdout = cap.stdout_string();
    let stderr = cap.stderr_string();
    assert!(
        stdout.contains("slab-os-marker"),
        "stdout relayed: {stdout:?}\nstderr: {stderr:?}\nexit_code: {}",
        exit.exit_code
    );
}

#[tokio::test]
#[ignore = "requires SLAB_SANDBOX_ELEVATED=1 + elevated shell; see module docs"]
async fn os_conpty_restricted_child_echo_roundtrip() {
    if !elevated_enabled() {
        eprintln!("skip: SLAB_SANDBOX_ELEVATED != 1 (run elevated)");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    let ctx = make_context(dir.path(), &workspace);

    let exec = ElevatedAclTokenExecutor::new(ctx.clone());
    prepare_with_diag(&exec, &ctx).expect("prepare (daemon + ACLs)");

    let cap = Arc::new(Capture::new());
    // ConPTY under the Low-IL AppContainer restricted token: the child sees a real pseudoconsole,
    // so echo output arrives on the merged PTY stream (pumped as Stdout).
    let mut req = make_request(&["cmd", "/c", "echo slab-os-marker"], &workspace);
    req.use_conpty = true;
    let run = exec
        .spawn_elevated(&req, Some(cap.clone() as Arc<dyn ErasedOutputSink>))
        .expect("spawn_elevated(conpty)");

    let exit = run.exit_future.await.expect("exit future");
    assert!(!exit.timed_out, "conpty command timed out");
    let stdout = cap.stdout_string();
    let stderr = cap.stderr_string();
    assert_eq!(
        exit.exit_code, 0,
        "conpty child exited cleanly\nstdout: {stdout:?}\nstderr: {stderr:?}"
    );
    assert!(
        stdout.contains("slab-os-marker"),
        "conpty stdout relayed: {stdout:?}\nstderr: {stderr:?}"
    );
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
    prepare_with_diag(&exec, &ctx).expect("prepare");

    let target = workspace.join("out.txt");
    let cap = Arc::new(Capture::new());
    // `cmd /c echo hi > out.txt` — the Low-IL child CAN write inside the lowered workspace. The file
    // is a side-effect INDEPENDENT of the stdio pipes, so it is the decisive probe for whether cmd
    // actually executed under the AppContainer token (vs. failing to start with no output).
    let req = make_request(&["cmd", "/c", "echo hi", ">", target.to_str().unwrap()], &workspace);
    let run = exec
        .spawn_elevated(&req, Some(cap.clone() as Arc<dyn ErasedOutputSink>))
        .expect("spawn_elevated");
    let exit = run.exit_future.await.expect("exit future");
    let file_created = target.exists();
    let file_contents = std::fs::read(&target)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default();
    let stderr = cap.stderr_string();
    let stdout = cap.stdout_string();
    eprintln!(
        "writes_inside diag: exit_code={}, file_created={}, file_contents={:?}, stderr={:?}, stdout={:?}",
        exit.exit_code, file_created, file_contents, stderr, stdout
    );
    assert_eq!(exit.exit_code, 0, "in-workspace write should succeed (see diag above)");
    assert!(file_created, "workspace write produced the file (see diag above)");
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
    prepare_with_diag(&exec, &ctx).expect("prepare");

    // Write OUTSIDE the workspace (into the parent temp dir, which is Medium-IL). The Low-IL
    // child's NO_WRITE_UP must block this: the file is not created.
    let outside = dir.path().join("escape.txt");
    let req = make_request(&["cmd", "/c", "echo bad", ">", outside.to_str().unwrap()], &workspace);
    let run = exec.spawn_elevated(&req, None).expect("spawn_elevated");
    let _exit = run.exit_future.await.expect("exit future");
    assert!(!outside.exists(), "Low-IL child must NOT write outside the workspace");
}

#[tokio::test]
#[ignore = "requires SLAB_SANDBOX_ELEVATED=1 + elevated shell; see module docs"]
async fn os_capabilities_report_wfp_os_enforced() {
    if !elevated_enabled() {
        eprintln!("skip: SLAB_SANDBOX_ELEVATED != 1");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    let ctx = make_context(dir.path(), &workspace);
    let exec = ElevatedAclTokenExecutor::new(ctx.clone());
    prepare_with_diag(&exec, &ctx).expect("prepare");

    // After provisioning (AppContainer spawn + WFP filter registered) the honest report must show
    // BOTH dimensions OS-enforced under the WFP setup kind.
    let caps = exec.capabilities();
    assert!(caps.provisioned, "provisioned after prepare");
    assert_eq!(caps.setup_kind, WindowsSetupKind::ElevatedAclTokenWfp);
    assert_eq!(caps.network_isolation, FsIsolationStrength::OsEnforced);
    assert_eq!(caps.filesystem_isolation, FsIsolationStrength::OsEnforced);
}

#[tokio::test]
#[ignore = "requires SLAB_SANDBOX_ELEVATED=1 + elevated shell; see module docs"]
async fn os_appcontainer_child_network_blocked() {
    if !elevated_enabled() {
        eprintln!("skip: SLAB_SANDBOX_ELEVATED != 1");
        return;
    }
    // Probe egress with curl.exe (ships in System32 on Windows 10+). If absent, skip rather than
    // produce a misleading pass (a not-found spawn also exits non-zero).
    let curl = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"))
        .join("System32")
        .join("curl.exe");
    if !curl.exists() {
        eprintln!("skip: System32\\curl.exe not found (needed to probe network egress)");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    let ctx = make_context(dir.path(), &workspace);
    let exec = ElevatedAclTokenExecutor::new(ctx.clone());
    prepare_with_diag(&exec, &ctx).expect("prepare");

    // The AppContainer child has no internet capability + our WFP package-SID block filter, so its
    // outbound connect must fail. curl exits 0 ONLY on a completed HTTP request. Capture output +
    // print the exit code: 6/7/28 ⇒ curl ran and the network was blocked; 1/other ⇒ curl failed to
    // init under the AppContainer token (the cmd tests show the same, so this distinguishes
    // cmd-specific failure from a token-wide inability to run any binary).
    let cap = Arc::new(Capture::new());
    let req = make_request(
        &["curl.exe", "--max-time", "8", "-s", "-o", "NUL", "http://example.com"],
        &workspace,
    );
    let run = exec
        .spawn_elevated(&req, Some(cap.clone() as Arc<dyn ErasedOutputSink>))
        .expect("spawn_elevated");
    let exit = run.exit_future.await.expect("exit future");
    eprintln!(
        "network_blocked diag: curl exit_code={}, stdout={:?}, stderr={:?}",
        exit.exit_code,
        cap.stdout_string(),
        cap.stderr_string()
    );
    assert_ne!(
        exit.exit_code, 0,
        "AppContainer child must NOT complete outbound HTTP (network OS-blocked)"
    );
}

/// Diagnostic probe: can a simple, non-cmd binary run AND produce output under the AppContainer
/// token? `whoami.exe` (System32) exits 0 + prints the user identity on stdout. Combined with the
/// curl + cmd results this disambiguates the spawn failures:
///   - exit=0 + output present  ⇒ the token runs simple programs and the stdio relay works ⇒ the
///     cmd failures are cmd-specific init failures.
///   - exit=0 + no output       ⇒ simple programs run but the stdio relay is broken.
///   - exit!=0                  ⇒ simple programs fail to init too ⇒ token-wide problem.
#[tokio::test]
#[ignore = "requires SLAB_SANDBOX_ELEVATED=1 + elevated shell; see module docs"]
async fn os_appcontainer_runs_simple_binary() {
    if !elevated_enabled() {
        eprintln!("skip: SLAB_SANDBOX_ELEVATED != 1");
        return;
    }
    let whoami = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"))
        .join("System32")
        .join("whoami.exe");
    if !whoami.exists() {
        eprintln!("skip: System32\\whoami.exe not found");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    let ctx = make_context(dir.path(), &workspace);
    let exec = ElevatedAclTokenExecutor::new(ctx.clone());
    prepare_with_diag(&exec, &ctx).expect("prepare");

    let cap = Arc::new(Capture::new());
    // Full path as argv[0] so CreateProcessAsUserW resolves it directly from System32 (no PATH
    // search). whoami exits 0 and prints the user identity.
    let whoami_str = whoami.to_string_lossy().into_owned();
    let req = make_request(&[whoami_str.as_str()], &workspace);
    let run = exec
        .spawn_elevated(&req, Some(cap.clone() as Arc<dyn ErasedOutputSink>))
        .expect("spawn_elevated");
    let exit = run.exit_future.await.expect("exit future");
    eprintln!(
        "simple_binary diag: whoami exit_code={}, stdout={:?}, stderr={:?}",
        exit.exit_code,
        cap.stdout_string(),
        cap.stderr_string()
    );
    assert_eq!(exit.exit_code, 0, "whoami should run (see diag above)");
    assert!(!cap.stdout_string().is_empty(), "whoami should print (see diag above)");
}

/// DIAGNOSTIC companion to `os_appcontainer_runs_simple_binary`: spawn whoami with
/// `diagnostic_plain_spawn` (Low-IL token, NO AppContainer identity, NO CREATE_NO_WINDOW). Compared
/// with the AppContainer whoami result this isolates the init-failure cause:
///   - runs here but fails under AppContainer ⇒ AppContainer identity / CREATE_NO_WINDOW is the
///     culprit.
///   - fails here too ⇒ the Low-IL token itself cannot run console apps.
#[tokio::test]
#[ignore = "requires SLAB_SANDBOX_ELEVATED=1 + elevated shell; see module docs"]
async fn os_diagnostic_plain_low_il_whoami() {
    if !elevated_enabled() {
        eprintln!("skip: SLAB_SANDBOX_ELEVATED != 1");
        return;
    }
    let whoami = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"))
        .join("System32")
        .join("whoami.exe");
    if !whoami.exists() {
        eprintln!("skip: System32\\whoami.exe not found");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    let ctx = make_context(dir.path(), &workspace);
    let exec = ElevatedAclTokenExecutor::new(ctx.clone());
    prepare_with_diag(&exec, &ctx).expect("prepare");

    let cap = Arc::new(Capture::new());
    let whoami_str = whoami.to_string_lossy().into_owned();
    let mut req = make_request(&[whoami_str.as_str()], &workspace);
    req.diagnostic_plain_spawn = true;
    let run = exec
        .spawn_elevated(&req, Some(cap.clone() as Arc<dyn ErasedOutputSink>))
        .expect("spawn_elevated");
    let exit = run.exit_future.await.expect("exit future");
    eprintln!(
        "plain_low_il diag: whoami exit_code={}, stdout={:?}, stderr={:?}",
        exit.exit_code,
        cap.stdout_string(),
        cap.stderr_string()
    );
    assert_eq!(exit.exit_code, 0, "whoami should run under plain Low-IL (see diag above)");
    assert!(!cap.stdout_string().is_empty(), "whoami should print (see diag above)");
}

/// DIAGNOSTIC: spawn whoami with `diagnostic_plain_spawn` + `diagnostic_no_low_il_token` — i.e.
/// `CreateProcessW` with the daemon's OWN token (no Low-IL restriction), no AppContainer. The
/// least-restrictive elevated spawn. If whoami runs here but not under the LowIntegrityToken, the
/// Low-IL token is definitively the init-failure cause (and the fix is to drop it / use the standard
/// kernel-derived AppContainer token).
#[tokio::test]
#[ignore = "requires SLAB_SANDBOX_ELEVATED=1 + elevated shell; see module docs"]
async fn os_diagnostic_no_token_whoami() {
    if !elevated_enabled() {
        eprintln!("skip: SLAB_SANDBOX_ELEVATED != 1");
        return;
    }
    let whoami = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"))
        .join("System32")
        .join("whoami.exe");
    if !whoami.exists() {
        eprintln!("skip: System32\\whoami.exe not found");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    let ctx = make_context(dir.path(), &workspace);
    let exec = ElevatedAclTokenExecutor::new(ctx.clone());
    prepare_with_diag(&exec, &ctx).expect("prepare");

    let cap = Arc::new(Capture::new());
    let whoami_str = whoami.to_string_lossy().into_owned();
    let mut req = make_request(&[whoami_str.as_str()], &workspace);
    req.diagnostic_plain_spawn = true;
    req.diagnostic_no_low_il_token = true;
    let run = exec
        .spawn_elevated(&req, Some(cap.clone() as Arc<dyn ErasedOutputSink>))
        .expect("spawn_elevated");
    let exit = run.exit_future.await.expect("exit future");
    eprintln!(
        "no_token diag: whoami exit_code={}, stdout={:?}, stderr={:?}",
        exit.exit_code,
        cap.stdout_string(),
        cap.stderr_string()
    );
    assert_eq!(
        exit.exit_code, 0,
        "whoami should run with the daemon's normal token (see diag above)"
    );
    assert!(!cap.stdout_string().is_empty(), "whoami should print (see diag above)");
}

/// DIAGNOSTIC: like `os_diagnostic_no_token_whoami` but the child INHERITS the daemon's full
/// environment (empty env map ⇒ no env block ⇒ inherit). The spawn env is otherwise {SystemRoot}
/// only — a near-empty env can break console-app CRT init. If whoami runs here but not under the
/// {SystemRoot}-only env, the minimal environment was the init-failure cause.
#[tokio::test]
#[ignore = "requires SLAB_SANDBOX_ELEVATED=1 + elevated shell; see module docs"]
async fn os_diagnostic_no_token_inherit_env() {
    if !elevated_enabled() {
        eprintln!("skip: SLAB_SANDBOX_ELEVATED != 1");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    let ctx = make_context(dir.path(), &workspace);
    let exec = ElevatedAclTokenExecutor::new(ctx.clone());
    prepare_with_diag(&exec, &ctx).expect("prepare");

    let cap = Arc::new(Capture::new());
    let mut req = make_request(&["C:\\Windows\\System32\\whoami.exe"], &workspace);
    req.diagnostic_plain_spawn = true;
    req.diagnostic_no_low_il_token = true;
    req.env.clear(); // inherit the daemon's full environment
    let run = exec
        .spawn_elevated(&req, Some(cap.clone() as Arc<dyn ErasedOutputSink>))
        .expect("spawn_elevated");
    let exit = run.exit_future.await.expect("exit future");
    eprintln!(
        "inherit_env diag: whoami exit_code={}, stdout={:?}, stderr={:?}",
        exit.exit_code,
        cap.stdout_string(),
        cap.stderr_string()
    );
    assert_eq!(exit.exit_code, 0, "whoami should run with inherited env (see diag above)");
    assert!(!cap.stdout_string().is_empty(), "whoami should print (see diag above)");
}

/// DIAGNOSTIC: like `os_diagnostic_no_token_whoami` but the child gets its OWN console
/// (`CREATE_NEW_CONSOLE`) instead of inheriting the daemon's (none). Tests whether the no-console
/// condition aborts console-app init. A console window may flash — that is expected.
#[tokio::test]
#[ignore = "requires SLAB_SANDBOX_ELEVATED=1 + elevated shell; see module docs"]
async fn os_diagnostic_no_token_new_console() {
    if !elevated_enabled() {
        eprintln!("skip: SLAB_SANDBOX_ELEVATED != 1");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    let ctx = make_context(dir.path(), &workspace);
    let exec = ElevatedAclTokenExecutor::new(ctx.clone());
    prepare_with_diag(&exec, &ctx).expect("prepare");

    let cap = Arc::new(Capture::new());
    let mut req = make_request(&["C:\\Windows\\System32\\whoami.exe"], &workspace);
    req.diagnostic_plain_spawn = true;
    req.diagnostic_no_low_il_token = true;
    req.diagnostic_new_console = true;
    let run = exec
        .spawn_elevated(&req, Some(cap.clone() as Arc<dyn ErasedOutputSink>))
        .expect("spawn_elevated");
    let exit = run.exit_future.await.expect("exit future");
    eprintln!(
        "new_console diag: whoami exit_code={}, stdout={:?}, stderr={:?}",
        exit.exit_code,
        cap.stdout_string(),
        cap.stderr_string()
    );
    assert_eq!(exit.exit_code, 0, "whoami should run with a new console (see diag above)");
    assert!(!cap.stdout_string().is_empty(), "whoami should print (see diag above)");
}
