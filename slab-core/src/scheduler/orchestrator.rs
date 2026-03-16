use std::collections::HashMap;

use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::scheduler::backend::admission::ResourceManager;
use crate::scheduler::backend::protocol::{
    BackendOp, BackendReply, BackendRequest, BackendRequestKind, ManagementEvent,
    RuntimeControlSignal, WorkerCommand,
};
use crate::scheduler::stage::Stage;
use crate::scheduler::storage::ResultStorage;
use crate::scheduler::types::{
    BackendLifecycleState, FailedGlobalOperation, GlobalConsistencyState, GlobalOperationKind,
    Payload, RuntimeError, StageStatus, TaskId, TaskStatus,
};

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
    fn emit_runtime_control_signal(&self, backend_id: &str, signal: RuntimeControlSignal) {
        let Ok(control_tx) = self.resource_manager.control_tx(backend_id) else {
            return;
        };
        let _ = control_tx.send(WorkerCommand::Runtime(signal));
    }

    fn global_kind_to_event(kind: GlobalOperationKind) -> (ManagementEvent, &'static str) {
        match kind {
            GlobalOperationKind::Initialize => (ManagementEvent::Initialize, "lib.load"),
            GlobalOperationKind::LoadModels => (ManagementEvent::LoadModel, "model.load"),
            GlobalOperationKind::UnloadModels => (ManagementEvent::UnloadModel, "model.unload"),
        }
    }

    async fn call_backend_management_inner(
        &self,
        backend_id: &str,
        event: ManagementEvent,
        op_name: &str,
        input: Payload,
    ) -> Result<Payload, RuntimeError> {
        let _mgmt_lease = self
            .resource_manager
            .acquire_management_lease(backend_id)
            .await?;
        self.resource_manager
            .set_backend_state(backend_id, BackendLifecycleState::Transitioning)
            .await?;

        let seq = self.resource_manager.next_seq(backend_id)?;
        let (watch_tx, watch_rx) = tokio::sync::watch::channel(false);
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        drop(watch_tx);

        let req = BackendRequest {
            kind: BackendRequestKind::Management(event),
            op: BackendOp {
                name: op_name.to_owned(),
                options: Payload::default(),
            },
            input,
            cancel_rx: watch_rx,
            broadcast_seq: Some(seq),
            reply_tx,
        };

        let ingress_tx = self.resource_manager.ingress_tx(backend_id)?;
        ingress_tx.try_send(req).map_err(|e| {
            let cap = ingress_tx.max_capacity();
            match e {
                mpsc::error::TrySendError::Full(_) => RuntimeError::QueueFull {
                    queue: backend_id.to_owned(),
                    capacity: cap,
                },
                mpsc::error::TrySendError::Closed(_) => RuntimeError::BackendShutdown,
            }
        })?;

        let reply = reply_rx.await.map_err(|_| RuntimeError::BackendShutdown)?;
        match reply {
            BackendReply::Value(payload) => {
                let state = match event {
                    ManagementEvent::Initialize => BackendLifecycleState::Initialized,
                    ManagementEvent::LoadModel => BackendLifecycleState::ModelLoaded,
                    ManagementEvent::UnloadModel => BackendLifecycleState::Initialized,
                };
                self.resource_manager
                    .set_backend_state(backend_id, state)
                    .await?;
                Ok(payload)
            }
            BackendReply::Error(msg) => {
                self.resource_manager
                    .set_backend_state(backend_id, BackendLifecycleState::Error)
                    .await?;
                Err(RuntimeError::GpuStageFailed {
                    stage_name: op_name.to_owned(),
                    message: msg,
                })
            }
            BackendReply::Stream(_) => Err(RuntimeError::GpuStageFailed {
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
        let orchestrator = Self {
            storage: storage.clone(),
            resource_manager,
        };

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
                OrchestratorCommand::Submit {
                    stages,
                    initial_payload,
                    reply_tx,
                } => {
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
                    if let Some(tx) = storage.get_cancel_tx(task_id).await {
                        // Signal cancellation to running task.
                        let _ = tx.send(true);
                        info!(task_id, "cancellation requested");
                    } else {
                        warn!(task_id, "cancel: task not found");
                    }
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
                storage
                    .set_stage_status(task_id, idx, StageStatus::StageCancelled)
                    .await;
                storage.set_status(task_id, TaskStatus::Cancelled).await;
                info!(task_id, stage_index = idx, "task cancelled before stage");
                return;
            }

            storage
                .set_status(
                    task_id,
                    TaskStatus::Running {
                        stage_index: idx,
                        stage_name: stage.name().to_owned(),
                    },
                )
                .await;
            storage
                .set_stage_status(task_id, idx, StageStatus::StageRunning)
                .await;

            match stage {
                Stage::Cpu(cpu_stage) => match cpu_stage.run(payload).await {
                    Ok(next_payload) => {
                        storage
                            .set_stage_status(task_id, idx, StageStatus::StageCompleted)
                            .await;
                        payload = next_payload;
                    }
                    Err(err) => {
                        storage
                            .set_stage_status(task_id, idx, StageStatus::StageFailed)
                            .await;
                        storage
                            .set_status(task_id, TaskStatus::Failed { error: err })
                            .await;
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
                            storage
                                .set_stage_status(task_id, idx, StageStatus::StageFailed)
                                .await;
                            storage
                                .set_status(task_id, TaskStatus::Failed { error: err })
                                .await;
                            return;
                        }
                    };

                    let result = gpu_stage.run(payload, cancel_rx.clone(), &rm).await;
                    drop(lease);

                    match result {
                        Ok(next_payload) => {
                            storage
                                .set_stage_status(task_id, idx, StageStatus::StageCompleted)
                                .await;
                            payload = next_payload;
                        }
                        Err(err) => {
                            storage
                                .set_stage_status(task_id, idx, StageStatus::StageFailed)
                                .await;
                            storage
                                .set_status(task_id, TaskStatus::Failed { error: err })
                                .await;
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
                            storage
                                .set_stage_status(task_id, idx, StageStatus::StageFailed)
                                .await;
                            storage
                                .set_status(task_id, TaskStatus::Failed { error: err })
                                .await;
                            return;
                        }
                    };

                    let result = stream_stage.run(payload, cancel_rx.clone(), &rm).await;
                    drop(lease);

                    match result {
                        Ok(handle) => {
                            storage
                                .set_stage_status(task_id, idx, StageStatus::StageCompleted)
                                .await;
                            storage
                                .set_status(task_id, TaskStatus::SucceededStreaming)
                                .await;
                            storage.set_stream_handle(task_id, handle).await;
                            info!(task_id, "task succeeded (streaming)");
                        }
                        Err(err) => {
                            storage
                                .set_stage_status(task_id, idx, StageStatus::StageFailed)
                                .await;
                            storage
                                .set_status(task_id, TaskStatus::Failed { error: err })
                                .await;
                        }
                    }
                    // Streaming stage is always terminal; stop here.
                    return;
                }
            }
        }

        // All stages completed; store final result.
        storage
            .set_status(task_id, TaskStatus::Succeeded { result: payload })
            .await;
        info!(task_id, "task succeeded");
    }

    // ── Public API ───────────────────────────────────────────────────────────

    /// Submit a pipeline for execution.
    ///
    /// Returns a [`TaskId`] immediately; execution happens in the background.
    /// Returns [`RuntimeError::OrchestratorQueueFull`] if the submission queue
    /// is saturated.
    pub async fn submit(
        &self,
        stages: Vec<Stage>,
        initial_payload: Payload,
    ) -> Result<TaskId, RuntimeError> {
        // Gate GPU-bearing submissions early when global state is inconsistent.
        // This prevents queueing work that is guaranteed to be rejected later.
        if stages
            .iter()
            .any(|stage| matches!(stage, Stage::Gpu(_) | Stage::GpuStream(_)))
        {
            self.resource_manager.ensure_inference_allowed().await?;
        }

        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.storage
            .submit_tx()
            .try_send(OrchestratorCommand::Submit {
                stages,
                initial_payload,
                reply_tx,
            })
            .map_err(|e| {
                let cap = self.storage.submit_tx().max_capacity();
                match e {
                    mpsc::error::TrySendError::Full(_) => {
                        RuntimeError::OrchestratorQueueFull { capacity: cap }
                    }
                    mpsc::error::TrySendError::Closed(_) => RuntimeError::BackendShutdown,
                }
            })?;

        reply_rx.await.map_err(|_| RuntimeError::BackendShutdown)
    }

    /// Request best-effort cancellation of a task.
    pub fn cancel(&self, task_id: TaskId) {
        // Best-effort: ignore send errors (task may have already completed).
        let _ = self
            .storage
            .submit_tx()
            .try_send(OrchestratorCommand::Cancel { task_id });
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
    /// status/result calls will return [`RuntimeError::TaskNotFound`].
    pub async fn cancel_and_purge(&self, task_id: TaskId) {
        // Signal cancellation directly so that execute_task sees the watch
        // flag set before the record is removed from storage.
        if let Some(tx) = self.storage.get_cancel_tx(task_id).await {
            let _ = tx.send(true);
        }
        self.storage.remove_task(task_id).await;
    }

    /// Run backend initialization (`lib.load`) under backend-scoped management lock.
    pub async fn initialize_backend(
        &self,
        backend_id: &str,
        input: Payload,
    ) -> Result<(), RuntimeError> {
        self.call_backend_management_inner(
            backend_id,
            ManagementEvent::Initialize,
            "lib.load",
            input,
        )
        .await
        .map(|_| ())
    }

    /// Reload backend dynamic library (`lib.reload`) under backend-scoped management lock.
    pub async fn reload_library_backend(
        &self,
        backend_id: &str,
        input: Payload,
    ) -> Result<(), RuntimeError> {
        self.call_backend_management_inner(
            backend_id,
            ManagementEvent::Initialize,
            "lib.reload",
            input,
        )
        .await
        .map(|_| ())
    }

    /// Load model for a backend under backend-scoped management lock.
    pub async fn load_model_backend(
        &self,
        backend_id: &str,
        input: Payload,
    ) -> Result<(), RuntimeError> {
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
    pub async fn unload_model_backend(&self, backend_id: &str) -> Result<(), RuntimeError> {
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
    pub async fn run_global_management(
        &self,
        kind: GlobalOperationKind,
        payloads: HashMap<String, Payload>,
    ) -> Result<(), RuntimeError> {
        let _global_guard = self.resource_manager.lock_global_management().await;
        let op_id = self.resource_manager.begin_global_reconcile().await;

        let (event, op_name) = Self::global_kind_to_event(kind);
        let backend_ids = self.resource_manager.backend_ids();
        let mut succeeded: Vec<String> = Vec::new();
        let mut failed: Vec<(String, RuntimeError)> = Vec::new();

        for backend_id in &backend_ids {
            let payload = payloads.get(backend_id).cloned().unwrap_or_default();
            match kind {
                GlobalOperationKind::LoadModels => {
                    self.emit_runtime_control_signal(
                        backend_id,
                        RuntimeControlSignal::GlobalLoad {
                            op_id,
                            payload: payload.clone(),
                        },
                    );
                }
                GlobalOperationKind::UnloadModels => {
                    self.emit_runtime_control_signal(
                        backend_id,
                        RuntimeControlSignal::GlobalUnload { op_id },
                    );
                }
                GlobalOperationKind::Initialize => {}
            }
            match self
                .call_backend_management_inner(backend_id, event, op_name, payload)
                .await
            {
                Ok(_) => succeeded.push(backend_id.clone()),
                Err(err) => {
                    failed.push((backend_id.clone(), err));
                    break;
                }
            }
        }

        if failed.is_empty() {
            self.resource_manager.mark_global_consistent().await;
            return Ok(());
        }

        let mut cleanup_report = Vec::new();
        match kind {
            GlobalOperationKind::LoadModels => {
                for backend_id in succeeded.iter().rev() {
                    if let Err(err) = self.unload_model_backend(backend_id).await {
                        cleanup_report.push(format!(
                            "cleanup unload failed for backend '{}': {}",
                            backend_id, err
                        ));
                    }
                }
            }
            GlobalOperationKind::UnloadModels => {
                for (backend_id, _) in &failed {
                    if let Err(err) = self.unload_model_backend(backend_id).await {
                        cleanup_report.push(format!(
                            "unload retry failed for backend '{}': {}",
                            backend_id, err
                        ));
                    }
                }
            }
            GlobalOperationKind::Initialize => {
                for backend_id in succeeded.iter().rev() {
                    if let Err(err) = self.unload_model_backend(backend_id).await {
                        cleanup_report.push(format!(
                            "best-effort initialize cleanup failed for backend '{}': {}",
                            backend_id, err
                        ));
                    }
                }
            }
        }

        let failed_backends: Vec<String> = failed.iter().map(|(id, _)| id.clone()).collect();
        self.resource_manager
            .mark_global_inconsistent(
                op_id,
                failed_backends,
                cleanup_report,
                FailedGlobalOperation { kind, payloads },
            )
            .await;
        Err(RuntimeError::GlobalStateInconsistent { op_id })
    }

    /// Retry the most recent failed global operation.
    pub async fn retry_last_failed_global(&self) -> Result<(), RuntimeError> {
        let failed = self
            .resource_manager
            .failed_global_operation()
            .await
            .ok_or(RuntimeError::NoFailedGlobalOperation)?;
        self.run_global_management(failed.kind, failed.payloads)
            .await
    }

    /// Read current global consistency state.
    pub async fn consistency_status(&self) -> GlobalConsistencyState {
        self.resource_manager.global_state().await
    }

    /// Read backend lifecycle state from the resource manager.
    pub async fn backend_state(
        &self,
        backend_id: &str,
    ) -> Result<BackendLifecycleState, RuntimeError> {
        self.resource_manager.backend_state(backend_id).await
    }

    /// Manual operator override for inconsistency gate.
    pub async fn manual_mark_consistent(&self, reason: &str) {
        self.resource_manager.manual_mark_consistent(reason).await;
    }

    /// Return a snapshot of the task's current status.
    pub async fn get_status(
        &self,
        task_id: TaskId,
    ) -> Result<crate::scheduler::storage::TaskStatusView, RuntimeError> {
        self.storage
            .get_status(task_id)
            .await
            .ok_or(RuntimeError::TaskNotFound { task_id })
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
    ) -> Option<crate::scheduler::backend::protocol::StreamHandle> {
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
}
