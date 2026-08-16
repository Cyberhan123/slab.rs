//! Decoupled mirrors of `slab_sandboxing`'s honesty enums. The thin shim in
//! `slab_sandboxing::platform::linux` translates these 1:1 at the boundary, keeping the dependency
//! direction one-way (this crate MUST NOT depend on `slab_sandboxing`).

use serde::{Deserialize, Serialize};

/// Honest strength of the filesystem-isolation dimension. Mirrors
/// `slab_sandboxing::IsolationStrength` 1:1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FsIsolationStrength {
    #[default]
    None,
    /// In-process lexical/path check (`validate_command`): defense-in-depth, bypassable.
    Lexical,
    /// OS kernel enforces it (bwrap bind mounts / landlock path-access rules).
    OsEnforced,
}

/// How the Linux sandbox is provisioned. Mirrors the Linux variants of
/// `slab_sandboxing::SetupKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LinuxSetupKind {
    #[default]
    None,
    /// bubblewrap only (no seccomp available, or network not blocked).
    Bwrap,
    /// bubblewrap + seccomp network filter.
    BwrapSeccomp,
    /// landlock filesystem fallback (bwrap unavailable) + seccomp.
    BwrapLandlock,
}

/// What the executor currently enforces — the sub-crate's analogue of
/// `slab_sandboxing::SandboxCapabilities`, reduced to the fields the shim needs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapabilitySnapshot {
    pub filesystem_isolation: FsIsolationStrength,
    /// Honest strength of the network-isolation dimension. `OsEnforced` only when the network
    /// predicate holds (`network_blocked && !managed_proxy_active`) and seccomp is installed.
    pub network_isolation: FsIsolationStrength,
    pub setup_kind: LinuxSetupKind,
    /// True when the landlock fallback is opted-in (`linux_allow_landlock_fallback`) but neither
    /// bwrap nor landlock could provision — the fail-closed gate blocks the shell.
    pub setup_required: bool,
    /// True when a real isolation mechanism is actually active.
    pub provisioned: bool,
    /// True when the driver is usable but not fully isolated (e.g. managed proxy ⇒ net not enforced).
    pub degraded: bool,
    pub details: String,
}

impl CapabilitySnapshot {
    /// Neither bwrap nor landlock available, fallback not opted-in.
    pub fn unsupported() -> Self {
        Self {
            filesystem_isolation: FsIsolationStrength::None,
            network_isolation: FsIsolationStrength::None,
            setup_kind: LinuxSetupKind::None,
            setup_required: false,
            provisioned: false,
            degraded: false,
            details: "No Linux sandbox mechanism available (no bwrap; landlock fallback not \
                      opted-in)."
                .to_string(),
        }
    }

    /// bubblewrap FS namespace + seccomp network filter. `network_enforced` is the
    /// `network_blocked && !managed_proxy_active` predicate — when false the network dimension is
    /// honestly reported `Lexical` (managed proxy needs outbound) and the driver is `degraded`.
    pub fn bwrap_seccomp(network_enforced: bool) -> Self {
        let (net, degraded, details) = if network_enforced {
            (
                FsIsolationStrength::OsEnforced,
                false,
                "bubblewrap FS namespace + --unshare-net + seccomp network filter are OS-enforced.",
            )
        } else {
            (
                FsIsolationStrength::Lexical,
                true,
                "bubblewrap FS namespace is OS-enforced; network not enforced (managed proxy or \
                 network allowed).",
            )
        };
        Self {
            filesystem_isolation: FsIsolationStrength::OsEnforced,
            network_isolation: net,
            setup_kind: LinuxSetupKind::BwrapSeccomp,
            setup_required: false,
            provisioned: true,
            degraded,
            details: details.to_string(),
        }
    }

    /// landlock FS fallback (bwrap unavailable) + seccomp network filter.
    pub fn bwrap_landlock_fallback(network_enforced: bool) -> Self {
        let (net, degraded, details) = if network_enforced {
            (
                FsIsolationStrength::OsEnforced,
                false,
                "landlock FS path-access + seccomp network filter are OS-enforced (bwrap \
                 unavailable fallback).",
            )
        } else {
            (
                FsIsolationStrength::Lexical,
                true,
                "landlock FS path-access is OS-enforced (bwrap unavailable fallback); network not \
                 enforced.",
            )
        };
        Self {
            filesystem_isolation: FsIsolationStrength::OsEnforced,
            network_isolation: net,
            setup_kind: LinuxSetupKind::BwrapLandlock,
            setup_required: false,
            provisioned: true,
            degraded,
            details: details.to_string(),
        }
    }

    /// landlock fallback opted-in but unavailable (bwrap absent + landlock ABI < 1) ⇒ fail-closed:
    /// `setup_required && degraded` makes `available_sandbox_driver` block the shell.
    pub fn degraded_landlock_required() -> Self {
        Self {
            filesystem_isolation: FsIsolationStrength::Lexical,
            network_isolation: FsIsolationStrength::None,
            setup_kind: LinuxSetupKind::BwrapLandlock,
            setup_required: true,
            provisioned: false,
            degraded: true,
            details: "Landlock fallback opted-in but neither bwrap nor landlock is available \
                      (shell blocked, fail-closed)."
                .to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bwrap_seccomp_reports_net_os_enforced_only_when_predicate_holds() {
        let enforced = CapabilitySnapshot::bwrap_seccomp(true);
        assert_eq!(enforced.network_isolation, FsIsolationStrength::OsEnforced);
        assert!(!enforced.degraded);
        assert_eq!(enforced.setup_kind, LinuxSetupKind::BwrapSeccomp);

        let not_enforced = CapabilitySnapshot::bwrap_seccomp(false);
        assert_eq!(not_enforced.network_isolation, FsIsolationStrength::Lexical);
        assert!(not_enforced.degraded);
    }

    #[test]
    fn degraded_landlock_required_is_degraded_setup_required() {
        let s = CapabilitySnapshot::degraded_landlock_required();
        assert!(s.setup_required);
        assert!(s.degraded);
        assert!(!s.provisioned);
        assert_eq!(s.setup_kind, LinuxSetupKind::BwrapLandlock);
    }

    #[test]
    fn unsupported_is_none() {
        let s = CapabilitySnapshot::unsupported();
        assert_eq!(s.filesystem_isolation, FsIsolationStrength::None);
        assert_eq!(s.setup_kind, LinuxSetupKind::None);
        assert!(!s.provisioned);
        assert!(!s.setup_required);
    }
}
