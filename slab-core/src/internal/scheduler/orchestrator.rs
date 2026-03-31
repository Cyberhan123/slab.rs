use std::collections::HashMap;
use std::time::Duration;

use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::internal::scheduler::backend::admission::ResourceManager;
use crate::internal::scheduler::backend::protocol::{
    BackendOp, BackendReply, BackendRequest, BackendRequestKind, ManagementEvent,
    RuntimeControlSignal, WorkerCommand,
};
use crate::internal::scheduler::stage::Stage;
use crate::internal::scheduler::storage::ResultStorage;
use crate::internal::scheduler::types::{
    BackendLifecycleState, CoreError, GlobalOperationKind, Payload, StageStatus, TaskId, TaskStatus,
};

/// Default wait timeout used when blocking until a task result is ready.
pub const DEFAULT_WAIT_TIMEOUT: Duration = Duration::from_secs(300);

/// Timeout for waiting until a streaming task exposes its stream handle.
pub const STREAM_INIT_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum time to wait for a GPU admission permit before giving up.
///
/// A generous timeout is preferable to an immediate rejection because GPU
/// tasks are typically short-lived and a slot will usually become available
/// within a few seconds.
#[cfg(not(test))]
const GPU_ACQUIRE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Short timeout used in unit tests so that the "no permits available" scenario
/// resolves quickly without slowing down the test suite.
#[cfg(test)]
const GPU_ACQUIRE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(200);

/// Commands sent to the orchestrator's internal event loop.
#[derive(Debug)]
pub enum OrchestratorCommand {
    /// Submit a new pipeline for execution.
    Submit {
        stages: Vec<Stage>,
        initial_payload: Payload,
        /// Channel used to return the allocated `TaskId` to the caller.
        reply_tx: tokio::sync::oneshot::Sender<TaskId>,
    },
    /// Request cancellation of a running task.
    Cancel { task_id: TaskId },
}

/// The runtime orchestrator.
///
/// Accepts pipeline submissions, drives each task's stage-by-stage state
/// machine, and records results in [`ResultStorage`].
///
/// # Usage
///
/// ```rust,ignore
/// let orchestrator = Orchestrator::start(ResourceManager::new(), queue_capacity: 64);
/// let task_id = orchestrator.submit(vec![stage], initial_payload).await?;
/// ```
#[derive(Clone, Debug)]
pub struct Orchestrator {
    storage: ResultStorage,
    resource_manager: ResourceManager,
}

impl Orchestrator {
    #[cfg_attr(not(test), allow(dead_code))]
    fn emit_runtime_control_signal(&self, backend_id: &str, signal: RuntimeControlSignal) {
        let Ok(control_tx) = self.resource_manager.control_tx(backend_id) else {
            return;
        };
        let _ = control_tx.send(WorkerCommand::Runtime(signal));
    }

    async fn call_backend_management_inner(
        &self,
        backend_id: &str,
        event: ManagementEvent,
        op_name: &str,
        input: Payload,
    ) -> Result<Payload, CoreError> {
        let _mgmt_lease = self.resource_manager.acquire_management_lease(backend_id).await?;
        self.resource_manager
            .set_backend_state(backend_id, BackendLifecycleState::Transitioning)
            .await?;

        let seq = self.resource_manager.next_seq(backend_id)?;
        let (watch_tx, watch_rx) = tokio::sync::watch::channel(false);
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        drop(watch_tx);

        let req = BackendRequest {
            kind: BackendRequestKind::Management(event),
            op: BackendOp { name: op_name.to_owned(), options: Payload::default() },
            input,
            cancel_rx: watch_rx,
            broadcast_seq: Some(seq),
            reply_tx,
        };

        let ingress_tx = self.resource_manager.ingress_tx(backend_id)?;
        ingress_tx.try_send(req).map_err(|e| {
            let cap = ingress_tx.max_capacity();
            match e {
                mpsc::error::TrySendError::Full(_) => {
                    CoreError::QueueFull { queue: backend_id.to_owned(), capacity: cap }
                }
                mpsc::error::TrySendError::Closed(_) => CoreError::BackendShutdown,
            }
        })?;

        let reply = reply_rx.await.map_err(|_| CoreError::BackendShutdown)?;
        match reply {
            BackendReply::Value(payload) => {
                let state = match event {
                    ManagementEvent::LoadModel => BackendLifecycleState::ModelLoaded,
                    ManagementEvent::UnloadModel => BackendLifecycleState::Initialized,
                };
                self.resource_manager.set_backend_state(backend_id, state).await?;
                Ok(payload)
            }
            BackendReply::Error(msg) => {
                self.resource_manager
                    .set_backend_state(backend_id, BackendLifecycleState::Error)
                    .await?;
                Err(CoreError::GpuStageFailed { stage_name: op_name.to_owned(), message: msg })
            }
            BackendReply::Stream(_) => Err(CoreError::GpuStageFailed {
                stage_name: op_name.to_owned(),
                message: "unexpected stream reply on management call".into(),
            }),
        }
    }

    /// Start the orchestrator.
    ///
    /// Spawns the internal command-dispatch loop and returns an
    /// `Orchestrator` handle.
    ///
    /// * `resource_manager` – pre-configured admission-control manager.
    /// * `queue_capacity`   – maximum number of pending submissions.
    pub fn start(resource_manager: ResourceManager, queue_capacity: usize) -> Self {
        let (submit_tx, submit_rx) = mpsc::channel::<OrchestratorCommand>(queue_capacity);
        let storage = ResultStorage::new(submit_tx);
        let orchestrator = Self { storage: storage.clone(), resource_manager };

        // Spawn the background dispatch loop.
        let loop_storage = storage.clone();
        let loop_rm = orchestrator.resource_manager.clone();
        tokio::spawn(async move {
            Self::run_loop(submit_rx, loop_storage, loop_rm).await;
        });

        orchestrator
    }

    /// Internal event loop: receives commands and drives task execution.
    async fn run_loop(
        mut rx: mpsc::Receiver<OrchestratorCommand>,
        storage: ResultStorage,
        rm: ResourceManager,
    ) {
        while let Some(cmd) = rx.recv().await {
            match cmd {
                OrchestratorCommand::Submit { stages, initial_payload, reply_tx } => {
                    let task_id = storage.create_task(stages.len()).await;
                    let _ = reply_tx.send(task_id);

                    let task_storage = storage.clone();
                    let task_rm = rm.clone();

                    tokio::spawn(async move {
                        Self::execute_task(task_id, stages, initial_payload, task_storage, task_rm)
                            .await;
                    });
                }

                OrchestratorCommand::Cancel { task_id } => {
                    match storage.get_cancel_tx(task_id).await { Some(tx) => {
                        // Signal cancellation to running task.
                        let _ = tx.send(true);
                        info!(task_id, "cancellation requested");
                    } _ => {
                        warn!(task_id, "cancel: task not found");
                    }}
                }
            }
        }
    }

    /// Drive a single task through all of its stages.
    async fn execute_task(
        task_id: TaskId,
        stages: Vec<Stage>,
        initial_payload: Payload,
        storage: ResultStorage,
        rm: ResourceManager,
    ) {
        let cancel_tx = match storage.get_cancel_tx(task_id).await {
            Some(tx) => tx,
            None => return,
        };
        let cancel_rx = cancel_tx.subscribe();

        let mut payload = initial_payload;

        for (idx, stage) in stages.iter().enumerate() {
            // Check cancellation before each stage.
            if *cancel_rx.borrow() {
                storage.set_stage_status(task_id, idx, StageStatus::Cancelled).await;
                storage.set_status(task_id, TaskStatus::Cancelled).await;
                info!(task_id, stage_index = idx, "task cancelled before stage");
                return;
            }

            storage
                .set_status(
                    task_id,
                    TaskStatus::Running { stage_index: idx, stage_name: stage.name().to_owned() },
                )
                .await;
            storage.set_stage_status(task_id, idx, StageStatus::Running).await;

            match stage {
                Stage::Cpu(cpu_stage) => match cpu_stage.run(payload).await {
                    Ok(next_payload) => {
                        storage.set_stage_status(task_id, idx, StageStatus::Completed).await;
                        payload = next_payload;
                    }
                    Err(err) => {
                        storage.set_stage_status(task_id, idx, StageStatus::Failed).await;
                        storage.set_status(task_id, TaskStatus::Failed { error: err }).await;
                        return;
                    }
                },

                Stage::Gpu(gpu_stage) => {
                    let lease = match rm
                        .acquire_inference_lease(&gpu_stage.backend_id, GPU_ACQUIRE_TIMEOUT)
                        .await
                    {
                        Ok(lease) => lease,
                        Err(err) => {
                            storage.set_stage_status(task_id, idx, StageStatus::Failed).await;
                            storage.set_status(task_id, TaskStatus::Failed { error: err }).await;
                            return;
                        }
                    };

                    let result = gpu_stage.run(payload, cancel_rx.clone(), &rm).await;
                    drop(lease);

                    match result {
                        Ok(next_payload) => {
                            storage.set_stage_status(task_id, idx, StageStatus::Completed).await;
                            payload = next_payload;
                        }
                        Err(err) => {
                            storage.set_stage_status(task_id, idx, StageStatus::Failed).await;
                            storage.set_status(task_id, TaskStatus::Failed { error: err }).await;
                            return;
                        }
                    }
                }

                Stage::GpuStream(stream_stage) => {
                    // Streaming stage must be last; acquire permit with timeout.
                    let lease = match rm
                        .acquire_inference_lease(&stream_stage.backend_id, GPU_ACQUIRE_TIMEOUT)
                        .await
                    {
                        Ok(lease) => lease,
                        Err(err) => {
                            storage.set_stage_status(task_id, idx, StageStatus::Failed).await;
                            storage.set_status(task_id, TaskStatus::Failed { error: err }).await;
                            return;
                        }
                    };

                    let result = stream_stage.run(payload, cancel_rx.clone(), &rm).await;
                    drop(lease);

                    match result {
                        Ok(handle) => {
                            storage.set_stage_status(task_id, idx, StageStatus::Completed).await;
                            storage.set_status(task_id, TaskStatus::SucceededStreaming).await;
                            storage.set_stream_handle(task_id, handle).await;
                            info!(task_id, "task succeeded (streaming)");
                        }
                        Err(err) => {
                            storage.set_stage_status(task_id, idx, StageStatus::Failed).await;
                            storage.set_status(task_id, TaskStatus::Failed { error: err }).await;
                        }
                    }
                    // Streaming stage is always terminal; stop here.
                    return;
                }
            }
        }

        // All stages completed; store final result.
        storage.set_status(task_id, TaskStatus::Succeeded { result: payload }).await;
        info!(task_id, "task succeeded");
    }

    // ── Public API ───────────────────────────────────────────────────────────

    /// Submit a pipeline for execution.
    ///
    /// Returns a [`TaskId`] immediately; execution happens in the background.
    /// Returns [`CoreError::OrchestratorQueueFull`] if the submission queue
    /// is saturated.
    pub async fn submit(
        &self,
        stages: Vec<Stage>,
        initial_payload: Payload,
    ) -> Result<TaskId, CoreError> {
        // Gate GPU-bearing submissions early when global state is inconsistent.
        // This prevents queueing work that is guaranteed to be rejected later.
        if stages.iter().any(|stage| matches!(stage, Stage::Gpu(_) | Stage::GpuStream(_))) {
            self.resource_manager.ensure_inference_allowed().await?;
        }

        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.storage
            .submit_tx()
            .try_send(OrchestratorCommand::Submit { stages, initial_payload, reply_tx })
            .map_err(|e| {
                let cap = self.storage.submit_tx().max_capacity();
                match e {
                    mpsc::error::TrySendError::Full(_) => {
                        CoreError::OrchestratorQueueFull { capacity: cap }
                    }
                    mpsc::error::TrySendError::Closed(_) => CoreError::BackendShutdown,
                }
            })?;

        reply_rx.await.map_err(|_| CoreError::BackendShutdown)
    }

    /// Request best-effort cancellation of a task.
    pub fn cancel(&self, task_id: TaskId) {
        // Best-effort: ignore send errors (task may have already completed).
        let _ = self.storage.submit_tx().try_send(OrchestratorCommand::Cancel { task_id });
    }

    /// Cancel a task and immediately remove its in-memory record.
    ///
    /// Unlike [`cancel`](Self::cancel), this signals cancellation **directly**
    /// through the task's `cancel_tx` watch channel rather than routing through
    /// the orchestrator command queue.  This avoids a race where purging the
    /// record first would prevent the queued cancel command from finding
    /// `cancel_tx`, leaving the task running indefinitely.
    ///
    /// After signalling cancellation the record is removed; subsequent
    /// status/result calls will return [`CoreError::TaskNotFound`].
    pub async fn cancel_and_purge(&self, task_id: TaskId) {
        // Signal cancellation directly so that execute_task sees the watch
        // flag set before the record is removed from storage.
        if let Some(tx) = self.storage.get_cancel_tx(task_id).await {
            let _ = tx.send(true);
        }
        self.storage.remove_task(task_id).await;
    }

    /// Load model for a backend under backend-scoped management lock.
    pub async fn load_model_backend(
        &self,
        backend_id: &str,
        input: Payload,
    ) -> Result<(), CoreError> {
        self.call_backend_management_inner(
            backend_id,
            ManagementEvent::LoadModel,
            "model.load",
            input,
        )
        .await
        .map(|_| ())
    }

    /// Unload model for a backend under backend-scoped management lock.
    pub async fn unload_model_backend(&self, backend_id: &str) -> Result<(), CoreError> {
        self.call_backend_management_inner(
            backend_id,
            ManagementEvent::UnloadModel,
            "model.unload",
            Payload::default(),
        )
        .await
        .map(|_| ())
    }

    /// Execute a global management operation with all-or-fail semantics.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn run_global_management(
        &self,
        kind: GlobalOperationKind,
        payloads: HashMap<String, Payload>,
    ) -> Result<(), CoreError> {
        let op_id = self.resource_manager.begin_global_reconcile().await;
        let backend_ids = self.resource_manager.backend_ids();
        let mut succeeded: Vec<String> = Vec::new();
        let mut failed = false;

        for backend_id in &backend_ids {
            let payload = payloads.get(backend_id).cloned().unwrap_or_default();
            match kind {
                GlobalOperationKind::LoadModels => {
                    self.emit_runtime_control_signal(
                        backend_id,
                        RuntimeControlSignal::GlobalLoad { op_id, payload: payload.clone() },
                    );
                }
            }
            match self
                .call_backend_management_inner(
                    backend_id,
                    ManagementEvent::LoadModel,
                    "model.load",
                    payload,
                )
                .await
            {
                Ok(_) => succeeded.push(backend_id.clone()),
                Err(err) => {
                    let _ = err;
                    failed = true;
                    break;
                }
            }
        }

        if !failed {
            self.resource_manager.mark_global_consistent().await;
            return Ok(());
        }

        match kind {
            GlobalOperationKind::LoadModels => {
                for backend_id in succeeded.iter().rev() {
                    let _ = self.unload_model_backend(backend_id).await;
                }
            }
        }

        self.resource_manager.mark_global_inconsistent(op_id).await;
        Err(CoreError::GlobalStateInconsistent { op_id })
    }

    /// Return a snapshot of the task's current status.
    pub async fn get_status(
        &self,
        task_id: TaskId,
    ) -> Result<crate::internal::scheduler::storage::TaskStatusView, CoreError> {
        self.storage.get_status(task_id).await.ok_or(CoreError::TaskNotFound { task_id })
    }

    /// Consume and return the completed payload for a non-streaming task.
    ///
    /// Returns `None` if the task is not yet completed or was a streaming task.
    pub async fn get_result(&self, task_id: TaskId) -> Option<Payload> {
        self.storage.take_result(task_id).await
    }

    /// Consume and return the [`StreamHandle`] for a streaming task.
    ///
    /// Returns `None` if the task is not yet completed or was a non-streaming task.
    pub async fn take_stream(
        &self,
        task_id: TaskId,
    ) -> Option<crate::internal::scheduler::backend::protocol::StreamHandle> {
        self.storage.take_stream(task_id).await
    }

    /// Remove the in-memory task record for `task_id`.
    ///
    /// Call this once the task has reached a terminal state **and** its result
    /// (or stream handle) has been fully consumed.  Failing to call this on
    /// long-lived processes will cause the task map to grow without bound.
    ///
    /// This is a no-op if `task_id` is not found (e.g. already purged).
    pub async fn purge_task(&self, task_id: TaskId) {
        self.storage.remove_task(task_id).await;
    }

    /// Poll until the task reaches a terminal state or `timeout` expires.
    ///
    /// On timeout the task is cancelled and purged before returning
    /// [`CoreError::Timeout`].
    pub async fn wait_terminal(
        &self,
        task_id: TaskId,
        timeout: Duration,
    ) -> Result<TaskStatus, CoreError> {
        let wait_result = tokio::time::timeout(timeout, async {
            loop {
                let view = self.get_status(task_id).await?;
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
                self.cancel_and_purge(task_id).await;
                Err(CoreError::Timeout)
            }
        }
    }

    /// Wait for a non-streaming task to complete and return its payload.
    pub async fn wait_result(
        &self,
        task_id: TaskId,
        timeout: Duration,
    ) -> Result<Payload, CoreError> {
        match self.wait_terminal(task_id, timeout).await? {
            TaskStatus::Succeeded { .. } => {
                self.get_result(task_id).await.ok_or(CoreError::TaskNotFound { task_id })
            }
            TaskStatus::ResultConsumed => Err(CoreError::GpuStageFailed {
                stage_name: "result".into(),
                message: "task result has already been consumed".into(),
            }),
            TaskStatus::Failed { error } => Err(error),
            TaskStatus::Cancelled => Err(CoreError::Cancelled),
            TaskStatus::SucceededStreaming => Err(CoreError::GpuStageFailed {
                stage_name: "result".into(),
                message: "streaming task has no unary result".into(),
            }),
            TaskStatus::Pending | TaskStatus::Running { .. } => Err(CoreError::Timeout),
        }
    }

    /// Wait for a streaming task to expose its stream handle.
    pub async fn wait_stream(
        &self,
        task_id: TaskId,
        timeout: Duration,
    ) -> Result<crate::internal::scheduler::backend::protocol::StreamHandle, CoreError> {
        let wait_result = tokio::time::timeout(timeout, async {
            loop {
                let view = self.get_status(task_id).await?;
                match view.status {
                    TaskStatus::SucceededStreaming => return Ok(()),
                    TaskStatus::Succeeded { .. } | TaskStatus::ResultConsumed => {
                        return Err(CoreError::GpuStageFailed {
                            stage_name: "stream".into(),
                            message: "non-streaming task has no stream".into(),
                        });
                    }
                    TaskStatus::Failed { error } => return Err(error),
                    TaskStatus::Cancelled => return Err(CoreError::Cancelled),
                    _ => tokio::time::sleep(Duration::from_millis(5)).await,
                }
            }
        })
        .await;

        match wait_result {
            Ok(Ok(())) => {
                self.take_stream(task_id).await.ok_or(CoreError::TaskNotFound { task_id })
            }
            Ok(Err(error)) => Err(error),
            Err(_) => {
                self.cancel_and_purge(task_id).await;
                Err(CoreError::Timeout)
            }
        }
    }
}
