use std::sync::Arc;

use tokio::sync::mpsc;

use crate::runtime::backend::protocol::{BackendOp, BackendReply, BackendRequest};
use crate::runtime::types::{Payload, RuntimeError};

/// Type alias for the boxed synchronous CPU work closure.
///
/// The closure receives an input `Payload` and returns either a new `Payload`
/// or an error string.
pub type CpuFn = Arc<dyn Fn(Payload) -> Result<Payload, String> + Send + Sync + 'static>;

/// Describes a single stage in a pipeline.
///
/// Stages are cloneable descriptions (not running tasks); the orchestrator
/// consumes them when executing a pipeline.
#[derive(Clone, Debug)]
pub enum Stage {
    /// CPU-bound work executed via `tokio::task::spawn_blocking`.
    Cpu(CpuStage),
    /// GPU-bound non-streaming work dispatched to a backend ingress queue.
    Gpu(GpuStage),
    /// GPU-bound streaming work; must be the final stage.
    GpuStream(GpuStreamStage),
}

impl Stage {
    /// Human-readable name used in status reporting.
    pub fn name(&self) -> &str {
        match self {
            Stage::Cpu(s) => &s.name,
            Stage::Gpu(s) => &s.name,
            Stage::GpuStream(s) => &s.name,
        }
    }
}

// ─── CPU Stage ────────────────────────────────────────────────────────────────

/// A stage that runs synchronous, CPU-bound logic inside `spawn_blocking`.
#[derive(Clone)]
pub struct CpuStage {
    /// Display name used in status messages and tracing.
    pub name: String,
    /// The work function; receives an input payload, returns output or error.
    pub work: CpuFn,
}

impl std::fmt::Debug for CpuStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CpuStage")
            .field("name", &self.name)
            .finish()
    }
}

impl CpuStage {
    /// Construct a new `CpuStage` from a name and a synchronous work function.
    pub fn new(
        name: impl Into<String>,
        work: impl Fn(Payload) -> Result<Payload, String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            work: Arc::new(work),
        }
    }

    /// Execute this stage inside `spawn_blocking`, returning the new payload.
    pub async fn run(&self, input: Payload) -> Result<Payload, RuntimeError> {
        let work = Arc::clone(&self.work);
        let name = self.name.clone();
        tokio::task::spawn_blocking(move || work(input))
            .await
            .map_err(|_| RuntimeError::CpuStageFailed {
                stage_name: name.clone(),
                message: "spawn_blocking task panicked".into(),
            })?
            .map_err(|message| RuntimeError::CpuStageFailed {
                stage_name: name,
                message,
            })
    }
}

// ─── GPU Stage ────────────────────────────────────────────────────────────────

/// A stage that dispatches work to a backend actor via a bounded ingress queue.
///
/// Admission control is applied by the orchestrator before calling
/// [`GpuStage::run`].
#[derive(Clone, Debug)]
pub struct GpuStage {
    /// Display name used in status messages and tracing.
    pub name: String,
    /// Identifier used to look up the backend's semaphore and ingress queue.
    pub backend_id: String,
    /// The logical operation forwarded to the backend.
    pub op: BackendOp,
    /// Ingress queue for sending requests to the backend worker.
    pub ingress_tx: mpsc::Sender<BackendRequest>,
}

impl GpuStage {
    /// Dispatch this stage to the backend and await a single reply.
    ///
    /// The caller is responsible for holding an admission `Permit` for the
    /// duration of this call.
    pub async fn run(
        &self,
        input: Payload,
        cancel_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<Payload, RuntimeError> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        let req = BackendRequest {
            op: self.op.clone(),
            input,
            cancel_rx,
            reply_tx,
        };

        self.ingress_tx.try_send(req).map_err(|e| {
            let cap = self.ingress_tx.max_capacity();
            match e {
                mpsc::error::TrySendError::Full(_) => RuntimeError::QueueFull {
                    queue: self.backend_id.clone(),
                    capacity: cap,
                },
                mpsc::error::TrySendError::Closed(_) => RuntimeError::BackendShutdown,
            }
        })?;

        let reply = reply_rx.await.map_err(|_| RuntimeError::BackendShutdown)?;
        match reply {
            BackendReply::Value(payload) => Ok(payload),
            BackendReply::Error(msg) => Err(RuntimeError::GpuStageFailed {
                stage_name: self.name.clone(),
                message: msg,
            }),
            BackendReply::Stream(_) => Err(RuntimeError::GpuStageFailed {
                stage_name: self.name.clone(),
                message: "unexpected stream reply on non-streaming stage".into(),
            }),
        }
    }
}

// ─── GPU Stream Stage ─────────────────────────────────────────────────────────

/// A streaming GPU stage; must be the **final** stage in its pipeline.
///
/// Instead of returning a `Payload`, it returns a `StreamHandle` (an `mpsc`
/// receiver) that yields [`crate::runtime::backend::protocol::StreamChunk`]
/// items.
#[derive(Clone, Debug)]
pub struct GpuStreamStage {
    /// Display name used in status messages and tracing.
    pub name: String,
    /// Identifier used to look up the backend's semaphore and ingress queue.
    pub backend_id: String,
    /// The logical operation forwarded to the backend.
    pub op: BackendOp,
    /// Ingress queue for sending requests to the backend worker.
    pub ingress_tx: mpsc::Sender<BackendRequest>,
}

impl GpuStreamStage {
    /// Dispatch this stage to the backend and return the streaming handle.
    ///
    /// The caller is responsible for holding an admission `Permit` for the
    /// duration of this call.
    pub async fn run(
        &self,
        input: Payload,
        cancel_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<crate::runtime::backend::protocol::StreamHandle, RuntimeError> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        let req = BackendRequest {
            op: self.op.clone(),
            input,
            cancel_rx,
            reply_tx,
        };

        self.ingress_tx.try_send(req).map_err(|e| {
            let cap = self.ingress_tx.max_capacity();
            match e {
                mpsc::error::TrySendError::Full(_) => RuntimeError::QueueFull {
                    queue: self.backend_id.clone(),
                    capacity: cap,
                },
                mpsc::error::TrySendError::Closed(_) => RuntimeError::BackendShutdown,
            }
        })?;

        let reply = reply_rx.await.map_err(|_| RuntimeError::BackendShutdown)?;
        match reply {
            BackendReply::Stream(handle) => Ok(handle),
            BackendReply::Error(msg) => Err(RuntimeError::GpuStageFailed {
                stage_name: self.name.clone(),
                message: msg,
            }),
            BackendReply::Value(_) => Err(RuntimeError::GpuStageFailed {
                stage_name: self.name.clone(),
                message: "expected stream reply but got value".into(),
            }),
        }
    }
}
