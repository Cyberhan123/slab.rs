use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::runtime::backend::admission::ResourceManager;
use crate::runtime::stage::Stage;
use crate::runtime::storage::ResultStorage;
use crate::runtime::types::{Payload, RuntimeError, StageStatus, TaskId, TaskStatus};

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
                Stage::Cpu(cpu_stage) => {
                    match cpu_stage.run(payload).await {
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

                Stage::Gpu(gpu_stage) => {
                    // Acquire admission permit before dispatching.
                    let permit = match rm.try_acquire(&gpu_stage.backend_id) {
                        Ok(p) => p,
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

                    let result = gpu_stage.run(payload, cancel_rx.clone()).await;
                    drop(permit); // release permit ASAP

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
                    // Streaming stage must be last; acquire permit.
                    let permit = match rm.try_acquire(&stream_stage.backend_id) {
                        Ok(p) => p,
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

                    let result = stream_stage.run(payload, cancel_rx.clone()).await;
                    drop(permit);

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

    /// Return a snapshot of the task's current status.
    pub async fn get_status(
        &self,
        task_id: TaskId,
    ) -> Result<crate::runtime::storage::TaskStatusView, RuntimeError> {
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
    ) -> Option<crate::runtime::backend::protocol::StreamHandle> {
        self.storage.take_stream(task_id).await
    }
}
