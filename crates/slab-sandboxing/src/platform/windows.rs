//! Thin Windows shim. The real isolation logic lives in the `slab-windows-sandbox` sub-crate
//! (which `slab-sandboxing` depends on, cfg-gated, downward only). This shim maps
//! `SandboxedCommand` → the sub-crate's `SpawnRequest`, delegates the spawn, then feeds the
//! returned `tokio::process::Child` + `kill_tree` closure into the shared `wait_for_child`.
//!
//! S2a: the executor is `JobOnlyExecutor` (Job-Object tree-cleanup + lexical guard — identical
//! to the pre-S2 behavior). S2b swaps in the elevated Low-IL restricted-token executor.

use async_trait::async_trait;

#[cfg(target_os = "windows")]
use crate::driver::{command_env, wait_for_child};
#[cfg(target_os = "windows")]
use crate::guard::validate_command;
use crate::{
    IsolationStrength, SandboxCapabilities, SandboxDriver, SandboxEnvironment, SandboxError,
    SandboxIsolation, SandboxPlatform, SandboxSetupStatus, SandboxedCommand, SandboxedOutput,
    SetupKind,
};
#[cfg(target_os = "windows")]
use slab_windows_sandbox::WindowsSandboxExecutor;

pub struct WindowsSandboxDriver {
    env: SandboxEnvironment,
    #[cfg(target_os = "windows")]
    executor: slab_windows_sandbox::JobOnlyExecutor,
}

impl WindowsSandboxDriver {
    pub fn new(env: SandboxEnvironment) -> Self {
        #[cfg(target_os = "windows")]
        {
            let setup_required = env.permissions.platform.windows_setup_required;
            Self { env, executor: slab_windows_sandbox::JobOnlyExecutor::new(setup_required) }
        }
        #[cfg(not(target_os = "windows"))]
        {
            Self { env }
        }
    }
}

#[async_trait]
impl SandboxDriver for WindowsSandboxDriver {
    fn name(&self) -> &str {
        "windows-job-object"
    }

    async fn run(&self, cmd: SandboxedCommand) -> Result<SandboxedOutput, SandboxError> {
        #[cfg(not(target_os = "windows"))]
        {
            let _ = cmd;
            return Err(SandboxError::UnsupportedPlatform);
        }

        #[cfg(target_os = "windows")]
        {
            // Lexical guard (defense-in-depth, bypassable) — unchanged from pre-S2.
            validate_command(&self.env, &cmd)?;

            let req = build_spawn_request(&self.env, &cmd);
            let spawned = self.executor.spawn_job_only(&req).map_err(map_win_err)?;
            // The shared output-capture/tree-kill loop owns the child; the executor's
            // `kill_tree` closure (dropping the Job handle) fires after the child exits.
            wait_for_child(spawned.child, cmd.timeout, cmd.output_sink.clone(), spawned.kill_tree)
                .await
        }
    }

    fn capabilities(&self) -> SandboxCapabilities {
        #[cfg(target_os = "windows")]
        {
            let snap = self.executor.capabilities();
            capabilities_from_snapshot(&snap)
        }
        #[cfg(not(target_os = "windows"))]
        {
            SandboxCapabilities {
                platform: SandboxPlatform::Windows,
                isolation: SandboxIsolation::Unsupported,
                ..SandboxCapabilities::default()
            }
        }
    }

    fn setup_status(&self) -> SandboxSetupStatus {
        #[cfg(target_os = "windows")]
        {
            let snap = self.executor.capabilities();
            if snap.provisioned {
                SandboxSetupStatus::ready(snap.details)
            } else {
                SandboxSetupStatus::degraded(snap.details)
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            SandboxSetupStatus::unavailable("Windows sandbox is only available on Windows")
        }
    }
}

#[cfg(target_os = "windows")]
fn build_spawn_request(
    env: &SandboxEnvironment,
    cmd: &SandboxedCommand,
) -> slab_windows_sandbox::SpawnRequest {
    use crate::policy::NetworkPolicy;

    let mut writable_roots = env.permissions.writable_roots.clone();
    if let Some(root) = &env.workspace_root {
        writable_roots.push(root.clone());
    }
    slab_windows_sandbox::SpawnRequest {
        argv: cmd.argv.clone(),
        env: command_env(env, cmd),
        cwd: cmd.cwd.clone(),
        denied_paths: env.permissions.denied_paths.clone(),
        denied_globs: env.permissions.denied_globs.clone(),
        writable_roots,
        workspace_root: env.workspace_root.clone(),
        network_blocked: matches!(env.permissions.network, NetworkPolicy::Blocked),
    }
}

/// Translate the sub-crate's decoupled snapshot into slab-sandboxing's honest capability report.
#[cfg(target_os = "windows")]
fn capabilities_from_snapshot(
    snap: &slab_windows_sandbox::CapabilitySnapshot,
) -> SandboxCapabilities {
    use slab_windows_sandbox::{FsIsolationStrength, WindowsSetupKind};

    let (filesystem_isolation, filesystem) = match snap.filesystem_isolation {
        FsIsolationStrength::OsEnforced => (IsolationStrength::OsEnforced, true),
        FsIsolationStrength::Lexical => (IsolationStrength::Lexical, false),
        FsIsolationStrength::None => (IsolationStrength::None, false),
    };
    let setup_kind = match snap.setup_kind {
        WindowsSetupKind::None => SetupKind::None,
        WindowsSetupKind::JobObject => SetupKind::JobObject,
        WindowsSetupKind::ElevatedAclToken => SetupKind::ElevatedAclToken,
        WindowsSetupKind::ElevatedAclTokenWfp => SetupKind::ElevatedAclTokenWfp,
    };
    // S2 reports `Elevated` once real OS-enforced fs isolation is provisioned (S2b). Network
    // stays lexical until WFP lands (S3).
    let isolation = if snap.provisioned
        && matches!(snap.filesystem_isolation, FsIsolationStrength::OsEnforced)
    {
        SandboxIsolation::Elevated
    } else {
        SandboxIsolation::Degraded
    };
    SandboxCapabilities {
        platform: SandboxPlatform::Windows,
        isolation,
        filesystem,
        network: false,
        network_isolation: IsolationStrength::Lexical,
        process_cleanup: true,
        setup_required: snap.setup_required,
        filesystem_isolation,
        setup_kind,
    }
}

#[cfg(target_os = "windows")]
fn map_win_err(e: slab_windows_sandbox::WindowsSandboxError) -> SandboxError {
    use slab_windows_sandbox::WindowsSandboxError;
    match e {
        WindowsSandboxError::EmptyCommand => SandboxError::EmptyCommand,
        WindowsSandboxError::SpawnFailed(s) => SandboxError::SpawnFailed(s),
        WindowsSandboxError::SetupFailed(s) => SandboxError::SetupFailed(s),
        WindowsSandboxError::UnsupportedPlatform => SandboxError::UnsupportedPlatform,
        WindowsSandboxError::PermissionDenied(s) => SandboxError::PermissionDenied(s),
        other => SandboxError::SetupFailed(other.to_string()),
    }
}
