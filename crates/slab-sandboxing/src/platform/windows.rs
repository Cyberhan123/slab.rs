//! Thin Windows shim. The real isolation logic lives in the `slab-windows-sandbox` sub-crate
//! (which `slab-sandboxing` depends on, cfg-gated, downward only). This shim maps
//! `SandboxedCommand` → the sub-crate's `SpawnRequest`, delegates the spawn, then feeds the result
//! into the shared `wait_for_child` (Job-only path) or `wait_for_elevated` (Low-IL path).
//!
//! S2b2: when `windows_setup_required` is set, the driver holds an `ElevatedAclTokenExecutor` and
//! `run()` takes the elevated Low-IL restricted-token path (once provisioned). Otherwise it stays
//! on the `JobOnlyExecutor` (today's behavior).

use async_trait::async_trait;

#[cfg(target_os = "windows")]
use std::sync::Arc;

#[cfg(target_os = "windows")]
use crate::driver::{OutputSink, OutputStream, command_env, wait_for_child, wait_for_elevated};
#[cfg(target_os = "windows")]
use crate::guard::validate_command;
#[cfg(target_os = "windows")]
use crate::{IsolationStrength, SetupKind};
use crate::{
    SandboxCapabilities, SandboxDriver, SandboxEnvironment, SandboxError, SandboxIsolation,
    SandboxPlatform, SandboxSetupStatus, SandboxedCommand, SandboxedOutput,
};
#[cfg(target_os = "windows")]
use slab_windows_sandbox::{ErasedOutputSink, WindowsSandboxExecutor};

pub struct WindowsSandboxDriver {
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    env: SandboxEnvironment,
    #[cfg(target_os = "windows")]
    executor: Box<dyn WindowsSandboxExecutor>,
    #[cfg(target_os = "windows")]
    cfg: Option<slab_windows_sandbox::PrepareContext>,
}

impl WindowsSandboxDriver {
    pub fn new(env: SandboxEnvironment) -> Self {
        #[cfg(target_os = "windows")]
        {
            let setup_required = env.permissions.platform.windows_setup_required;
            let cfg = build_prepare_context(&env);
            let executor: Box<dyn WindowsSandboxExecutor> = if setup_required {
                Box::new(slab_windows_sandbox::ElevatedAclTokenExecutor::new(cfg.clone()))
            } else {
                Box::new(slab_windows_sandbox::JobOnlyExecutor::new(false))
            };
            Self { env, executor, cfg: Some(cfg) }
        }
        #[cfg(not(target_os = "windows"))]
        {
            Self { env }
        }
    }

    /// Drive the one-time elevation + ACL provisioning round-trip. Called by
    /// `create_platform_driver` (Windows, when `windows_setup_required`) BEFORE the fail-closed
    /// gate evaluates `setup_status`. No-op for the Job-only driver.
    #[cfg(target_os = "windows")]
    pub(crate) fn prepare(&self) -> Result<(), SandboxError> {
        if let Some(cfg) = &self.cfg {
            self.executor.prepare(cfg).map_err(map_win_err)?;
        }
        Ok(())
    }
}

#[async_trait]
impl SandboxDriver for WindowsSandboxDriver {
    fn name(&self) -> &str {
        "windows-sandbox"
    }

    async fn run(&self, cmd: SandboxedCommand) -> Result<SandboxedOutput, SandboxError> {
        #[cfg(not(target_os = "windows"))]
        {
            let _ = cmd;
            return Err(SandboxError::UnsupportedPlatform);
        }

        #[cfg(target_os = "windows")]
        {
            // Lexical guard (defense-in-depth, bypassable) — applies to both paths.
            validate_command(&self.env, &cmd)?;

            let req = build_spawn_request(&self.env, &cmd);
            let snap = self.executor.capabilities();
            if self.executor.is_elevated_capable() && snap.provisioned {
                // Elevated Low-IL restricted-token path: child lives in the daemon; bytes relay
                // over the named pipe. The sink is bridged via SinkAdapter (no slab-sandboxing
                // dep in the sub-crate).
                let sink = cmd
                    .output_sink
                    .clone()
                    .map(|s| Arc::new(SinkAdapter(s)) as Arc<dyn ErasedOutputSink>);
                let elevated = self.executor.spawn_elevated(&req, sink).map_err(map_win_err)?;
                wait_for_elevated(elevated, cmd.timeout).await
            } else {
                // Job-only path: shared output-capture/tree-kill loop owns the child.
                let spawned = self.executor.spawn_job_only(&req).map_err(map_win_err)?;
                wait_for_child(
                    spawned.child,
                    cmd.timeout,
                    cmd.output_sink.clone(),
                    spawned.kill_tree,
                )
                .await
            }
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

/// Bridge `slab_sandboxing::OutputSink` → the sub-crate's `ErasedOutputSink` (the sub-crate
/// cannot depend on `slab_sandboxing`).
#[cfg(target_os = "windows")]
struct SinkAdapter(Arc<dyn OutputSink>);

#[cfg(target_os = "windows")]
impl ErasedOutputSink for SinkAdapter {
    fn on_output(&self, stream: slab_windows_sandbox::OutputStreamKind, delta: &str) {
        let mapped = match stream {
            slab_windows_sandbox::OutputStreamKind::Stdout => OutputStream::Stdout,
            slab_windows_sandbox::OutputStreamKind::Stderr => OutputStream::Stderr,
        };
        self.0.on_output(mapped, delta);
    }
}

/// Build the `PrepareContext` (session path set + runtime paths) the elevated executor needs to
/// provision. Runtime paths match the daemon (`app_home_dir` for key/marker; helper exe beside
/// `current_exe`).
#[cfg(target_os = "windows")]
fn build_prepare_context(env: &SandboxEnvironment) -> slab_windows_sandbox::PrepareContext {
    use crate::policy::NetworkPolicy;
    use std::path::PathBuf;

    let app_home = slab_utils::app_home::app_home_dir();
    let mut writable_roots = env.permissions.writable_roots.clone();
    if let Some(root) = &env.workspace_root {
        writable_roots.push(root.clone());
    }
    let helper_exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("slab-sandbox-helper.exe").to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("slab-sandbox-helper.exe"));

    slab_windows_sandbox::PrepareContext {
        workspace_root: env.workspace_root.clone(),
        denied_paths: env.permissions.denied_paths.clone(),
        denied_globs: env.permissions.denied_globs.clone(),
        writable_roots,
        network_blocked: matches!(env.permissions.network, NetworkPolicy::Blocked),
        helper_exe,
        key_path: app_home.join("sandbox-helper.key"),
        ipc_dir: app_home.join("sandbox-ipc"),
        marker_path: app_home.join("sandbox-marker.json"),
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
        // Opt-in ConPTY (S6a). Meaningful only on the elevated AppContainer path; the job-only
        // executor ignores it. Default false keeps the working piped-stdio path.
        use_conpty: env.permissions.platform.windows_use_conpty,
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
    let (network_isolation, network) = match snap.network_isolation {
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
    let fs_enforced = matches!(snap.filesystem_isolation, FsIsolationStrength::OsEnforced);
    let net_enforced = matches!(snap.network_isolation, FsIsolationStrength::OsEnforced);
    // Both dimensions OS-enforced (S3: Low-IL ACL fs + AppContainer/WFP net) ⇒ Full. FS-only
    // OS-enforced (S2b) ⇒ Elevated. Otherwise Degraded.
    let isolation = if snap.provisioned && fs_enforced && net_enforced {
        SandboxIsolation::Full
    } else if snap.provisioned && fs_enforced {
        SandboxIsolation::Elevated
    } else {
        SandboxIsolation::Degraded
    };
    SandboxCapabilities {
        platform: SandboxPlatform::Windows,
        isolation,
        filesystem,
        network,
        network_isolation,
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
