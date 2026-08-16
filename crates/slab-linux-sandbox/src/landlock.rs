//! Landlock filesystem-isolation fallback, used ONLY when `bwrap` is unavailable (containers
//! without user namespaces). Mutually exclusive with the bwrap path on the FS dimension — never
//! stacked (under the no-helper-binary constraint there is no hook to apply landlock after bwrap
//! sets up its mount view, and applying landlock to the bwrap process breaks its setup).
//!
//! We use **raw `landlock_*` syscalls** (via `libc::SYS_landlock_*`) rather than the `landlock`
//! crate's high-level `Ruleset`/`restrict_self()`, for two reasons: (1) `RulesetCreated` exposes no
//! file-descriptor accessor, and (2) the high-level `restrict_self()` allocates and is therefore
//! NOT async-signal-safe — but it MUST run in the child's `pre_exec` hook. The `landlock` crate is
//! still a dependency: we use its `ABI`/`AccessFs` to compute correct per-ABI access masks (ABI 3+
//! adds `Truncate`, ABI 5+ adds `IoctlDev` — hardcoding the ABI-1 mask would leave those write-like
//! operations unhandled and leak them past the sandbox).

use std::ffi::CString;
use std::path::Path;

use landlock::{ABI, AccessFs};

use crate::error::LinuxSandboxError;
use crate::request::{SandboxPolicyMirror, SpawnRequest};

/// `LANDLOCK_CREATE_RULESET_VERSION` — passed to `landlock_create_ruleset` to query the running
/// kernel's ABI version (defined locally; libc does not expose it).
const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1;
/// `LANDLOCK_RULE_PATH_BENEATH` — rule type for `landlock_add_rule`.
const LANDLOCK_RULE_PATH_BENEATH: u32 = 1;

/// `struct landlock_ruleset_attr { u64 handled_access_fs; u64 handled_access_net; }`. We only ever
/// handle filesystem accesses (network is seccomp's job), so we pass `size = 8` and the kernel
/// reads only `handled_access_fs` on every ABI.
#[repr(C)]
struct LandlockRulesetAttr {
    handled_access_fs: u64,
}

/// `struct landlock_path_beneath_attr { u64 allowed_access; s32 parent_fd; } __attribute__((packed))`.
#[repr(C, packed)]
struct LandlockPathBeneathAttr {
    allowed_access: u64,
    parent_fd: i32,
}

/// Probe the running kernel's landlock ABI version. `None` ⇒ landlock unsupported (ABI < 1).
pub fn probe_abi_version() -> Option<i32> {
    // SAFETY: the version probe passes a NULL attr with size 0; it performs no memory access and
    // only returns the ABI version (or -1 with errno).
    let v = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            std::ptr::null::<u8>(),
            0usize,
            LANDLOCK_CREATE_RULESET_VERSION,
        )
    };
    if v < 0 { None } else { Some(v as i32) }
}

/// Per-ABI access masks: `(all, read, write)`. Computed via the `landlock` crate so the handled set
/// matches whatever the running kernel actually understands (avoids leaking ABI-3+ `Truncate` etc.).
/// `all = read | write` (the crate's `from_all` is exactly this union).
fn access_masks(abi: ABI) -> (u64, u64, u64) {
    let read = AccessFs::from_read(abi).bits();
    let write = AccessFs::from_write(abi).bits();
    (read | write, read, write)
}

/// Open `path` with `O_PATH | O_CLOEXEC` (the fd identifies the directory for landlock without
/// granting access itself). `None` if the path does not exist (caller skips it).
fn open_path_fd(path: &Path) -> Option<i32> {
    let s = path.to_str()?;
    let c = CString::new(s).ok()?;
    // SAFETY: `c` is a valid NUL-terminated path; O_PATH opens without permission checks.
    let fd = unsafe { libc::open(c.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
    if fd < 0 { None } else { Some(fd) }
}

/// Build a landlock ruleset: handle all FS accesses (deny-default), then grant read on `/` and
/// write on the writable roots (workspace + extra writable + temp). Returns the ruleset FD; the
/// caller keeps it open until after the child has called `restrict_self_raw`.
///
/// Fail-closed: any `landlock_create_ruleset`/`landlock_add_rule` failure ⇒ `Err` ⇒ no spawn.
pub fn build_ruleset_fd(req: &SpawnRequest) -> Result<i32, LinuxSandboxError> {
    let version = probe_abi_version()
        .ok_or_else(|| LinuxSandboxError::LandlockUnavailable("ABI < 1".into()))?;
    let abi = ABI::from(version);
    let (all, read, write) = access_masks(abi);

    let attr = LandlockRulesetAttr { handled_access_fs: all };
    // SAFETY: attr is a valid 8-byte struct; size 8 makes the kernel read only handled_access_fs.
    let ruleset_fd = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            &attr as *const LandlockRulesetAttr,
            8usize,
            0u32,
        )
    };
    if ruleset_fd < 0 {
        return Err(LinuxSandboxError::LandlockUnavailable(format!(
            "landlock_create_ruleset failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    let ruleset_fd = ruleset_fd as i32;
    // Set CLOEXEC so the ruleset fd closes at the child's exec (after restrict_self runs) and does
    // not leak into the sandboxed program.
    // SAFETY: fcntl(F_SETFD) on a valid fd with FD_CLOEXEC.
    unsafe {
        libc::fcntl(ruleset_fd, libc::F_SETFD, libc::FD_CLOEXEC);
    }

    // Helper to add a PATH_BENEATH rule; returns Err on syscall failure (fail-closed).
    let add = |parent_fd: i32, allowed_access: u64| -> Result<(), LinuxSandboxError> {
        let rule = LandlockPathBeneathAttr { allowed_access, parent_fd };
        // SAFETY: `rule` is a valid packed struct; the kernel copies its 12 bytes.
        let rc = unsafe {
            libc::syscall(
                libc::SYS_landlock_add_rule,
                ruleset_fd,
                LANDLOCK_RULE_PATH_BENEATH,
                &rule as *const LandlockPathBeneathAttr,
                0u32,
            )
        };
        if rc != 0 {
            return Err(LinuxSandboxError::LandlockUnavailable(format!(
                "landlock_add_rule failed: {}",
                std::io::Error::last_os_error()
            )));
        }
        Ok(())
    };

    let add_path = |p: &Path, access: u64| -> Result<(), LinuxSandboxError> {
        if let Some(fd) = open_path_fd(p) {
            let res = add(fd, access);
            // SAFETY: fd came from open(); closing it is always valid.
            unsafe { libc::close(fd) };
            res?;
        }
        Ok(())
    };

    match req.sandbox_policy {
        SandboxPolicyMirror::DangerFullAccess => {
            // Grant all access on "/" — matches bwrap --bind / /.
            add_path(Path::new("/"), all)?;
        }
        SandboxPolicyMirror::WorkspaceWrite | SandboxPolicyMirror::ReadOnly => {
            // Read everywhere.
            add_path(Path::new("/"), read)?;
            // Write on workspace + writable roots + temp dir. Protected metadata names cannot be
            // excluded under landlock (union, most-permissive) — documented; the lexical guard
            // remains defense-in-depth.
            if let Some(ref root) = req.workspace_root {
                add_path(root, write)?;
            }
            for w in &req.writable_roots {
                add_path(w, write)?;
            }
            add_path(&std::env::temp_dir(), write)?;
        }
    }

    Ok(ruleset_fd)
}

/// Apply the ruleset to the calling thread (and all descendants). Raw syscall only — called from
/// the child's `pre_exec` hook (async-signal-safe).
///
/// # Safety
/// Called between fork and execve. Must remain async-signal-safe.
pub(crate) unsafe fn restrict_self_raw(ruleset_fd: i32) -> std::io::Result<()> {
    let rc = unsafe { libc::syscall(libc::SYS_landlock_restrict_self, ruleset_fd, 0u32) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_abi_version_returns_some_on_supported_kernels() {
        // On this host the probe either succeeds (Linux ≥ 5.13) or returns None (older/CI). Either
        // is valid; we only assert it does not panic.
        let _ = probe_abi_version();
    }
}
