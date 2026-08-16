//! bubblewrap (`bwrap`) filesystem-namespace setup, moved out of the pre-S4 monolithic
//! `slab_sandboxing::platform::linux`. Builds the `bwrap` argv prefix from a `SpawnRequest` (this
//! crate must NOT import `slab_sandboxing::SandboxEnvironment` — cycle-free invariant).

use std::path::{Path, PathBuf};

use crate::request::SpawnRequest;

/// Walk `$PATH` (skipping the current directory) for an executable named `bwrap`.
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

/// Build the `bwrap` argv prefix (everything before `-- <command...>`).
///
/// Invariants preserved from the pre-S4 driver:
/// - `--new-session` is load-bearing: the child becomes a session/group leader so
///   `make_kill_tree` can tear down the whole tree with `kill(-(pgid), SIGKILL)`.
/// - `--unshare-net` and the seccomp network filter share the same predicate
///   (`network_blocked && !managed_proxy_active`) — a managed proxy needs outbound.
/// - Protected metadata children (`.git`/`.slab`/`.agents`) are re-bound read-only inside writable
///   roots via `bind_protected_children`.
pub fn build_bwrap_args(req: &SpawnRequest) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "--die-with-parent".into(),
        "--new-session".into(),
        "--unshare-user".into(),
        "--unshare-pid".into(),
    ];

    if req.network_enforced() {
        args.push("--unshare-net".into());
    }

    args.push("--proc".into());
    args.push("/proc".into());
    args.push("--dev".into());
    args.push("/dev".into());
    args.push("--ro-bind".into());
    args.push("/".into());
    args.push("/".into());

    match req.sandbox_policy {
        crate::request::SandboxPolicyMirror::ReadOnly => {}
        crate::request::SandboxPolicyMirror::WorkspaceWrite => {
            if let Some(ref root) = req.workspace_root {
                bind_rw(&mut args, root);
                bind_protected_children(&mut args, &req.protected_path_names, root);
            }
            for writable_root in &req.writable_roots {
                bind_rw(&mut args, writable_root);
                bind_protected_children(&mut args, &req.protected_path_names, writable_root);
            }
            bind_rw(&mut args, &std::env::temp_dir());
        }
        crate::request::SandboxPolicyMirror::DangerFullAccess => {
            args.push("--bind".into());
            args.push("/".into());
            args.push("/".into());
        }
    }

    for readable in &req.readable_roots {
        bind_ro(&mut args, readable);
    }
    for denied in &req.denied_paths {
        mask_path(&mut args, denied);
    }

    args.push("--".into());
    args
}

fn bind_rw(args: &mut Vec<String>, path: &Path) {
    args.push("--bind".into());
    args.push(path.display().to_string());
    args.push(path.display().to_string());
}

fn bind_ro(args: &mut Vec<String>, path: &Path) {
    args.push("--ro-bind".into());
    args.push(path.display().to_string());
    args.push(path.display().to_string());
}

fn bind_protected_children(args: &mut Vec<String>, protected_names: &[String], root: &Path) {
    for name in protected_names {
        let protected = root.join(name);
        if protected.exists() {
            bind_ro(args, &protected);
        }
    }
}

fn mask_path(args: &mut Vec<String>, path: &Path) {
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

/// Build a tree-kill closure for a Unix child: send `SIGKILL` to the child's process group so
/// backgrounded descendants die and release the pipes. The child must be a group leader — ensured
/// by bwrap's `--new-session` (bwrap path) or `process_group(0)` (landlock direct-spawn path).
/// Mirrors `slab_sandboxing::driver::unix_kill_tree`, kept local because that helper is
/// `pub(crate)` and this crate cannot depend on `slab_sandboxing`.
pub(crate) fn make_kill_tree(
    child: &tokio::process::Child,
) -> Option<Box<dyn FnOnce() + Send + 'static>> {
    child.id().map(|p| {
        Box::new(move || {
            // A negative pid targets the whole process group.
            let _ = unsafe { libc::kill(-(p as i32), libc::SIGKILL) };
        }) as Box<dyn FnOnce() + Send + 'static>
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::SandboxPolicyMirror;
    use std::path::PathBuf;

    fn req(
        policy: SandboxPolicyMirror,
        network_blocked: bool,
        managed_proxy: bool,
    ) -> SpawnRequest {
        SpawnRequest {
            argv: vec!["/bin/true".into()],
            env: Default::default(),
            cwd: None,
            network_blocked,
            managed_proxy_active: managed_proxy,
            sandbox_policy: policy,
            workspace_root: Some(PathBuf::from("/ws")),
            writable_roots: vec![],
            readable_roots: vec![],
            denied_paths: vec![],
            protected_path_names: vec![".git".into(), ".slab".into(), ".agents".into()],
        }
    }

    #[test]
    fn build_bwrap_args_includes_unshare_net_only_when_blocked_and_no_proxy() {
        let on = build_bwrap_args(&req(SandboxPolicyMirror::WorkspaceWrite, true, false));
        assert!(on.iter().any(|a| a == "--unshare-net"));

        let proxy = build_bwrap_args(&req(SandboxPolicyMirror::WorkspaceWrite, true, true));
        assert!(!proxy.iter().any(|a| a == "--unshare-net"));

        let allowed = build_bwrap_args(&req(SandboxPolicyMirror::WorkspaceWrite, false, false));
        assert!(!allowed.iter().any(|a| a == "--unshare-net"));
    }

    #[test]
    fn build_bwrap_args_always_ends_with_separator() {
        let args = build_bwrap_args(&req(SandboxPolicyMirror::ReadOnly, true, false));
        assert_eq!(args.last().unwrap(), "--");
        assert!(args.contains(&"--new-session".to_string()));
    }
}
