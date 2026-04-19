use std::marker::PhantomData;

use slab_runtime_core::Payload;
use slab_runtime_core::backend::BackendOp;

use super::error::RuntimeError as CoreError;
use super::orchestrator::Orchestrator;
use super::stage::{CpuStage, GpuStage, GpuStreamStage, Stage};
use super::types::TaskId;

pub struct NoStream;

pub struct HasStream;

pub struct PipelineBuilder<S = NoStream> {
    orchestrator: Orchestrator,
    stages: Vec<Stage>,
    initial_payload: Payload,
    _state: PhantomData<S>,
}

impl PipelineBuilder<NoStream> {
    pub fn new(orchestrator: Orchestrator, initial_payload: Payload) -> Self {
        Self { orchestrator, stages: Vec::new(), initial_payload, _state: PhantomData }
    }

    pub fn cpu_stage(mut self, stage: CpuStage) -> Self {
        self.stages.push(Stage::Cpu(stage));
        self
    }

    pub fn gpu(
        mut self,
        name: impl Into<String>,
        backend_id: impl Into<String>,
        op: BackendOp,
    ) -> Self {
        self.stages.push(Stage::Gpu(GpuStage {
            name: name.into(),
            backend_id: backend_id.into(),
            op,
        }));
        self
    }

    pub fn gpu_stream(
        mut self,
        name: impl Into<String>,
        backend_id: impl Into<String>,
        op: BackendOp,
    ) -> PipelineBuilder<HasStream> {
        self.stages.push(Stage::GpuStream(GpuStreamStage {
            name: name.into(),
            backend_id: backend_id.into(),
            op,
        }));
        PipelineBuilder {
            orchestrator: self.orchestrator,
            stages: self.stages,
            initial_payload: self.initial_payload,
            _state: PhantomData,
        }
    }

    pub async fn run(self) -> Result<TaskId, CoreError> {
        self.orchestrator.submit(self.stages, self.initial_payload).await
    }
}

impl PipelineBuilder<HasStream> {
    pub async fn run_stream(self) -> Result<TaskId, CoreError> {
        self.orchestrator.submit(self.stages, self.initial_payload).await
    }
}
