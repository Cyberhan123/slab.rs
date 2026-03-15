use std::time::SystemTime;

// Re-export base types so existing scheduler-internal `use crate::scheduler::types::*`
// imports continue to resolve.
pub use crate::base::error::CoreError;
pub use crate::base::types::{Payload, StageStatus, TaskId, TaskStatus};

/// Backward-compatible alias used throughout the scheduler layer.
pub type RuntimeError = CoreError;

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
#[derive(Debug, Clone)]
pub enum GlobalConsistencyState {
    Consistent {
        generation: u64,
    },
    Reconciling {
        op_id: u64,
        started_at: SystemTime,
    },
    Inconsistent {
        op_id: u64,
        failed_backends: Vec<String>,
        cleanup_report: Vec<String>,
        since: SystemTime,
    },
}

/// Global management operation kind stored for retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalOperationKind {
    InitializeAll,
    LoadModelsAll,
    UnloadModelsAll,
}

/// Snapshot of a failed global operation used for retry.
#[derive(Debug, Clone)]
pub struct FailedGlobalOperation {
    pub kind: GlobalOperationKind,
    pub payloads: std::collections::HashMap<String, Payload>,
}
