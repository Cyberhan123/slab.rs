mod api;
mod base;
mod dispatch;
mod engine;
mod ports;
pub mod model;
pub mod runtime;
mod scheduler;
mod spec;
pub mod task;

pub use base::error::CoreError;
pub use base::types::TaskId;
pub use model::{
    AutoModel, AutoModelForAudioTranscription, AutoModelForImageEmbedding,
    AutoModelForImageGeneration, AutoModelForTextGeneration, ModelDeployment,
};
pub use runtime::{BuiltinDriversConfig, Runtime, RuntimeBuilder};
pub use spec::*;
pub use task::{
    AudioTranscriptionPipeline, ImageEmbeddingPipeline, ImageGenerationPipeline, Pipeline,
    PipelineModelInput, TaskHandle, TaskSnapshot, TaskStageState, TaskState,
    TextGenerationPipeline,
};

pub type RuntimeError = CoreError;

pub fn pipeline(
    runtime: &Runtime,
    task_kind: TaskKind,
    model: impl Into<PipelineModelInput>,
) -> Result<Pipeline, CoreError> {
    task::pipeline(runtime, task_kind, model)
}
