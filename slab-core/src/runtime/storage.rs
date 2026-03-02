use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use crate::runtime::backend::protocol::StreamHandle;
use crate::runtime::types::{Payload, StageStatus, TaskId, TaskStatus};

/// The complete in-memory record for a single submitted task.
#[derive(Debug)]
pub struct TaskRecord {
    pub task_id: TaskId,
    pub status: TaskStatus,
    pub stage_statuses: Vec<StageStatus>,
    /// Number of pipeline stages for this task.
    pub num_stages: usize,
    /// Streaming handle; `Some` only after a `SucceededStreaming` transition.
    pub stream_handle: Option<StreamHandle>,
    /// Cancellation sender: dropping the orchestrator side signals cancellation.
    pub cancel_tx: Arc<tokio::sync::watch::Sender<bool>>,
}

/// Centralized, thread-safe result storage for all tasks.
///
/// Uses a `tokio::sync::RwLock<HashMap>` so many readers can observe task
/// status concurrently while a single orchestrator writer updates it.
#[derive(Debug, Clone)]
pub struct ResultStorage {
    inner: Arc<RwLock<HashMap<TaskId, TaskRecord>>>,
    next_id: Arc<std::sync::atomic::AtomicU64>,
    /// Bounded channel for submitting work to the orchestrator.
    submit_tx: mpsc::Sender<crate::runtime::orchestrator::OrchestratorCommand>,
}

impl ResultStorage {
    /// Create a new, empty storage instance.
    pub fn new(submit_tx: mpsc::Sender<crate::runtime::orchestrator::OrchestratorCommand>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            submit_tx,
        }
    }

    /// Allocate a new `TaskId` and insert a `Pending` record.
    pub async fn create_task(&self, num_stages: usize) -> TaskId {
        let task_id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let (cancel_tx, _cancel_rx) = tokio::sync::watch::channel(false);

        let record = TaskRecord {
            task_id,
            status: TaskStatus::Pending,
            stage_statuses: vec![StageStatus::StagePending; num_stages],
            num_stages,
            stream_handle: None,
            cancel_tx: Arc::new(cancel_tx),
        };

        self.inner.write().await.insert(task_id, record);
        task_id
    }

    /// Update the status of a task.
    pub async fn set_status(&self, task_id: TaskId, status: TaskStatus) {
        if let Some(record) = self.inner.write().await.get_mut(&task_id) {
            record.status = status;
        }
    }

    /// Update the status of a specific stage.
    pub async fn set_stage_status(&self, task_id: TaskId, stage_index: usize, status: StageStatus) {
        if let Some(record) = self.inner.write().await.get_mut(&task_id) {
            if let Some(s) = record.stage_statuses.get_mut(stage_index) {
                *s = status;
            }
        }
    }

    /// Attach a streaming handle to a task (called after `SucceededStreaming`).
    pub async fn set_stream_handle(&self, task_id: TaskId, handle: StreamHandle) {
        if let Some(record) = self.inner.write().await.get_mut(&task_id) {
            record.stream_handle = Some(handle);
        }
    }

    /// Retrieve the cancellation sender for a task.
    pub async fn get_cancel_tx(
        &self,
        task_id: TaskId,
    ) -> Option<Arc<tokio::sync::watch::Sender<bool>>> {
        self.inner
            .read()
            .await
            .get(&task_id)
            .map(|r| Arc::clone(&r.cancel_tx))
    }

    /// Return a snapshot of the task status.
    pub async fn get_status(&self, task_id: TaskId) -> Option<TaskStatusView> {
        let guard = self.inner.read().await;
        let record = guard.get(&task_id)?;
        Some(TaskStatusView {
            task_id,
            status: record.status.clone(),
            stage_statuses: record.stage_statuses.clone(),
        })
    }

    /// Consume and return the payload for a completed task.
    ///
    /// On success the task transitions to [`TaskStatus::ResultConsumed`] so
    /// that subsequent status queries still report a terminal (succeeded) state
    /// rather than reverting to `Pending`.
    pub async fn take_result(&self, task_id: TaskId) -> Option<Payload> {
        let mut guard = self.inner.write().await;
        let record = guard.get_mut(&task_id)?;
        // Only extract when the task has genuinely succeeded.
        if !matches!(record.status, TaskStatus::Succeeded { .. }) {
            return None;
        }
        // Swap to ResultConsumed so the task remains in a terminal state after
        // the payload is taken; callers checking status will still see
        // "succeeded" rather than the misleading "pending" state.
        let old = std::mem::replace(&mut record.status, TaskStatus::ResultConsumed);
        if let TaskStatus::Succeeded { result } = old {
            Some(result)
        } else {
            None
        }
    }

    /// Consume and return the `StreamHandle` for a streaming task.
    pub async fn take_stream(&self, task_id: TaskId) -> Option<StreamHandle> {
        self.inner
            .write()
            .await
            .get_mut(&task_id)?
            .stream_handle
            .take()
    }

    /// Return a clone of the submit sender (used by pipeline builder).
    pub fn submit_tx(&self) -> mpsc::Sender<crate::runtime::orchestrator::OrchestratorCommand> {
        self.submit_tx.clone()
    }
}

/// A read-only view of a task's current state returned to callers.
#[derive(Debug, Clone)]
pub struct TaskStatusView {
    pub task_id: TaskId,
    pub status: TaskStatus,
    pub stage_statuses: Vec<StageStatus>,
}
