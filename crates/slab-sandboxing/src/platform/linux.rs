use std::path::PathBuf;

use async_trait::async_trait;
#[cfg(target_os = "linux")]
use tracing::debug;

#[cfg(target_os = "linux")]
use crate::{NetworkPolicy, SandboxPolicy, guard::validate_command};
use crate::{
    SandboxCapabilities, SandboxDriver, SandboxEnvironment, SandboxError, SandboxIsolation,
    SandboxPlatform, SandboxSetupStatus, SandboxedCommand, SandboxedOutput,
};

pub struct LinuxSandboxDriver {
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    env: SandboxEnvironment,
}

impl LinuxSandboxDriver {
    pub fn new(env: SandboxEnvironment) -> Self {
        Self { env }
    }

    pub fn available() -> bool {
        find_bwrap().is_some()
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
            use std::process::Stdio;

            use crate::driver::{command_env, wait_for_child};

            validate_command(&self.env, &cmd)?;

            let bwrap = find_bwrap()
                .ok_or_else(|| SandboxError::BwrapNotAvailable("bwrap not found on PATH".into()))?;
            let bwrap_args = build_bwrap_args(&self.env)?;
            debug!(bwrap = ?bwrap, args = ?bwrap_args, "spawning bwrap sandbox");

            let mut command = tokio::process::Command::new(&bwrap);
            command.args(&bwrap_args);
            command.args(&cmd.argv);
            for (key, value) in command_env(&self.env, &cmd) {
                command.env(key, value);
            }
            if let Some(ref cwd) = cmd.cwd {
                command.current_dir(cwd);
            }
            command.kill_on_drop(true);
            command.stdout(Stdio::piped());
            command.stderr(Stdio::piped());

            let spawned = command.spawn().map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
            wait_for_child(spawned, cmd.timeout).await
        }
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            platform: SandboxPlatform::Linux,
            isolation: if Self::available() {
                SandboxIsolation::Full
            } else {
                SandboxIsolation::Unsupported
            },
            filesystem: Self::available(),
            network: Self::available(),
            process_cleanup: Self::available(),
            setup_required: false,
        }
    }

    fn setup_status(&self) -> SandboxSetupStatus {
        #[cfg(target_os = "linux")]
        {
            if Self::available() {
                SandboxSetupStatus::ready("bubblewrap is available")
            } else {
                SandboxSetupStatus::unavailable(
                    "bubblewrap is required for Slab Linux sandbox execution",
                )
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            SandboxSetupStatus::unavailable("Linux sandbox is only available on Linux")
        }
    }
}

fn find_bwrap() -> Option<PathBuf> {
    let path_var = std::env::var("PATH").ok()?;
    let cwd = std::env::current_dir().ok();

    for dir in std::env::split_paths(&path_var) {
        if let Some(ref cwd_path) = cwd
            && &dir == cwd_path
        {
            continue;
        }
        let candidate = dir.join("bwrap");
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn build_bwrap_args(env: &SandboxEnvironment) -> Result<Vec<String>, SandboxError> {
    let mut args: Vec<String> = Vec::new();

    args.push("--die-with-parent".into());
    args.push("--new-session".into());
    args.push("--unshare-user".into());
    args.push("--unshare-pid".into());

    if matches!(env.permissions.network, NetworkPolicy::Blocked)
        && env.permissions.managed_proxy.is_none()
    {
        args.push("--unshare-net".into());
    }

    args.push("--proc".into());
    args.push("/proc".into());
    args.push("--dev".into());
    args.push("/dev".into());
    args.push("--ro-bind".into());
    args.push("/".into());
    args.push("/".into());

    match env.policy {
        SandboxPolicy::ReadOnly => {}
        SandboxPolicy::WorkspaceWrite => {
            if let Some(ref root) = env.workspace_root {
                bind_rw(&mut args, root);
                bind_protected_children(&mut args, env, root);
            }
            for writable_root in &env.permissions.writable_roots {
                bind_rw(&mut args, writable_root);
                bind_protected_children(&mut args, env, writable_root);
            }
            bind_rw(&mut args, &std::env::temp_dir());
        }
        SandboxPolicy::DangerFullAccess => {
            args.push("--bind".into());
            args.push("/".into());
            args.push("/".into());
        }
    }

    for readable in &env.permissions.readable_roots {
        bind_ro(&mut args, readable);
    }
    for denied in &env.permissions.denied_paths {
        mask_path(&mut args, denied);
    }

    args.push("--".into());
    Ok(args)
}

#[cfg(target_os = "linux")]
fn bind_rw(args: &mut Vec<String>, path: &std::path::Path) {
    args.push("--bind".into());
    args.push(path.display().to_string());
    args.push(path.display().to_string());
}

#[cfg(target_os = "linux")]
fn bind_ro(args: &mut Vec<String>, path: &std::path::Path) {
    args.push("--ro-bind".into());
    args.push(path.display().to_string());
    args.push(path.display().to_string());
}

#[cfg(target_os = "linux")]
fn bind_protected_children(
    args: &mut Vec<String>,
    env: &SandboxEnvironment,
    root: &std::path::Path,
) {
    for name in &env.permissions.protected_path_names {
        let protected = root.join(name);
        if protected.exists() {
            bind_ro(args, &protected);
        }
    }
}

#[cfg(target_os = "linux")]
fn mask_path(args: &mut Vec<String>, path: &std::path::Path) {
    if path.is_dir() {
        args.push("--tmpfs".into());
        args.push(path.display().to_string());
        return;
    }
    if path.exists() {
        args.push("--bind".into());
        args.push("/dev/null".into());
        args.push(path.display().to_string());
    }
}
