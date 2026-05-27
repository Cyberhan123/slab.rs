//! Shared agent types used across `slab-server` and `slab-agent`.
//!
//! These types carry no HTTP, SQL, or transport-layer concerns so they can be
//! freely reused across crate boundaries without pulling in server or runtime
//! dependencies.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// Lifecycle status of a single agent thread.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AgentThreadStatus {
    /// Created but not yet executing.
    Pending,
    /// Actively executing turns.
    Running,
    /// A caller requested cancellation for the current turn.
    Interrupting,
    /// The current turn was cancelled and the thread may accept more input.
    Interrupted,
    /// Finished successfully.
    Completed,
    /// Terminated due to an error.
    Errored,
    /// Terminated by an explicit shutdown request.
    Shutdown,
}

/// Lifecycle status of a single tool call within an agent thread.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ToolCallStatus {
    /// Queued but not yet executing.
    Pending,
    /// Currently executing.
    Running,
    /// Finished successfully.
    Completed,
    /// Terminated due to an error.
    Failed,
}
