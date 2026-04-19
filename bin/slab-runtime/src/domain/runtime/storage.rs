use std::collections::HashMap;
use std::sync::Arc;

use slab_runtime_core::Payload;
use slab_runtime_core::backend::StreamHandle;
use tokio::sync::{RwLock, mpsc};

use super::orchestrator::OrchestratorCommand;
use super::types::{StageStatus, TaskId, TaskStatus};

#[derive(Debug)]
pub struct TaskRecord {
    pub status: TaskStatus,
    pub stage_statuses: Vec<StageStatus>,
    pub stream_handle: Option<StreamHandle>,
    pub cancel_tx: Arc<tokio::sync::watch::Sender<bool>>,
}

#[derive(Debug, Clone)]
pub struct ResultStorage {
    inner: Arc<RwLock<HashMap<TaskId, TaskRecord>>>,
    next_id: Arc<std::sync::atomic::AtomicU64>,
    submit_tx: mpsc::Sender<OrchestratorCommand>,
}

impl ResultStorage {
    pub fn new(submit_tx: mpsc::Sender<OrchestratorCommand>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            submit_tx,
        }
    }

    pub async fn create_task(&self, num_stages: usize) -> TaskId {
        let task_id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (cancel_tx, _cancel_rx) = tokio::sync::watch::channel(false);

        let record = TaskRecord {
            status: TaskStatus::Pending,
            stage_statuses: vec![StageStatus::Pending; num_stages],
            stream_handle: None,
            cancel_tx: Arc::new(cancel_tx),
        };

        self.inner.write().await.insert(task_id, record);
        task_id
    }

    pub async fn set_status(&self, task_id: TaskId, status: TaskStatus) {
        if let Some(record) = self.inner.write().await.get_mut(&task_id) {
            record.status = status;
        }
    }

    pub async fn set_stage_status(&self, task_id: TaskId, stage_index: usize, status: StageStatus) {
        if let Some(record) = self.inner.write().await.get_mut(&task_id)
            && let Some(stage) = record.stage_statuses.get_mut(stage_index)
        {
            *stage = status;
        }
    }

    pub async fn set_stream_handle(&self, task_id: TaskId, handle: StreamHandle) {
        if let Some(record) = self.inner.write().await.get_mut(&task_id) {
            record.stream_handle = Some(handle);
        }
    }

    pub async fn get_cancel_tx(
        &self,
        task_id: TaskId,
    ) -> Option<Arc<tokio::sync::watch::Sender<bool>>> {
        self.inner.read().await.get(&task_id).map(|record| Arc::clone(&record.cancel_tx))
    }

    pub async fn get_status(&self, task_id: TaskId) -> Option<TaskStatus> {
        let guard = self.inner.read().await;
        let record = guard.get(&task_id)?;
        Some(record.status.clone())
    }

    pub async fn take_result(&self, task_id: TaskId) -> Option<Payload> {
        let mut guard = self.inner.write().await;
        let record = guard.get_mut(&task_id)?;
        if !matches!(record.status, TaskStatus::Succeeded { .. }) {
            return None;
        }

        let old_status = std::mem::replace(&mut record.status, TaskStatus::ResultConsumed);
        if let TaskStatus::Succeeded { result } = old_status { Some(result) } else { None }
    }

    pub async fn take_stream(&self, task_id: TaskId) -> Option<StreamHandle> {
        self.inner.write().await.get_mut(&task_id)?.stream_handle.take()
    }

    pub async fn remove_task(&self, task_id: TaskId) {
        self.inner.write().await.remove(&task_id);
    }

    pub fn submit_tx(&self) -> mpsc::Sender<OrchestratorCommand> {
        self.submit_tx.clone()
    }
}
