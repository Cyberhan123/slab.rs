use std::time::Duration;

use crate::base::types::{Payload, TaskId, TaskStatus};
use crate::scheduler::orchestrator::Orchestrator;
use crate::scheduler::stage::Stage;
use crate::scheduler::types::RuntimeError;

/// Default wait timeout used by the model-first runtime facade.
pub const DEFAULT_WAIT_TIMEOUT: Duration = Duration::from_secs(300);

/// Timeout for waiting until a streaming task exposes its stream handle.
pub const STREAM_INIT_TIMEOUT: Duration = Duration::from_secs(30);

/// Thin internal facade over the orchestrator that centralizes task waiting,
/// result extraction, stream hand-off, and cleanup semantics.
#[derive(Clone, Debug)]
pub struct ExecutionKernel {
    orchestrator: Orchestrator,
}

impl ExecutionKernel {
    pub fn new(orchestrator: Orchestrator) -> Self {
        Self { orchestrator }
    }

    pub fn orchestrator(&self) -> &Orchestrator {
        &self.orchestrator
    }

    /// Submit an execution plan already compiled into internal scheduler stages.
    pub async fn submit(
        &self,
        stages: Vec<Stage>,
        initial_payload: Payload,
    ) -> Result<TaskId, RuntimeError> {
        self.orchestrator.submit(stages, initial_payload).await
    }

    pub async fn snapshot(
        &self,
        task_id: TaskId,
    ) -> Result<crate::scheduler::storage::TaskStatusView, RuntimeError> {
        self.orchestrator.get_status(task_id).await
    }

    pub fn cancel(&self, task_id: TaskId) {
        self.orchestrator.cancel(task_id);
    }

    pub async fn cancel_and_purge(&self, task_id: TaskId) {
        self.orchestrator.cancel_and_purge(task_id).await;
    }

    pub async fn purge(&self, task_id: TaskId) {
        self.orchestrator.purge_task(task_id).await;
    }

    pub async fn take_result(&self, task_id: TaskId) -> Result<Payload, RuntimeError> {
        self.orchestrator
            .get_result(task_id)
            .await
            .ok_or(RuntimeError::TaskNotFound { task_id })
    }

    pub async fn take_stream(
        &self,
        task_id: TaskId,
    ) -> Result<crate::scheduler::backend::protocol::StreamHandle, RuntimeError> {
        self.orchestrator
            .take_stream(task_id)
            .await
            .ok_or(RuntimeError::TaskNotFound { task_id })
    }

    pub async fn wait_terminal(
        &self,
        task_id: TaskId,
        timeout: Duration,
    ) -> Result<TaskStatus, RuntimeError> {
        let wait_result = tokio::time::timeout(timeout, async {
            loop {
                let view = self.orchestrator.get_status(task_id).await?;
                match view.status.clone() {
                    status if status.is_terminal() => return Ok(status),
                    _ => tokio::time::sleep(Duration::from_millis(5)).await,
                }
            }
        })
        .await;

        match wait_result {
            Ok(status) => status,
            Err(_) => {
                self.orchestrator.cancel_and_purge(task_id).await;
                Err(RuntimeError::Timeout)
            }
        }
    }

    pub async fn wait_result(
        &self,
        task_id: TaskId,
        timeout: Duration,
    ) -> Result<Payload, RuntimeError> {
        match self.wait_terminal(task_id, timeout).await? {
            TaskStatus::Succeeded { .. } => self.take_result(task_id).await,
            TaskStatus::ResultConsumed => Err(RuntimeError::GpuStageFailed {
                stage_name: "result".into(),
                message: "task result has already been consumed".into(),
            }),
            TaskStatus::Failed { error } => Err(error),
            TaskStatus::Cancelled => Err(RuntimeError::BackendShutdown),
            TaskStatus::SucceededStreaming => Err(RuntimeError::GpuStageFailed {
                stage_name: "result".into(),
                message: "streaming task has no unary result".into(),
            }),
            TaskStatus::Pending | TaskStatus::Running { .. } => Err(RuntimeError::Timeout),
        }
    }

    pub async fn wait_stream(
        &self,
        task_id: TaskId,
        timeout: Duration,
    ) -> Result<crate::scheduler::backend::protocol::StreamHandle, RuntimeError> {
        let wait_result = tokio::time::timeout(timeout, async {
            loop {
                let view = self.orchestrator.get_status(task_id).await?;
                match view.status {
                    TaskStatus::SucceededStreaming => return Ok(()),
                    TaskStatus::Succeeded { .. } | TaskStatus::ResultConsumed => {
                        return Err(RuntimeError::GpuStageFailed {
                            stage_name: "stream".into(),
                            message: "non-streaming task has no stream".into(),
                        });
                    }
                    TaskStatus::Failed { error } => return Err(error),
                    TaskStatus::Cancelled => return Err(RuntimeError::BackendShutdown),
                    _ => tokio::time::sleep(Duration::from_millis(5)).await,
                }
            }
        })
        .await;

        match wait_result {
            Ok(Ok(())) => self.take_stream(task_id).await,
            Ok(Err(error)) => Err(error),
            Err(_) => {
                self.orchestrator.cancel_and_purge(task_id).await;
                Err(RuntimeError::Timeout)
            }
        }
    }
}
