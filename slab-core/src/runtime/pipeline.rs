use std::marker::PhantomData;
use tokio::sync::mpsc;

use crate::runtime::backend::protocol::BackendOp;
use crate::runtime::orchestrator::Orchestrator;
use crate::runtime::stage::{CpuStage, GpuStage, GpuStreamStage, Stage};
use crate::runtime::types::{Payload, RuntimeError, TaskId};

// ─── Typestate markers ────────────────────────────────────────────────────────

/// Marker: the pipeline has no streaming terminal stage yet.
pub struct NoStream;

/// Marker: the pipeline ends with a streaming stage; no more stages can be added.
pub struct HasStream;

// ─── Builder ──────────────────────────────────────────────────────────────────

/// Declarative pipeline builder.
///
/// Use the fluent API to append stages, then call [`run`] (non-streaming) or
/// [`run_stream`] (streaming terminal) to submit to the [`Orchestrator`].
///
/// # Typestate invariant
///
/// Once [`gpu_stream`] is called the builder transitions to `PipelineBuilder<HasStream>`,
/// which only exposes [`run_stream`].  This enforces the *streaming-is-terminal*
/// constraint at compile time.
///
/// [`run`]: PipelineBuilder::<NoStream>::run
/// [`run_stream`]: PipelineBuilder::<HasStream>::run_stream
/// [`gpu_stream`]: PipelineBuilder::<NoStream>::gpu_stream
pub struct PipelineBuilder<S = NoStream> {
    orchestrator: Orchestrator,
    stages: Vec<Stage>,
    initial_payload: Payload,
    _state: PhantomData<S>,
}

impl PipelineBuilder<NoStream> {
    /// Create a new builder bound to the given orchestrator and initial payload.
    pub fn new(orchestrator: Orchestrator, initial_payload: Payload) -> Self {
        Self {
            orchestrator,
            stages: Vec::new(),
            initial_payload,
            _state: PhantomData,
        }
    }

    /// Append a CPU stage.
    ///
    /// `work` receives the current payload and returns the transformed payload.
    /// It is executed inside `tokio::task::spawn_blocking`.
    pub fn cpu(
        mut self,
        name: impl Into<String>,
        work: impl Fn(Payload) -> Result<Payload, String> + Send + Sync + 'static,
    ) -> Self {
        self.stages.push(Stage::Cpu(CpuStage::new(name, work)));
        self
    }

    /// Append a CPU stage from a pre-built [`CpuStage`].
    pub fn cpu_stage(mut self, stage: CpuStage) -> Self {
        self.stages.push(Stage::Cpu(stage));
        self
    }

    /// Append a non-streaming GPU stage.
    pub fn gpu(
        mut self,
        name: impl Into<String>,
        backend_id: impl Into<String>,
        op: BackendOp,
        ingress_tx: mpsc::Sender<crate::runtime::backend::protocol::BackendRequest>,
    ) -> Self {
        let stage = GpuStage {
            name: name.into(),
            backend_id: backend_id.into(),
            op,
            ingress_tx,
        };
        self.stages.push(Stage::Gpu(stage));
        self
    }

    /// Append a GPU stage from a pre-built [`GpuStage`].
    pub fn gpu_stage(mut self, stage: GpuStage) -> Self {
        self.stages.push(Stage::Gpu(stage));
        self
    }

    /// Append a streaming terminal GPU stage.
    ///
    /// Transitions the builder to `PipelineBuilder<HasStream>`, preventing
    /// further stage additions.
    pub fn gpu_stream(
        mut self,
        name: impl Into<String>,
        backend_id: impl Into<String>,
        op: BackendOp,
        ingress_tx: mpsc::Sender<crate::runtime::backend::protocol::BackendRequest>,
    ) -> PipelineBuilder<HasStream> {
        let stage = GpuStreamStage {
            name: name.into(),
            backend_id: backend_id.into(),
            op,
            ingress_tx,
        };
        self.stages.push(Stage::GpuStream(stage));
        PipelineBuilder {
            orchestrator: self.orchestrator,
            stages: self.stages,
            initial_payload: self.initial_payload,
            _state: PhantomData,
        }
    }

    /// Append a streaming terminal GPU stage from a pre-built [`GpuStreamStage`].
    pub fn gpu_stream_stage(mut self, stage: GpuStreamStage) -> PipelineBuilder<HasStream> {
        self.stages.push(Stage::GpuStream(stage));
        PipelineBuilder {
            orchestrator: self.orchestrator,
            stages: self.stages,
            initial_payload: self.initial_payload,
            _state: PhantomData,
        }
    }

    /// Submit the pipeline for execution and return the allocated [`TaskId`].
    pub async fn run(self) -> Result<TaskId, RuntimeError> {
        self.orchestrator
            .submit(self.stages, self.initial_payload)
            .await
    }
}

impl PipelineBuilder<HasStream> {
    /// Submit the streaming pipeline and return the allocated [`TaskId`].
    ///
    /// After the task transitions to `SucceededStreaming`, call
    /// [`Orchestrator::take_stream`] with the returned [`TaskId`] to obtain the
    /// stream handle.
    pub async fn run_stream(self) -> Result<TaskId, RuntimeError> {
        self.orchestrator
            .submit(self.stages, self.initial_payload)
            .await
    }
}
