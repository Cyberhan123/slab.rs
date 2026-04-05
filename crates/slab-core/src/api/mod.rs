mod codec;

pub mod pipeline;
pub mod runtime;
pub mod task;

pub use crate::base::error::CoreError;
pub use crate::inference::{
    AudioTranscriptionRequest, AudioTranscriptionResponse, ImageEmbeddingRequest,
    ImageEmbeddingResponse, ImageGenerationRequest, ImageGenerationResponse, JsonOptions,
    TextGenerationChunk, TextGenerationRequest, TextGenerationResponse,
};
pub use crate::model::{Capability, DriverHints, ModelFamily, ModelSource, ModelSpec};
pub use codec::encode_model_pack_load_payload;
pub use pipeline::Pipeline;
pub use runtime::{DriversConfig, Runtime, RuntimeBuilder};
pub use task::{TaskHandle, TaskSnapshot, TaskState};

pub(crate) use codec::build_ggml_whisper_full_params_from_legacy;
