pub mod base;
mod engine;
mod scheduler;

pub mod api;
pub mod ports;

pub use base::error::CoreError;
pub use base::types::{Payload, TaskId, TaskStatus};
pub use scheduler::storage::TaskStatusView;

/// Backward-compatible alias: `RuntimeError` is now [`CoreError`].
pub type RuntimeError = CoreError;
