//! Network-only seccomp BPF filter. The filter's *mismatch* action is `Allow` (syscalls not in the
//! map, or whose rule conditions don't match, pass through); its *match* action is `KillProcess`
//! (a matching rule kills the whole process tree — no forked child can exfil after the parent
//! dies).
//!
//! The filter gates **socket creation by family**: `socket()` matches (⇒ kill) when its `domain`
//! argument is neither `AF_UNIX` nor `AF_NETLINK`. seccomp cannot introspect an fd's family
//! after creation, so filtering the data-plane syscalls (`connect`, `sendmsg`, …) instead would
//! also kill family-agnostic local IPC — which real binaries depend on (glibc's NSS daemon probe
//! `connect()`s an `AF_UNIX` socket during `getpwuid` under a sanitized environment, so an
//! unconditional `connect` kill makes even `bash -c 'echo hi'` die with SIGSYS). With creation
//! gated, no `AF_INET`/`AF_INET6`/`AF_PACKET` socket can ever come into existence, and the
//! data-plane syscalls can only ever operate on local (or inherited-and-CLOEXEC-closed)
//! descriptors — the same exfiltration guarantee without the collateral damage.
//!
//! The BPF program is compiled BEFORE spawn (it allocates). The `pre_exec` hook installs it via
//! raw syscalls only (`prctl` + the `seccomp` syscall), which is async-signal-safe. We do NOT use
//! `seccompiler::apply_filter` in the hook because it formats errors (may allocate).

use std::collections::BTreeMap;

use seccompiler::{
    BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
    SeccompRule, TargetArch, sock_filter,
};

use crate::error::LinuxSandboxError;

/// `SECCOMP_SET_MODE_FILTER` — defined locally because `libc` does not yet expose it
/// (see <https://github.com/rust-lang/libc/issues/3342>, mirrored from seccompiler's own backend).
const SECCOMP_SET_MODE_FILTER: libc::c_uint = 1;

/// Compile the network-only seccomp filter to a BPF program. Allocates — call BEFORE spawn, never
/// inside `pre_exec`.
pub fn compile_network_filter() -> Result<BpfProgram, LinuxSandboxError> {
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();

    // socket(domain, type, protocol): the rule MATCHES (⇒ match_action KillProcess) when arg0
    // (domain) is neither AF_UNIX nor AF_NETLINK. `AF_UNIX` keeps local IPC alive (glibc's nscd
    // probe, the shell's own sockets); `AF_NETLINK` is required by bwrap's namespace setup, which
    // opens a NETLINK_ROUTE socket while configuring the sandbox (and by getifaddrs-style probes
    // in sandboxed binaries) — netlink is kernel-local and cannot carry data off the machine.
    // Everything else (inet/inet6/packet) can never be created, so connect/sendmsg/… have
    // nothing network-ish to operate on (see the module docs).
    let af_unix =
        SeccompCondition::new(0, SeccompCmpArgLen::Qword, SeccompCmpOp::Ne, libc::AF_UNIX as u64)
            .map_err(|e| LinuxSandboxError::SeccompCompile(e.to_string()))?;
    let af_netlink = SeccompCondition::new(
        0,
        SeccompCmpArgLen::Qword,
        SeccompCmpOp::Ne,
        libc::AF_NETLINK as u64,
    )
    .map_err(|e| LinuxSandboxError::SeccompCompile(e.to_string()))?;
    rules.insert(
        libc::SYS_socket,
        vec![
            SeccompRule::new(vec![af_unix, af_netlink])
                .map_err(|e| LinuxSandboxError::SeccompCompile(e.to_string()))?,
        ],
    );

    // mismatch_action = Allow (default for non-network / AF_UNIX socket),
    // match_action = KillProcess (a rule's conditions match).
    let target_arch = TargetArch::try_from(std::env::consts::ARCH)
        .map_err(|e| LinuxSandboxError::SeccompCompile(format!("bad target arch: {e:?}")))?;
    let filter =
        SeccompFilter::new(rules, SeccompAction::Allow, SeccompAction::KillProcess, target_arch)
            .map_err(|e| LinuxSandboxError::SeccompCompile(e.to_string()))?;
    let bpf: BpfProgram = BpfProgram::try_from(filter)
        .map_err(|e| LinuxSandboxError::SeccompCompile(e.to_string()))?;
    Ok(bpf)
}

/// Kernel BPF program wrapper — `sock_fprog` is private in seccompiler, so we define a layout-
/// identical repr(C) struct for the raw `seccomp` syscall.
#[repr(C)]
struct sock_fprog {
    len: u16,
    filter: *const sock_filter,
}

/// Install `PR_SET_NO_NEW_PRIVS` followed by the seccomp filter. Raw syscalls only — called from
/// the child's `pre_exec` hook (async-signal-safe: no allocation, no locks). Ordering is
/// load-bearing: NO_NEW_PRIVS MUST be set before installing the filter (kernel requirement), and
/// it is also required for `bwrap --unshare-user` unprivileged user-namespace creation.
///
/// An empty program means "no network filter" (e.g. managed proxy active); NO_NEW_PRIVS is still
/// installed because bwrap userns requires it.
///
/// # Safety
/// Called between fork and execve. Must remain async-signal-safe.
pub(crate) unsafe fn install_no_new_privs_and_seccomp(bpf: &[sock_filter]) -> std::io::Result<()> {
    // SAFETY: prctl(PR_SET_NO_NEW_PRIVS) is a raw syscall with constant arguments.
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    if bpf.is_empty() {
        return Ok(());
    }
    let prog = sock_fprog { len: bpf.len() as u16, filter: bpf.as_ptr() };
    // SAFETY: the seccomp(2) syscall with SECCOMP_SET_MODE_FILTER copies the BPF program from
    // userspace; `prog` points at a valid `sock_fprog` wrapping the live `bpf` slice.
    let rc = unsafe {
        libc::syscall(libc::SYS_seccomp, SECCOMP_SET_MODE_FILTER, 0u32, &prog as *const sock_fprog)
    };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_network_filter_succeeds_and_is_nonempty() {
        let bpf = compile_network_filter().expect("filter compiles");
        assert!(!bpf.is_empty(), "BPF program must have instructions");
    }
}
