use async_trait::async_trait;

#[cfg(target_os = "macos")]
use crate::{NetworkPolicy, SandboxPolicy, guard::validate_command};
use crate::{
    SandboxCapabilities, SandboxDriver, SandboxEnvironment, SandboxError, SandboxIsolation,
    SandboxPlatform, SandboxSetupStatus, SandboxedCommand, SandboxedOutput,
};

pub struct MacosSandboxDriver {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    env: SandboxEnvironment,
}

impl MacosSandboxDriver {
    pub fn new(env: SandboxEnvironment) -> Self {
        Self { env }
    }
}

#[async_trait]
impl SandboxDriver for MacosSandboxDriver {
    fn name(&self) -> &str {
        "macos-seatbelt"
    }

    async fn run(&self, cmd: SandboxedCommand) -> Result<SandboxedOutput, SandboxError> {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = cmd;
            return Err(SandboxError::UnsupportedPlatform);
        }

        #[cfg(target_os = "macos")]
        {
            use std::process::Stdio;

            use crate::driver::{command_env, wait_for_child};

            validate_command(&self.env, &cmd)?;

            let profile = build_seatbelt_profile(&self.env);
            let profile_path = std::env::temp_dir().join(format!(
                "slab-seatbelt-{}-{}.sbpl",
                std::process::id(),
                monotonic_nanos()
            ));
            std::fs::write(&profile_path, profile)
                .map_err(|e| SandboxError::SetupFailed(e.to_string()))?;

            let mut command = tokio::process::Command::new("/usr/bin/sandbox-exec");
            command.arg("-f");
            command.arg(&profile_path);
            command.arg("--");
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
            let output = wait_for_child(spawned, cmd.timeout).await;
            let _ = std::fs::remove_file(profile_path);
            output
        }
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            platform: SandboxPlatform::Macos,
            isolation: if cfg!(target_os = "macos") {
                SandboxIsolation::Full
            } else {
                SandboxIsolation::Unsupported
            },
            filesystem: cfg!(target_os = "macos"),
            network: cfg!(target_os = "macos"),
            process_cleanup: cfg!(target_os = "macos"),
            setup_required: false,
        }
    }

    fn setup_status(&self) -> SandboxSetupStatus {
        #[cfg(target_os = "macos")]
        {
            if std::path::Path::new("/usr/bin/sandbox-exec").exists() {
                SandboxSetupStatus::ready("sandbox-exec is available")
            } else {
                SandboxSetupStatus::unavailable("sandbox-exec is not available")
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            SandboxSetupStatus::unavailable("macOS sandbox is only available on macOS")
        }
    }
}

#[cfg(target_os = "macos")]
fn build_seatbelt_profile(env: &SandboxEnvironment) -> String {
    let mut lines = vec![
        "(version 1)".to_string(),
        "(deny default)".to_string(),
        "(allow process*)".to_string(),
        "(allow file-read*)".to_string(),
        "(allow sysctl-read)".to_string(),
        "(allow mach-lookup)".to_string(),
    ];

    if matches!(env.permissions.network, NetworkPolicy::Allowed)
        || env.permissions.managed_proxy.is_some()
    {
        lines.push("(allow network*)".to_string());
    }

    match env.policy {
        SandboxPolicy::ReadOnly => {}
        SandboxPolicy::WorkspaceWrite => {
            if let Some(root) = &env.workspace_root {
                allow_write_subpaths(&mut lines, root);
            }
            for root in &env.permissions.writable_roots {
                allow_write_subpaths(&mut lines, root);
            }
            allow_write_subpaths(&mut lines, &std::env::temp_dir());
        }
        SandboxPolicy::DangerFullAccess => {
            lines.push("(allow file-write*)".to_string());
        }
    }

    for denied in &env.permissions.denied_paths {
        deny_file_subpaths(&mut lines, denied);
    }
    for name in &env.permissions.protected_path_names {
        if let Some(root) = &env.workspace_root {
            deny_write_subpaths(&mut lines, &root.join(name));
        }
    }

    lines.join("\n")
}

#[cfg(target_os = "macos")]
fn allow_write_subpaths(lines: &mut Vec<String>, path: &std::path::Path) {
    for path in seatbelt_paths(path) {
        lines.push(format!("(allow file-write* (subpath \"{}\"))", escape_sbpl_path(&path)));
    }
}

#[cfg(target_os = "macos")]
fn deny_file_subpaths(lines: &mut Vec<String>, path: &std::path::Path) {
    for path in seatbelt_paths(path) {
        lines.push(format!("(deny file* (subpath \"{}\"))", escape_sbpl_path(&path)));
    }
}

#[cfg(target_os = "macos")]
fn deny_write_subpaths(lines: &mut Vec<String>, path: &std::path::Path) {
    for path in seatbelt_paths(path) {
        lines.push(format!("(deny file-write* (subpath \"{}\"))", escape_sbpl_path(&path)));
    }
}

#[cfg(target_os = "macos")]
fn seatbelt_paths(path: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut paths = vec![path.to_path_buf()];
    if let Ok(canonical) = dunce::canonicalize(path)
        && !paths.iter().any(|candidate| candidate == &canonical)
    {
        paths.push(canonical);
    }
    paths
}

#[cfg(target_os = "macos")]
fn escape_sbpl_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "macos")]
fn monotonic_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}
