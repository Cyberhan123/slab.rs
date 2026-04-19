use std::time::Duration;

use slab_runtime_core::Payload;
use slab_runtime_core::backend::{
    BackendOp, BackendReply, BackendRequest, BackendRequestKind, ManagementEvent, ResourceManager,
    StreamHandle,
};
use tokio::sync::mpsc;
use tracing::info;

use super::error::RuntimeError as CoreError;
use super::stage::Stage;
use super::storage::ResultStorage;
use super::types::{StageStatus, TaskId, TaskStatus};

pub const DEFAULT_WAIT_TIMEOUT: Duration = Duration::from_secs(300);
pub const STREAM_INIT_TIMEOUT: Duration = Duration::from_secs(30);

#[cfg(not(test))]
const GPU_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(30);

#[cfg(test)]
const GPU_ACQUIRE_TIMEOUT: Duration = Duration::from_millis(200);

#[derive(Debug)]
pub enum OrchestratorCommand {
    Submit {
        stages: Vec<Stage>,
        initial_payload: Payload,
        reply_tx: tokio::sync::oneshot::Sender<TaskId>,
    },
}

#[derive(Clone, Debug)]
pub struct Orchestrator {
    storage: ResultStorage,
    resource_manager: ResourceManager,
}

impl Orchestrator {
    async fn call_backend_management_inner(
        &self,
        backend_id: &str,
        event: ManagementEvent,
        op_name: &str,
        input: Payload,
    ) -> Result<Payload, CoreError> {
        let _mgmt_lease = self.resource_manager.acquire_management_lease(backend_id).await?;
        let seq = self.resource_manager.next_seq(backend_id)?;
        let (watch_tx, watch_rx) = tokio::sync::watch::channel(false);
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        drop(watch_tx);

        let request = BackendRequest {
            kind: BackendRequestKind::Management(event),
            op: BackendOp { name: op_name.to_owned(), options: Payload::default() },
            input,
            cancel_rx: watch_rx,
            broadcast_seq: Some(seq),
            reply_tx,
        };

        let ingress_tx = self.resource_manager.ingress_tx(backend_id)?;
        ingress_tx.try_send(request).map_err(|error| {
            let capacity = ingress_tx.max_capacity();
            match error {
                mpsc::error::TrySendError::Full(_) => {
                    CoreError::QueueFull { queue: backend_id.to_owned(), capacity }
                }
                mpsc::error::TrySendError::Closed(_) => CoreError::BackendShutdown,
            }
        })?;

        let reply = reply_rx.await.map_err(|_| CoreError::BackendShutdown)?;
        match reply {
            BackendReply::Value(payload) => Ok(payload),
            BackendReply::Error(message) => {
                Err(CoreError::GpuStageFailed { stage_name: op_name.to_owned(), message })
            }
            BackendReply::Stream(_) => Err(CoreError::GpuStageFailed {
                stage_name: op_name.to_owned(),
                message: "unexpected stream reply on management call".into(),
            }),
        }
    }

    pub fn start(resource_manager: ResourceManager, queue_capacity: usize) -> Self {
        let (submit_tx, submit_rx) = mpsc::channel::<OrchestratorCommand>(queue_capacity);
        let storage = ResultStorage::new(submit_tx);
        let orchestrator = Self { storage: storage.clone(), resource_manager };

        let loop_storage = storage.clone();
        let loop_rm = orchestrator.resource_manager.clone();
        tokio::spawn(async move {
            Self::run_loop(submit_rx, loop_storage, loop_rm).await;
        });

        orchestrator
    }

    async fn run_loop(
        mut rx: mpsc::Receiver<OrchestratorCommand>,
        storage: ResultStorage,
        rm: ResourceManager,
    ) {
        while let Some(command) = rx.recv().await {
            match command {
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
            }
        }
    }

    async fn execute_task(
        task_id: TaskId,
        stages: Vec<Stage>,
        initial_payload: Payload,
        storage: ResultStorage,
        rm: ResourceManager,
    ) {
        let cancel_tx = match storage.get_cancel_tx(task_id).await {
            Some(cancel_tx) => cancel_tx,
            None => return,
        };
        let cancel_rx = cancel_tx.subscribe();
        let mut payload = initial_payload;

        for (index, stage) in stages.iter().enumerate() {
            if *cancel_rx.borrow() {
                storage.set_stage_status(task_id, index, StageStatus::Cancelled).await;
                storage.set_status(task_id, TaskStatus::Cancelled).await;
                info!(task_id, stage_index = index, "task cancelled before stage");
                return;
            }

            storage.set_status(task_id, TaskStatus::Running).await;
            storage.set_stage_status(task_id, index, StageStatus::Running).await;

            match stage {
                Stage::Cpu(cpu_stage) => match cpu_stage.run(payload).await {
                    Ok(next_payload) => {
                        storage.set_stage_status(task_id, index, StageStatus::Completed).await;
                        payload = next_payload;
                    }
                    Err(error) => {
                        storage.set_stage_status(task_id, index, StageStatus::Failed).await;
                        storage.set_status(task_id, TaskStatus::Failed { error }).await;
                        return;
                    }
                },
                Stage::Gpu(gpu_stage) => {
                    let lease = match rm
                        .acquire_inference_lease(&gpu_stage.backend_id, GPU_ACQUIRE_TIMEOUT)
                        .await
                    {
                        Ok(lease) => lease,
                        Err(error) => {
                            storage.set_stage_status(task_id, index, StageStatus::Failed).await;
                            storage
                                .set_status(task_id, TaskStatus::Failed { error: error.into() })
                                .await;
                            return;
                        }
                    };

                    let result = gpu_stage.run(payload, cancel_rx.clone(), &rm).await;
                    drop(lease);

                    match result {
                        Ok(next_payload) => {
                            storage.set_stage_status(task_id, index, StageStatus::Completed).await;
                            payload = next_payload;
                        }
                        Err(error) => {
                            storage.set_stage_status(task_id, index, StageStatus::Failed).await;
                            storage.set_status(task_id, TaskStatus::Failed { error }).await;
                            return;
                        }
                    }
                }
                Stage::GpuStream(stream_stage) => {
                    let lease = match rm
                        .acquire_inference_lease(&stream_stage.backend_id, GPU_ACQUIRE_TIMEOUT)
                        .await
                    {
                        Ok(lease) => lease,
                        Err(error) => {
                            storage.set_stage_status(task_id, index, StageStatus::Failed).await;
                            storage
                                .set_status(task_id, TaskStatus::Failed { error: error.into() })
                                .await;
                            return;
                        }
                    };

                    let result = stream_stage.run(payload, cancel_rx.clone(), &rm).await;
                    drop(lease);

                    match result {
                        Ok(handle) => {
                            storage.set_stage_status(task_id, index, StageStatus::Completed).await;
                            storage.set_status(task_id, TaskStatus::SucceededStreaming).await;
                            storage.set_stream_handle(task_id, handle).await;
                            info!(task_id, "task succeeded (streaming)");
                        }
                        Err(error) => {
                            storage.set_stage_status(task_id, index, StageStatus::Failed).await;
                            storage.set_status(task_id, TaskStatus::Failed { error }).await;
                        }
                    }
                    return;
                }
            }
        }

        storage.set_status(task_id, TaskStatus::Succeeded { result: payload }).await;
        info!(task_id, "task succeeded");
    }

    pub async fn submit(
        &self,
        stages: Vec<Stage>,
        initial_payload: Payload,
    ) -> Result<TaskId, CoreError> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.storage
            .submit_tx()
            .try_send(OrchestratorCommand::Submit { stages, initial_payload, reply_tx })
            .map_err(|error| {
                let capacity = self.storage.submit_tx().max_capacity();
                match error {
                    mpsc::error::TrySendError::Full(_) => {
                        CoreError::OrchestratorQueueFull { capacity }
                    }
                    mpsc::error::TrySendError::Closed(_) => CoreError::BackendShutdown,
                }
            })?;

        reply_rx.await.map_err(|_| CoreError::BackendShutdown)
    }

    pub async fn cancel_and_purge(&self, task_id: TaskId) {
        if let Some(cancel_tx) = self.storage.get_cancel_tx(task_id).await {
            let _ = cancel_tx.send(true);
        }
        self.storage.remove_task(task_id).await;
    }

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

    pub async fn get_status(&self, task_id: TaskId) -> Result<TaskStatus, CoreError> {
        self.storage.get_status(task_id).await.ok_or(CoreError::TaskNotFound { task_id })
    }

    pub async fn purge_task(&self, task_id: TaskId) {
        self.storage.remove_task(task_id).await;
    }

    async fn get_result(&self, task_id: TaskId) -> Option<Payload> {
        self.storage.take_result(task_id).await
    }

    async fn take_stream(&self, task_id: TaskId) -> Option<StreamHandle> {
        self.storage.take_stream(task_id).await
    }

    pub async fn wait_terminal(
        &self,
        task_id: TaskId,
        timeout: Duration,
    ) -> Result<TaskStatus, CoreError> {
        let wait_result = tokio::time::timeout(timeout, async {
            loop {
                let status = self.get_status(task_id).await?;
                match status {
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
            TaskStatus::Pending | TaskStatus::Running => Err(CoreError::Timeout),
        }
    }

    pub async fn wait_stream(
        &self,
        task_id: TaskId,
        timeout: Duration,
    ) -> Result<StreamHandle, CoreError> {
        let wait_result = tokio::time::timeout(timeout, async {
            loop {
                let status = self.get_status(task_id).await?;
                match status {
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
