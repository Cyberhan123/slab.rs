use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use slab_sandboxing::{
    NetworkPolicy, SandboxDriver, SandboxEnvironment, SandboxError, SandboxPolicy,
    SandboxedCommand, create_platform_driver,
};
use tempfile::TempDir;

fn smoke_workspace(policy: SandboxPolicy) -> Option<(TempDir, std::sync::Arc<dyn SandboxDriver>)> {
    let workspace = tempfile::tempdir().expect("temp workspace");
    let env = SandboxEnvironment::new(Some(workspace.path().to_path_buf()), policy);
    let driver = smoke_driver(env)?;
    Some((workspace, driver))
}

fn smoke_driver(env: SandboxEnvironment) -> Option<std::sync::Arc<dyn SandboxDriver>> {
    let driver = create_platform_driver(env).expect("platform sandbox driver");
    let status = driver.setup_status();
    if !status.available {
        if std::env::var("SLAB_SANDBOX_SMOKE_ALLOW_SKIP").ok().as_deref() == Some("1") {
            eprintln!("skipping sandbox smoke: {}", status.details);
            return None;
        }
        panic!("{}", status.details);
    }
    Some(driver)
}

#[tokio::test]
async fn platform_driver_reports_capabilities() {
    let Some((_workspace, driver)) = smoke_workspace(SandboxPolicy::WorkspaceWrite) else {
        return;
    };
    let capabilities = driver.capabilities();

    assert!(driver.setup_status().available);
    assert!(capabilities.filesystem || capabilities.isolation as u8 > 0);
}

#[tokio::test]
async fn platform_driver_streams_output_through_sink() {
    let Some((workspace, driver)) = smoke_workspace(SandboxPolicy::WorkspaceWrite) else {
        return;
    };

    #[derive(Clone)]
    struct CapturingSink(std::sync::Arc<std::sync::Mutex<String>>);
    impl slab_sandboxing::OutputSink for CapturingSink {
        fn on_output(&self, _stream: slab_sandboxing::OutputStream, delta: &str) {
            *self.0.lock().unwrap() += delta;
        }
    }
    let captured = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let sink = std::sync::Arc::new(CapturingSink(captured.clone()))
        as std::sync::Arc<dyn slab_sandboxing::OutputSink>;

    let mut cmd = shell_command("echo slab-smoke-marker", workspace.path());
    cmd.output_sink = Some(sink);

    // Wrap in a hard timeout so a deadlock fails the test instead of hanging.
    let output = tokio::time::timeout(Duration::from_secs(15), driver.run(cmd))
        .await
        .expect("driver.run hung past 15s (streaming deadlock?)")
        .expect("run");
    let streamed = captured.lock().unwrap().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    eprintln!("streamed={streamed:?} stdout={stdout:?} exit={}", output.exit_code);
    assert!(
        streamed.contains("slab-smoke-marker") || stdout.contains("slab-smoke-marker"),
        "neither stream nor stdout carried the marker"
    );
}

/// Reproduce the production shell path: `bash -lc "<cmd>"` through the real
/// platform driver with a streaming sink. Skips when no POSIX shell is found.
#[tokio::test]
async fn platform_driver_streams_bash_lc_command_with_sink() {
    let Some(bash) = find_posix_shell() else {
        eprintln!("skipping: no bash/sh found on PATH");
        return;
    };
    let Some((workspace, driver)) = smoke_workspace(SandboxPolicy::WorkspaceWrite) else {
        return;
    };

    #[derive(Clone)]
    struct CapturingSink(std::sync::Arc<std::sync::Mutex<String>>);
    impl slab_sandboxing::OutputSink for CapturingSink {
        fn on_output(&self, _stream: slab_sandboxing::OutputStream, delta: &str) {
            *self.0.lock().unwrap() += delta;
        }
    }
    let captured = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let sink = std::sync::Arc::new(CapturingSink(captured.clone()))
        as std::sync::Arc<dyn slab_sandboxing::OutputSink>;

    let cmd = SandboxedCommand {
        argv: vec![bash.to_string_lossy().into_owned(), "-lc".to_string(), "date +%A".to_string()],
        env: HashMap::new(),
        cwd: Some(workspace.path().to_path_buf()),
        timeout: Some(Duration::from_secs(10)),
        output_sink: Some(sink),
    };

    let output = tokio::time::timeout(Duration::from_secs(20), driver.run(cmd))
        .await
        .expect("driver.run hung past 20s (bash streaming deadlock?)")
        .expect("run");
    let streamed = captured.lock().unwrap().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    eprintln!("bash streamed={streamed:?} stdout={stdout:?} exit={}", output.exit_code);
    assert!(output.exit_code == 0, "stderr={}", String::from_utf8_lossy(&output.stderr));
    assert!(!stdout.trim().is_empty(), "expected weekday from `date +%A`");
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

#[tokio::test]
async fn read_only_denies_workspace_write() {
    let Some((workspace, driver)) = smoke_workspace(SandboxPolicy::ReadOnly) else {
        return;
    };
    let target = workspace.path().join("ro-denied.txt");

    let result = driver.run(shell_command("echo denied > ro-denied.txt", workspace.path())).await;

    assert!(matches!(result, Err(SandboxError::PermissionDenied(_))));
    assert!(!target.exists());
}

#[tokio::test]
async fn workspace_write_allows_workspace_write() {
    let Some((workspace, driver)) = smoke_workspace(SandboxPolicy::WorkspaceWrite) else {
        return;
    };
    let target = workspace.path().join("allowed.txt");

    let output = driver
        .run(shell_command("echo allowed > allowed.txt", workspace.path()))
        .await
        .expect("workspace write should run");

    assert_eq!(output.exit_code, 0, "stderr={}", output.stderr_str());
    assert!(target.exists());
}

#[tokio::test]
async fn workspace_write_denies_outside_write() {
    let Some((workspace, driver)) = smoke_workspace(SandboxPolicy::WorkspaceWrite) else {
        return;
    };
    let outside =
        std::env::current_dir().expect("cwd").join("target").join("sandbox-outside-smoke");
    std::fs::create_dir_all(&outside).expect("outside dir");
    let target = outside.join("blocked.txt");
    let _ = std::fs::remove_file(&target);
    let command = format!("echo blocked > {}", shell_path(&target));

    let result = driver.run(shell_command(&command, workspace.path())).await;

    assert!(matches!(result, Err(SandboxError::PermissionDenied(_))));
    assert!(!target.exists());
}

#[tokio::test]
async fn workspace_write_denies_protected_metadata_write() {
    let Some((workspace, driver)) = smoke_workspace(SandboxPolicy::WorkspaceWrite) else {
        return;
    };
    std::fs::create_dir_all(workspace.path().join(".GiT")).expect("metadata dir");
    let target = workspace.path().join(".GiT").join("config");
    let command = if cfg!(target_os = "windows") {
        "echo blocked > .GiT\\config"
    } else {
        "echo blocked > .GiT/config"
    };

    let result = driver.run(shell_command(command, workspace.path())).await;

    assert!(matches!(result, Err(SandboxError::PermissionDenied(_))));
    assert!(!target.exists());
}

#[tokio::test]
async fn blocked_network_denies_http_command() {
    let workspace = tempfile::tempdir().expect("temp workspace");
    let mut env = SandboxEnvironment::new(
        Some(workspace.path().to_path_buf()),
        SandboxPolicy::WorkspaceWrite,
    );
    env.permissions.network = NetworkPolicy::Blocked;
    let Some(driver) = smoke_driver(env) else {
        return;
    };

    let result =
        driver.run(shell_command("curl --max-time 1 https://example.com", workspace.path())).await;

    assert!(matches!(result, Err(SandboxError::PermissionDenied(_))));
}

#[cfg(target_os = "windows")]
#[tokio::test]
async fn workspace_write_denies_windows_namespace_escape() {
    let Some((workspace, driver)) = smoke_workspace(SandboxPolicy::WorkspaceWrite) else {
        return;
    };

    let result = driver
        .run(shell_command("echo blocked > \\\\?\\C:\\slab-sandbox-escape.txt", workspace.path()))
        .await;

    assert!(matches!(result, Err(SandboxError::PermissionDenied(_))));
}

fn shell_command(command: &str, cwd: &Path) -> SandboxedCommand {
    SandboxedCommand {
        argv: shell_argv(command),
        env: HashMap::new(),
        cwd: Some(cwd.to_path_buf()),
        timeout: Some(Duration::from_secs(10)),
        output_sink: None,
    }
}

fn shell_argv(command: &str) -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        vec!["cmd".to_string(), "/c".to_string(), command.to_string()]
    }

    #[cfg(not(target_os = "windows"))]
    {
        vec!["sh".to_string(), "-lc".to_string(), command.to_string()]
    }
}

fn shell_path(path: &Path) -> String {
    let raw = PathBuf::from(path).to_string_lossy().into_owned();
    if cfg!(target_os = "windows") {
        format!("\"{raw}\"")
    } else {
        format!("'{}'", raw.replace('\'', "'\\''"))
    }
}
