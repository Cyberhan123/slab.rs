//! Decoupled mirrors of `slab_sandboxing`'s honesty enums. The thin shim in
//! `slab_sandboxing::platform::windows` translates these 1:1 at the boundary, keeping the
//! dependency direction one-way (this crate MUST NOT depend on `slab_sandboxing`).

use serde::{Deserialize, Serialize};

/// Honest strength of the filesystem-isolation dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FsIsolationStrength {
    #[default]
    None,
    /// In-process lexical/path check (`validate_command`): defense-in-depth, bypassable.
    Lexical,
    /// OS kernel enforces it (Low-integrity label ACEs).
    OsEnforced,
}

/// How the Windows sandbox is (or would be) provisioned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WindowsSetupKind {
    #[default]
    None,
    /// Job Object (tree-kill) + lexical guard only — today's state.
    JobObject,
    /// Restricted token + Low-integrity-label ACL filesystem containment, no WFP (S2).
    ElevatedAclToken,
    /// Restricted token + ACL + WFP/firewall network blocking (reserved for S3).
    ElevatedAclTokenWfp,
}

/// What the executor currently enforces — the sub-crate's analogue of
/// `slab_sandboxing::SandboxCapabilities`, reduced to the fields the shim needs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapabilitySnapshot {
    pub filesystem_isolation: FsIsolationStrength,
    pub setup_kind: WindowsSetupKind,
    /// True when the platform config requests elevated setup (`windows_setup_required`).
    pub setup_required: bool,
    /// True when provisioning actually succeeded (marker valid + daemon connected in S2b).
    /// Until then the driver is honest about being degraded.
    pub provisioned: bool,
    /// True when the driver is usable but not fully isolated.
    pub degraded: bool,
    pub details: String,
}

impl CapabilitySnapshot {
    /// The non-elevated baseline: Job-Object cleanup + lexical guard, nothing OS-enforced.
    pub fn job_only(setup_required: bool) -> Self {
        let details = if setup_required {
            "Windows elevated sandbox setup is required before token, ACL, and firewall isolation."
        } else {
            "Windows Job Object cleanup and Slab policy guard are active; elevated setup not requested."
        };
        Self {
            filesystem_isolation: FsIsolationStrength::Lexical,
            setup_kind: WindowsSetupKind::JobObject,
            setup_required,
            provisioned: false,
            degraded: true,
            details: details.to_string(),
        }
    }

    /// Elevated setup requested but not yet provisioned — fail-closed (degraded ⇒ gate blocks).
    pub fn degraded_required() -> Self {
        Self {
            filesystem_isolation: FsIsolationStrength::Lexical,
            setup_kind: WindowsSetupKind::ElevatedAclToken,
            setup_required: true,
            provisioned: false,
            degraded: true,
            details: "Windows elevated sandbox setup required but not yet provisioned \
                      (shell blocked, fail-closed)."
                .to_string(),
        }
    }

    /// Real OS-enforced isolation: Low-IL restricted token + integrity-label ACLs provisioned.
    pub fn elevated() -> Self {
        Self {
            filesystem_isolation: FsIsolationStrength::OsEnforced,
            setup_kind: WindowsSetupKind::ElevatedAclToken,
            setup_required: true,
            provisioned: true,
            degraded: false,
            details: "Windows Low-IL restricted token + integrity-label ACLs are OS-enforced."
                .to_string(),
        }
    }
}
