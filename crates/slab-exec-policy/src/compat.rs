//! Backward-compatibility shim for the legacy `ShellPolicy` enum, kept so
//! `slab-shell-command` and `slab-agent-tools` compile during the migration.
//! New code should use [`crate::PermissionMode`] + [`crate::PermissionBaseline`].

use serde::{Deserialize, Serialize};

use crate::decision::{PermissionBaseline, PermissionMode};

/// Deprecated. Replaced by [`PermissionMode`] + [`PermissionBaseline`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellPolicy {
    #[default]
    Allow,
    RequireApproval,
    Block,
}

impl ShellPolicy {
    /// Map the legacy policy onto the new `(mode, baseline)` pair used to
    /// construct an [`crate::engine::ExecPolicyEngine`].
    pub fn to_mode_baseline(self) -> (PermissionMode, PermissionBaseline) {
        match self {
            Self::Allow => (PermissionMode::Custom, PermissionBaseline::WorkspaceWrite),
            Self::RequireApproval => {
                (PermissionMode::RequestApproval, PermissionBaseline::WorkspaceWrite)
            }
            Self::Block => (PermissionMode::Custom, PermissionBaseline::ReadOnly),
        }
    }
}
