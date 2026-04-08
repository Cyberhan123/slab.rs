// Re-export base types so scheduler-internal modules can share a compact
// `types::*` import surface without pulling from `base` directly at each call site.
pub use crate::base::error::CoreError;
pub use crate::base::types::{Payload, StageStatus, TaskId, TaskStatus};

/// Backend lifecycle state tracked centrally by the resource manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendLifecycleState {
    Uninitialized,
    Initialized,
    ModelLoaded,
    Transitioning,
    Error,
}

/// Cluster-wide consistency state used to gate inference after failed global operations.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone)]
pub enum GlobalConsistencyState {
    Consistent,
    Reconciling,
    Inconsistent { op_id: u64 },
}

/// Global management operation kind exercised by the internal scheduler tests.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalOperationKind {
    LoadModels,
}
