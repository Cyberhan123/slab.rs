use slab_runtime_core::Payload;

use super::error::RuntimeError;

pub type TaskId = u64;

#[derive(Debug, Clone)]
pub enum TaskStatus {
    Pending,
    Running,
    Succeeded { result: Payload },
    ResultConsumed,
    SucceededStreaming,
    Failed { error: RuntimeError },
    Cancelled,
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskStatus::Succeeded { .. }
                | TaskStatus::ResultConsumed
                | TaskStatus::SucceededStreaming
                | TaskStatus::Failed { .. }
                | TaskStatus::Cancelled
        )
    }
}

#[derive(Debug, Clone)]
pub enum StageStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}
