//! Plan-and-execute value types: the durable [`Plan`] a thread authors in
//! Plan interaction mode and the per-step status vocabulary.
//!
//! These types are the wire shape stored by [`crate::port::PlanStorePort`] and
//! authored by the `plan` / `update_plan` tools (in `slab-agent-tools`). They
//! live here — not in `slab-agent-tools` — so the port trait (defined in this
//! crate) can reference them without a reverse dependency.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Lifecycle status of a single plan step.
#[derive(TS, Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PlanStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Blocked,
}

impl PlanStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Blocked => "blocked",
        }
    }
}

/// A single step in a plan.
#[derive(TS, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[ts(export)]
pub struct PlanItem {
    /// What to do for this step.
    pub step: String,
    /// Lifecycle status of the step.
    pub status: PlanStatus,
    /// Lightweight dependencies (step references), not a full DAG. Carried for
    /// rendering; not enforced by the plan-and-execute state machine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
    /// Optional `verify:<target>:<passed|failed>` reference produced by the
    /// `verify` tool, binding execution evidence to a (usually completed) step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_ref: Option<String>,
}

/// Aggregate step counts by status.
#[derive(TS, Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[ts(export)]
pub struct PlanCounts {
    pub pending: usize,
    pub in_progress: usize,
    pub completed: usize,
    pub blocked: usize,
}

/// A structured, durable plan authored by the agent in Plan interaction mode.
///
/// Produced by normalizing the `plan` / `update_plan` tool arguments. The
/// `plan_id` is generated on first creation; the store keys plans by thread id.
#[derive(TS, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[ts(export)]
pub struct Plan {
    /// Stable identifier (generated when the `plan` tool creates the plan).
    pub plan_id: String,
    /// Optional human-readable summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub items: Vec<PlanItem>,
    pub counts: PlanCounts,
    /// Index of the current (in-progress) step, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_step: Option<usize>,
}

impl Plan {
    /// Render a short single-line summary suitable for an approval prompt or
    /// tool-result header.
    pub fn summary_line(&self) -> String {
        let head = self.summary.clone().unwrap_or_else(|| "plan".to_owned());
        format!(
            "{} ({} steps: {} done, {} in progress, {} pending)",
            head,
            self.items.len(),
            self.counts.completed,
            self.counts.in_progress,
            self.counts.pending,
        )
    }
}
