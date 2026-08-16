//! Thin Linux shim. The real isolation logic lives in the `slab-linux-sandbox` sub-crate (which
//! `slab-sandboxing` depends on, cfg-gated, downward only). This shim maps `SandboxedCommand` → the
//! sub-crate's `SpawnRequest`, delegates the spawn, then feeds the result into the shared
//! `wait_for_child`. bwrap stays the primary FS mechanism (already real `OsEnforced`); seccomp is
//! always stacked on the network dimension; landlock is the FS fallback when bwrap is unavailable
//! (`linux_allow_landlock_fallback`). bwrap and landlock are mutually exclusive on the FS dimension.

use async_trait::async_trait;
#[cfg(target_os = "linux")]
use tracing::debug;

#[cfg(target_os = "linux")]
use crate::driver::wait_for_child;
#[cfg(target_os = "linux")]
use crate::{IsolationStrength, SetupKind};
#[cfg(target_os = "linux")]
use crate::{NetworkPolicy, SandboxPolicy, guard::validate_command};
use crate::{
    SandboxCapabilities, SandboxDriver, SandboxEnvironment, SandboxError, SandboxIsolation,
    SandboxPlatform, SandboxSetupStatus, SandboxedCommand, SandboxedOutput,
};

pub struct LinuxSandboxDriver {
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    env: SandboxEnvironment,
    #[cfg(target_os = "linux")]
    executor: Box<dyn slab_linux_sandbox::LinuxSandboxExecutor>,
}

impl LinuxSandboxDriver {
    pub fn new(env: SandboxEnvironment) -> Self {
        #[cfg(target_os = "linux")]
        {
            let network_blocked = matches!(env.permissions.network, NetworkPolicy::Blocked);
            let managed_proxy_active = env.permissions.managed_proxy.is_some();
            let allow_fallback = env.permissions.platform.linux_allow_landlock_fallback;
            let executor = slab_linux_sandbox::select_executor(
                allow_fallback,
                network_blocked,
                managed_proxy_active,
            );
            Self { env, executor }
        }
        #[cfg(not(target_os = "linux"))]
        {
            Self { env }
        }
    }
}

#[async_trait]
impl SandboxDriver for LinuxSandboxDriver {
    fn name(&self) -> &str {
        "linux-bwrap"
    }

    async fn run(&self, cmd: SandboxedCommand) -> Result<SandboxedOutput, SandboxError> {
        #[cfg(not(target_os = "linux"))]
        {
            let _ = cmd;
            return Err(SandboxError::UnsupportedPlatform);
        }

        #[cfg(target_os = "linux")]
        {
            // Lexical guard (defense-in-depth, bypassable) — applies before delegation.
            validate_command(&self.env, &cmd)?;

            let req = build_spawn_request(&self.env, &cmd);
            let spawned = self.executor.spawn(&req).map_err(map_linux_err)?;
            debug!(pid = spawned.child.id(), "spawned Linux sandboxed child");
            wait_for_child(spawned.child, cmd.timeout, cmd.output_sink.clone(), spawned.kill_tree)
                .await
        }
    }

    fn capabilities(&self) -> SandboxCapabilities {
        #[cfg(target_os = "linux")]
        {
            let snap = self.executor.capabilities();
            capabilities_from_snapshot(&snap)
        }
        #[cfg(not(target_os = "linux"))]
        {
            SandboxCapabilities {
                platform: SandboxPlatform::Linux,
                isolation: SandboxIsolation::Unsupported,
                ..SandboxCapabilities::default()
            }
        }
    }

    fn setup_status(&self) -> SandboxSetupStatus {
        #[cfg(target_os = "linux")]
        {
            let snap = self.executor.capabilities();
            if snap.provisioned {
                SandboxSetupStatus::ready(snap.details)
            } else if snap.setup_required {
                // Fail-closed: landlock fallback opted-in but unavailable. `degraded` keeps
                // `available=true` so the `setup_required && degraded` gate branch blocks the shell.
                SandboxSetupStatus::degraded(snap.details)
            } else {
                SandboxSetupStatus::unavailable(snap.details)
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            SandboxSetupStatus::unavailable("Linux sandbox is only available on Linux")
        }
    }
}

/// Build the sub-crate `SpawnRequest` from the shared environment + command.
#[cfg(target_os = "linux")]
fn build_spawn_request(
    env: &SandboxEnvironment,
    cmd: &SandboxedCommand,
) -> slab_linux_sandbox::SpawnRequest {
    use crate::driver::command_env;
    use slab_linux_sandbox::SandboxPolicyMirror;

    let sandbox_policy = match env.policy {
        SandboxPolicy::ReadOnly => SandboxPolicyMirror::ReadOnly,
        SandboxPolicy::WorkspaceWrite => SandboxPolicyMirror::WorkspaceWrite,
        SandboxPolicy::DangerFullAccess => SandboxPolicyMirror::DangerFullAccess,
    };

    slab_linux_sandbox::SpawnRequest {
        argv: cmd.argv.clone(),
        env: command_env(env, cmd),
        cwd: cmd.cwd.clone(),
        network_blocked: matches!(env.permissions.network, NetworkPolicy::Blocked),
        managed_proxy_active: env.permissions.managed_proxy.is_some(),
        sandbox_policy,
        workspace_root: env.workspace_root.clone(),
        writable_roots: env.permissions.writable_roots.clone(),
        readable_roots: env.permissions.readable_roots.clone(),
        denied_paths: env.permissions.denied_paths.clone(),
        protected_path_names: env.permissions.protected_path_names.clone(),
    }
}

/// Translate the sub-crate's decoupled snapshot into slab-sandboxing's honest capability report.
#[cfg(target_os = "linux")]
fn capabilities_from_snapshot(
    snap: &slab_linux_sandbox::CapabilitySnapshot,
) -> SandboxCapabilities {
    use slab_linux_sandbox::{FsIsolationStrength, LinuxSetupKind};

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
        LinuxSetupKind::None => SetupKind::None,
        LinuxSetupKind::Bwrap => SetupKind::Bwrap,
        LinuxSetupKind::BwrapSeccomp => SetupKind::BwrapSeccomp,
        LinuxSetupKind::BwrapLandlock => SetupKind::BwrapLandlock,
    };

    let fs_enforced = matches!(snap.filesystem_isolation, FsIsolationStrength::OsEnforced);
    let net_enforced = matches!(snap.network_isolation, FsIsolationStrength::OsEnforced);
    // bwrap = namespace isolation (process can't see non-bound paths) ⇒ Full.
    // landlock = path-access filtering in the same namespace ⇒ KernelFiltered.
    // Both dims OsEnforced otherwise ⇒ Full (bwrap+seccomp) / Elevated (fs only, e.g. managed proxy).
    let isolation = if snap.provisioned && fs_enforced && net_enforced {
        match snap.setup_kind {
            LinuxSetupKind::BwrapLandlock => SandboxIsolation::KernelFiltered,
            _ => SandboxIsolation::Full,
        }
    } else if snap.provisioned && fs_enforced {
        SandboxIsolation::Elevated
    } else if snap.provisioned {
        SandboxIsolation::Degraded
    } else {
        SandboxIsolation::Unsupported
    };

    SandboxCapabilities {
        platform: SandboxPlatform::Linux,
        isolation,
        filesystem,
        network,
        filesystem_isolation,
        network_isolation,
        process_cleanup: true,
        setup_required: snap.setup_required,
        setup_kind,
    }
}

/// Map the sub-crate's error into the shared `SandboxError`.
#[cfg(target_os = "linux")]
fn map_linux_err(e: slab_linux_sandbox::LinuxSandboxError) -> SandboxError {
    use slab_linux_sandbox::LinuxSandboxError;
    match e {
        LinuxSandboxError::EmptyCommand => SandboxError::EmptyCommand,
        LinuxSandboxError::SpawnFailed(s) => SandboxError::SpawnFailed(s),
        LinuxSandboxError::BwrapNotAvailable(s) => SandboxError::BwrapNotAvailable(s),
        LinuxSandboxError::PermissionDenied(s) => SandboxError::PermissionDenied(s),
        LinuxSandboxError::UnsupportedPlatform => SandboxError::UnsupportedPlatform,
        other => SandboxError::SetupFailed(other.to_string()),
    }
}
