use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use slab_sandboxing::{
    NetworkPolicy, SandboxDriver, SandboxEnvironment, SandboxError, SandboxPolicy,
    SandboxedCommand, create_platform_driver,
};
use tempfile::TempDir;

fn smoke_workspace(policy: SandboxPolicy) -> (TempDir, Arc<dyn SandboxDriver>) {
    let workspace = tempfile::tempdir().expect("temp workspace");
    let env = SandboxEnvironment::new(Some(workspace.path().to_path_buf()), policy);
    let driver = create_platform_driver(env).expect("platform sandbox driver");
    let status = driver.setup_status();
    if !status.available {
        if std::env::var("SLAB_SANDBOX_SMOKE_ALLOW_SKIP").ok().as_deref() == Some("1") {
            eprintln!("skipping sandbox smoke: {}", status.details);
            return (workspace, driver);
        }
        panic!("{}", status.details);
    }
    (workspace, driver)
}

#[tokio::test]
async fn platform_driver_reports_capabilities() {
    let (_workspace, driver) = smoke_workspace(SandboxPolicy::WorkspaceWrite);
    let capabilities = driver.capabilities();

    assert!(matches!(driver.setup_status().available, true));
    assert!(capabilities.filesystem || capabilities.isolation as u8 > 0);
}

#[tokio::test]
async fn read_only_denies_workspace_write() {
    let (workspace, driver) = smoke_workspace(SandboxPolicy::ReadOnly);
    let target = workspace.path().join("ro-denied.txt");

    let result = driver.run(shell_command("echo denied > ro-denied.txt", workspace.path())).await;

    assert!(matches!(result, Err(SandboxError::PermissionDenied(_))));
    assert!(!target.exists());
}

#[tokio::test]
async fn workspace_write_allows_workspace_write() {
    let (workspace, driver) = smoke_workspace(SandboxPolicy::WorkspaceWrite);
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
    let (workspace, driver) = smoke_workspace(SandboxPolicy::WorkspaceWrite);
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
    let (workspace, driver) = smoke_workspace(SandboxPolicy::WorkspaceWrite);
    std::fs::create_dir_all(workspace.path().join(".GiT")).expect("metadata dir");
    let target = workspace.path().join(".GiT").join("config");

    let result = driver.run(shell_command("echo blocked > .GiT\\config", workspace.path())).await;

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
    let driver = create_platform_driver(env).expect("platform sandbox driver");

    let result =
        driver.run(shell_command("curl --max-time 1 https://example.com", workspace.path())).await;

    assert!(matches!(result, Err(SandboxError::PermissionDenied(_))));
}

#[cfg(target_os = "windows")]
#[tokio::test]
async fn workspace_write_denies_windows_namespace_escape() {
    let (workspace, driver) = smoke_workspace(SandboxPolicy::WorkspaceWrite);

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
