//! Tool exposure: which [`OperationCategory`] sets are visible to the agent
//! under the current resolved permission behavior.
//!
//! This drives *progressive tool exposure* — the agent only sees the tools it
//! could actually invoke this turn. In read-only mode the shell / file-write /
//! network tools are hidden from the tool list entirely, not merely blocked
//! post-hoc, so the model can plan around the sandbox instead of discovering
//! its limits by failing.

use crate::category::OperationCategory;
use crate::decision::{PermissionBaseline, PermissionMode};

/// A bit-set over [`OperationCategory`] describing which tool categories the
/// agent is allowed to see and call for the current turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ToolExposure(u8);

impl ToolExposure {
    const SHELL: u8 = 1 << 0;
    const FILE_EDIT: u8 = 1 << 1;
    const NETWORK: u8 = 1 << 2;
    const READ_ONLY: u8 = 1 << 3;

    /// Every category exposed — used under `FullControl` and as the default
    /// when no concrete engine is wired.
    pub fn all() -> Self {
        Self(Self::SHELL | Self::FILE_EDIT | Self::NETWORK | Self::READ_ONLY)
    }

    /// Only read-only tools exposed — used under `StrictReadOnly`.
    pub fn read_only() -> Self {
        Self(Self::READ_ONLY)
    }

    /// Expose an additional category.
    #[must_use]
    pub fn with(self, category: OperationCategory) -> Self {
        Self(self.0 | Self::bit_for(category))
    }

    /// Whether `category` is exposed.
    pub fn contains(self, category: OperationCategory) -> bool {
        self.0 & Self::bit_for(category) != 0
    }

    const fn bit_for(category: OperationCategory) -> u8 {
        match category {
            OperationCategory::Shell => Self::SHELL,
            OperationCategory::FileEdit => Self::FILE_EDIT,
            OperationCategory::Network => Self::NETWORK,
            OperationCategory::ReadOnly => Self::READ_ONLY,
        }
    }
}

/// Read-only snapshot of the resolved permission state for a thread. Serves
/// both the tool-exposure filter (`.exposure`) and the permission-instruction
/// text rendered to the LLM (`.mode` + `.baseline`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermissionStateSnapshot {
    /// The raw per-thread mode (before `ApproveForMe` stub resolution).
    pub mode: PermissionMode,
    /// The global `agent.permissions` baseline.
    pub baseline: PermissionBaseline,
    /// The tool categories the resolved behavior permits this turn.
    pub exposure: ToolExposure,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_exposes_every_category() {
        let exposure = ToolExposure::all();
        assert!(exposure.contains(OperationCategory::Shell));
        assert!(exposure.contains(OperationCategory::FileEdit));
        assert!(exposure.contains(OperationCategory::Network));
        assert!(exposure.contains(OperationCategory::ReadOnly));
    }

    #[test]
    fn read_only_hides_mutations() {
        let exposure = ToolExposure::read_only();
        assert!(exposure.contains(OperationCategory::ReadOnly));
        assert!(!exposure.contains(OperationCategory::Shell));
        assert!(!exposure.contains(OperationCategory::FileEdit));
        assert!(!exposure.contains(OperationCategory::Network));
    }

    #[test]
    fn with_adds_categories() {
        let exposure = ToolExposure::read_only()
            .with(OperationCategory::FileEdit)
            .with(OperationCategory::Shell);
        assert!(exposure.contains(OperationCategory::ReadOnly));
        assert!(exposure.contains(OperationCategory::FileEdit));
        assert!(exposure.contains(OperationCategory::Shell));
        assert!(!exposure.contains(OperationCategory::Network));
    }
}
