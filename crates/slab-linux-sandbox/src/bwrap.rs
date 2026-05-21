use slab_sandboxing::{NetworkPolicy, SandboxEnvironment, SandboxError, SandboxedCommand};
use std::path::PathBuf;

/// Find the bwrap binary on PATH, excluding the current working directory.
pub fn find_bwrap() -> Option<PathBuf> {
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

/// Build the bwrap argument list for the given environment and command.
pub fn build_bwrap_args(
    env: &SandboxEnvironment,
    _cmd: &SandboxedCommand,
) -> Result<Vec<String>, SandboxError> {
    use slab_sandboxing::SandboxPolicy;

    let mut args: Vec<String> = Vec::new();

    args.push("--unshare-user".into());
    args.push("--unshare-pid".into());

    if matches!(env.permissions.network, NetworkPolicy::Blocked) {
        args.push("--unshare-net".into());
    }

    args.push("--proc".into());
    args.push("/proc".into());

    args.push("--ro-bind".into());
    args.push("/".into());
    args.push("/".into());

    match env.policy {
        SandboxPolicy::WorkspaceWrite => {
            if let Some(ref root) = env.workspace_root {
                args.push("--bind".into());
                args.push(root.display().to_string());
                args.push(root.display().to_string());

                let git_dir = root.join(".git");
                if git_dir.exists() {
                    args.push("--ro-bind".into());
                    args.push(git_dir.display().to_string());
                    args.push(git_dir.display().to_string());
                }
            }

            for writable_root in &env.permissions.writable_roots {
                args.push("--bind".into());
                args.push(writable_root.display().to_string());
                args.push(writable_root.display().to_string());
            }
        }
        SandboxPolicy::DangerFullAccess => {
            args.push("--bind".into());
            args.push("/".into());
            args.push("/".into());
        }
        SandboxPolicy::ReadOnly => {}
    }

    for denied in &env.permissions.denied_paths {
        // Only apply deny rules for paths that currently exist.  Note: there is
        // a TOCTOU window between this check and the bwrap spawn — paths created
        // after this point will not be denied.  Callers should configure the
        // sandbox before any untrusted process can create such paths.
        if denied.exists() {
            if denied.is_dir() {
                args.push("--tmpfs".into());
                args.push(denied.display().to_string());
            } else {
                args.push("--bind".into());
                args.push("/dev/null".into());
                args.push(denied.display().to_string());
            }
        }
    }

    args.push("--".into());

    Ok(args)
}
