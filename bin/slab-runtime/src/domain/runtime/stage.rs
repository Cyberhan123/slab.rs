use std::sync::Arc;

use slab_runtime_core::Payload;
use slab_runtime_core::backend::{
    BackendOp, BackendReply, BackendRequest, BackendRequestKind, ResourceManager, StreamHandle,
};

use super::error::RuntimeError as CoreError;

pub type CpuFn = Arc<dyn Fn(Payload) -> Result<Payload, CoreError> + Send + Sync + 'static>;

#[derive(Clone, Debug)]
pub enum Stage {
    Cpu(CpuStage),
    Gpu(GpuStage),
    GpuStream(GpuStreamStage),
}

#[derive(Clone)]
pub struct CpuStage {
    pub name: String,
    pub work: CpuFn,
}

impl std::fmt::Debug for CpuStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CpuStage").field("name", &self.name).finish()
    }
}

impl CpuStage {
    pub fn new(
        name: impl Into<String>,
        work: impl Fn(Payload) -> Result<Payload, CoreError> + Send + Sync + 'static,
    ) -> Self {
        Self { name: name.into(), work: Arc::new(work) }
    }

    pub async fn run(&self, input: Payload) -> Result<Payload, CoreError> {
        let work = Arc::clone(&self.work);
        let name = self.name.clone();
        let result = tokio::task::spawn_blocking(move || work(input)).await.map_err(|_| {
            CoreError::CpuStageFailed {
                stage_name: name.clone(),
                message: "spawn_blocking task panicked".into(),
            }
        })?;

        result.map_err(|error| match error {
            CoreError::CpuStageFailed { .. } => error,
            other => CoreError::CpuStageFailed { stage_name: name, message: other.to_string() },
        })
    }
}

#[derive(Clone, Debug)]
pub struct GpuStage {
    pub name: String,
    pub backend_id: String,
    pub op: BackendOp,
}

impl GpuStage {
    pub async fn run(
        &self,
        input: Payload,
        cancel_rx: tokio::sync::watch::Receiver<bool>,
        rm: &ResourceManager,
    ) -> Result<Payload, CoreError> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        let ingress_tx = rm.ingress_tx(&self.backend_id)?;
        let req = BackendRequest {
            kind: BackendRequestKind::Inference,
            op: self.op.clone(),
            input,
            cancel_rx,
            broadcast_seq: None,
            reply_tx,
        };

        ingress_tx.try_send(req).map_err(|error| {
            let capacity = ingress_tx.max_capacity();
            match error {
                tokio::sync::mpsc::error::TrySendError::Full(_) => {
                    CoreError::QueueFull { queue: self.backend_id.clone(), capacity }
                }
                tokio::sync::mpsc::error::TrySendError::Closed(_) => CoreError::BackendShutdown,
            }
        })?;

        let reply = reply_rx.await.map_err(|_| CoreError::BackendShutdown)?;
        match reply {
            BackendReply::Value(payload) => Ok(payload),
            BackendReply::Error(message) => {
                Err(CoreError::GpuStageFailed { stage_name: self.name.clone(), message })
            }
            BackendReply::Stream(_) => Err(CoreError::GpuStageFailed {
                stage_name: self.name.clone(),
                message: "unexpected stream reply on non-streaming stage".into(),
            }),
        }
    }
}

#[derive(Clone, Debug)]
pub struct GpuStreamStage {
    pub name: String,
    pub backend_id: String,
    pub op: BackendOp,
}

impl GpuStreamStage {
    pub async fn run(
        &self,
        input: Payload,
        cancel_rx: tokio::sync::watch::Receiver<bool>,
        rm: &ResourceManager,
    ) -> Result<StreamHandle, CoreError> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        let ingress_tx = rm.ingress_tx(&self.backend_id)?;
        let req = BackendRequest {
            kind: BackendRequestKind::Inference,
            op: self.op.clone(),
            input,
            cancel_rx,
            broadcast_seq: None,
            reply_tx,
        };

        ingress_tx.try_send(req).map_err(|error| {
            let capacity = ingress_tx.max_capacity();
            match error {
                tokio::sync::mpsc::error::TrySendError::Full(_) => {
                    CoreError::QueueFull { queue: self.backend_id.clone(), capacity }
                }
                tokio::sync::mpsc::error::TrySendError::Closed(_) => CoreError::BackendShutdown,
            }
        })?;

        let reply = reply_rx.await.map_err(|_| CoreError::BackendShutdown)?;
        match reply {
            BackendReply::Stream(handle) => Ok(handle),
            BackendReply::Error(message) => {
                Err(CoreError::GpuStageFailed { stage_name: self.name.clone(), message })
            }
            BackendReply::Value(_) => Err(CoreError::GpuStageFailed {
                stage_name: self.name.clone(),
                message: "expected stream reply but got value".into(),
            }),
        }
    }
}
