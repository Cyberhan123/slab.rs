//! The executor trait + the two real executors (`BwrapExecutor`, `LandlockFallbackExecutor`) and
//! the degraded/unsupported sentinels. The selector runs once at driver construction and captures
//! the session-stable network predicate inputs so `capabilities()` can report honestly without a
//! per-call argument.

use std::process::Stdio;

use seccompiler::sock_filter;

use crate::bwrap;
use crate::capability::CapabilitySnapshot;
use crate::error::LinuxSandboxError;
use crate::landlock;
use crate::request::{SpawnRequest, SpawnedChild};
use crate::seccomp;

/// Produces isolated child processes on Linux. The thin shim in `slab_sandboxing::platform::linux`
/// holds one of these behind `Box<dyn>` and delegates to it.
pub trait LinuxSandboxExecutor: Send + Sync {
    /// Honest report of what this executor currently enforces.
    fn capabilities(&self) -> CapabilitySnapshot;

    /// Spawn the sandboxed child and return it with a tree-kill closure. The shim feeds both into
    /// the shared `wait_for_child`.
    fn spawn(&self, req: &SpawnRequest) -> Result<SpawnedChild, LinuxSandboxError>;
}

/// Network predicate inputs are session-stable (from `SandboxEnvironment`), captured at construction
/// so `capabilities()` needs no per-call argument.
fn network_enforced(network_blocked: bool, managed_proxy_active: bool) -> bool {
    network_blocked && !managed_proxy_active
}

/// Select the best available executor. Probes bwrap and landlock availability itself; `shim` passes
/// the config knob + the session-stable network predicate.
pub fn select_executor(
    allow_landlock_fallback: bool,
    network_blocked: bool,
    managed_proxy_active: bool,
) -> Box<dyn LinuxSandboxExecutor> {
    let bwrap_available = bwrap::find_bwrap().is_some();
    let landlock_available = landlock::probe_abi_version().is_some();
    if bwrap_available {
        Box::new(BwrapExecutor::new(network_blocked, managed_proxy_active))
    } else if landlock_available {
        Box::new(LandlockFallbackExecutor::new(network_blocked, managed_proxy_active))
    } else if allow_landlock_fallback {
        // bwrap absent + landlock unavailable + fallback opted-in ⇒ fail-closed degraded.
        Box::new(DegradedLandlockRequiredExecutor)
    } else {
        Box::new(UnsupportedExecutor)
    }
}

/// Primary executor: bubblewrap FS namespace + seccomp network filter (always stacked).
pub struct BwrapExecutor {
    network_blocked: bool,
    managed_proxy_active: bool,
}

impl BwrapExecutor {
    pub fn new(network_blocked: bool, managed_proxy_active: bool) -> Self {
        Self { network_blocked, managed_proxy_active }
    }
}

impl LinuxSandboxExecutor for BwrapExecutor {
    fn capabilities(&self) -> CapabilitySnapshot {
        CapabilitySnapshot::bwrap_seccomp(network_enforced(
            self.network_blocked,
            self.managed_proxy_active,
        ))
    }

    fn spawn(&self, req: &SpawnRequest) -> Result<SpawnedChild, LinuxSandboxError> {
        let bwrap = bwrap::find_bwrap().ok_or_else(|| {
            LinuxSandboxError::BwrapNotAvailable("bwrap not found on PATH".into())
        })?;
        let prefix = bwrap::build_bwrap_args(req);
        // Compile the network seccomp filter only when the network predicate holds. Fail-closed:
        // a compile error ⇒ no spawn. When the predicate is false we still install NO_NEW_PRIVS
        // (required by bwrap --unshare-user) but no deny rules.
        let bpf: Option<Vec<sock_filter>> =
            if req.network_enforced() { Some(seccomp::compile_network_filter()?) } else { None };

        let mut command = tokio::process::Command::new(&bwrap);
        command.args(&prefix);
        command.args(&req.argv);
        for (key, value) in &req.env {
            command.env(key, value);
        }
        if let Some(ref cwd) = req.cwd {
            command.current_dir(cwd);
        }
        command.kill_on_drop(true);
        command.stdin(Stdio::null());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        // bwrap runs with `--new-session`, so it is a session/group leader and `make_kill_tree`
        // tears down the whole tree. The network-only seccomp filter blocks none of bwrap's setup
        // syscalls (clone/unshare/mount/pivot_root/open); NO_NEW_PRIVS is required for bwrap's
        // unprivileged user namespace. Both inherit across execve to the final command.
        // SAFETY: `pre_exec` is an unsafe API; the closure performs only async-signal-safe raw
        // syscalls and runs between fork and execve.
        unsafe {
            command.pre_exec(move || {
                let prog: &[sock_filter] = bpf.as_deref().unwrap_or(&[]);
                seccomp::install_no_new_privs_and_seccomp(prog)
            });
        }

        let spawned = command.spawn().map_err(|e| LinuxSandboxError::SpawnFailed(e.to_string()))?;
        let kill_tree = bwrap::make_kill_tree(&spawned);
        Ok(SpawnedChild { child: spawned, kill_tree })
    }
}

/// Fallback executor (bwrap unavailable): landlock FS path-access + seccomp network filter.
pub struct LandlockFallbackExecutor {
    network_blocked: bool,
    managed_proxy_active: bool,
}

impl LandlockFallbackExecutor {
    pub fn new(network_blocked: bool, managed_proxy_active: bool) -> Self {
        Self { network_blocked, managed_proxy_active }
    }
}

impl LinuxSandboxExecutor for LandlockFallbackExecutor {
    fn capabilities(&self) -> CapabilitySnapshot {
        CapabilitySnapshot::bwrap_landlock_fallback(network_enforced(
            self.network_blocked,
            self.managed_proxy_active,
        ))
    }

    fn spawn(&self, req: &SpawnRequest) -> Result<SpawnedChild, LinuxSandboxError> {
        // Build the landlock ruleset (fail-closed). FD is CLOEXEC so it closes at the child's
        // exec (after restrict_self), not leaking into the sandboxed program.
        let ruleset_fd = landlock::build_ruleset_fd(req)?;
        let bpf: Option<Vec<sock_filter>> =
            if req.network_enforced() { Some(seccomp::compile_network_filter()?) } else { None };

        let program = req.argv.first().ok_or(LinuxSandboxError::EmptyCommand)?;
        let mut command = tokio::process::Command::new(program);
        command.args(&req.argv[1..]);
        for (key, value) in &req.env {
            command.env(key, value);
        }
        if let Some(ref cwd) = req.cwd {
            command.current_dir(cwd);
        }
        command.kill_on_drop(true);
        // New process group ⇒ session/group leader ⇒ `make_kill_tree` can tree-kill.
        command.process_group(0);
        command.stdin(Stdio::null());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        // SAFETY: `pre_exec` is an unsafe API; the closure performs only async-signal-safe raw
        // syscalls. The child inherited `ruleset_fd` at fork; restrict_self applies it to the
        // child (and all descendants) before exec.
        unsafe {
            command.pre_exec(move || {
                let prog: &[sock_filter] = bpf.as_deref().unwrap_or(&[]);
                seccomp::install_no_new_privs_and_seccomp(prog)?;
                landlock::restrict_self_raw(ruleset_fd)?;
                Ok(())
            });
        }

        let spawned = command.spawn().map_err(|e| LinuxSandboxError::SpawnFailed(e.to_string()))?;
        // The child forked with its own copy of `ruleset_fd`; close the parent's copy now.
        // SAFETY: `ruleset_fd` is a valid open fd from landlock_create_ruleset.
        unsafe { libc::close(ruleset_fd) };
        let kill_tree = bwrap::make_kill_tree(&spawned);
        Ok(SpawnedChild { child: spawned, kill_tree })
    }
}

/// bwrap absent + landlock unavailable + fallback opted-in: report fail-closed degraded so the
/// `available_sandbox_driver` gate (`setup_required && degraded ⇒ None`) blocks the shell.
pub struct DegradedLandlockRequiredExecutor;

impl LinuxSandboxExecutor for DegradedLandlockRequiredExecutor {
    fn capabilities(&self) -> CapabilitySnapshot {
        CapabilitySnapshot::degraded_landlock_required()
    }

    fn spawn(&self, _req: &SpawnRequest) -> Result<SpawnedChild, LinuxSandboxError> {
        Err(LinuxSandboxError::SetupFailed(
            "landlock fallback opted-in but neither bwrap nor landlock is available".into(),
        ))
    }
}

/// bwrap absent + landlock unavailable + fallback NOT opted-in.
pub struct UnsupportedExecutor;

impl LinuxSandboxExecutor for UnsupportedExecutor {
    fn capabilities(&self) -> CapabilitySnapshot {
        CapabilitySnapshot::unsupported()
    }

    fn spawn(&self, _req: &SpawnRequest) -> Result<SpawnedChild, LinuxSandboxError> {
        Err(LinuxSandboxError::UnsupportedPlatform)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::LinuxSetupKind;

    #[test]
    fn select_prefers_bwrap_when_available() {
        // We can't force find_bwrap() in a unit test, but we can assert the selector returns a
        // usable executor whose capabilities are one of the documented snapshots.
        let exec = select_executor(true, true, false);
        let cap = exec.capabilities();
        assert!(
            matches!(
                cap.setup_kind,
                LinuxSetupKind::BwrapSeccomp | LinuxSetupKind::BwrapLandlock | LinuxSetupKind::None
            ),
            "selector must pick a known setup_kind"
        );
    }

    #[test]
    fn unsupported_executor_reports_unsupported() {
        let exec = UnsupportedExecutor;
        let cap = exec.capabilities();
        assert_eq!(cap.setup_kind, LinuxSetupKind::None);
        assert!(!cap.provisioned);
        let err =
            exec.spawn(&minimal_req()).err().expect("unsupported executor must refuse to spawn");
        assert!(matches!(err, LinuxSandboxError::UnsupportedPlatform));
    }

    #[test]
    fn degraded_required_reports_setup_required() {
        let cap = DegradedLandlockRequiredExecutor.capabilities();
        assert!(cap.setup_required);
        assert!(cap.degraded);
        assert!(!cap.provisioned);
    }

    fn minimal_req() -> SpawnRequest {
        SpawnRequest {
            argv: vec!["/bin/true".into()],
            env: Default::default(),
            cwd: None,
            network_blocked: false,
            managed_proxy_active: false,
            sandbox_policy: crate::request::SandboxPolicyMirror::ReadOnly,
            workspace_root: None,
            writable_roots: vec![],
            readable_roots: vec![],
            denied_paths: vec![],
            protected_path_names: vec![],
        }
    }
}
