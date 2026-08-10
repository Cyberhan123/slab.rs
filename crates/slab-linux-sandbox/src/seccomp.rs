//! Network-only seccomp BPF filter. The filter's *mismatch* action is `Allow` (syscalls not in the
//! map, or whose rule conditions don't match, pass through); its *match* action is `KillProcess`
//! (a matching rule kills the whole process tree — no forked child can exfil after the parent
//! dies). The network syscalls are added as rules: `socket()` matches (⇒ kill) when its `domain`
//! argument is **not** `AF_UNIX`, so libc/NSS/fontconfig init probes that open Unix sockets
//! survive; the other network syscalls match unconditionally. (seccomp cannot introspect an fd's
//! family after creation, so `connect`/`sendmsg` on an AF_UNIX fd are collateral damage —
//! acceptable, since outbound exfiltration is fully blocked.)
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

/// Network syscalls denied unconditionally (kill on call). `socket` is handled separately so the
/// `AF_UNIX` exemption can be applied to its `domain` argument. `libc::SYS_*` is `c_long` (= `i64`
/// on the supported x86_64 linux target), matching the `i64` keys seccompiler requires.
const BLOCKED: &[libc::c_long] = &[
    libc::SYS_connect,
    libc::SYS_bind,
    libc::SYS_listen,
    libc::SYS_accept,
    libc::SYS_accept4,
    libc::SYS_sendto,
    libc::SYS_recvfrom,
    libc::SYS_sendmsg,
    libc::SYS_recvmsg,
    libc::SYS_getsockopt,
    libc::SYS_setsockopt,
    libc::SYS_shutdown,
    libc::SYS_socketpair,
];

/// Compile the network-only seccomp filter to a BPF program. Allocates — call BEFORE spawn, never
/// inside `pre_exec`.
pub fn compile_network_filter() -> Result<BpfProgram, LinuxSandboxError> {
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();

    // socket(domain, type, protocol): the rule MATCHES (⇒ match_action KillProcess) when arg0
    // (domain) != AF_UNIX. A matching domain (AF_UNIX) leaves the rule unmatched ⇒ mismatch_action
    // Allow.
    let af_unix =
        SeccompCondition::new(0, SeccompCmpArgLen::Qword, SeccompCmpOp::Ne, libc::AF_UNIX as u64)
            .map_err(|e| LinuxSandboxError::SeccompCompile(e.to_string()))?;
    rules.insert(
        libc::SYS_socket,
        vec![
            SeccompRule::new(vec![af_unix])
                .map_err(|e| LinuxSandboxError::SeccompCompile(e.to_string()))?,
        ],
    );

    for &sysno in BLOCKED {
        // Empty conditions ⇒ the rule matches unconditionally ⇒ KillProcess.
        rules.insert(
            sysno,
            vec![
                SeccompRule::new(vec![])
                    .map_err(|e| LinuxSandboxError::SeccompCompile(e.to_string()))?,
            ],
        );
    }

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
