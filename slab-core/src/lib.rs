pub mod base;
pub mod engine;
mod scheduler;

pub mod api;
pub mod ports;

pub use base::error::CoreError;
pub use base::types::{Payload, TaskId, TaskStatus};
pub use scheduler::storage::TaskStatusView;

/// Re-export all high-level capability types and traits so that callers can
/// write `use slab_core::capabilities::TextGenerationBackend` without knowing
/// the internal module path.
pub mod capabilities {
    pub use crate::ports::capabilities::{
        AudioTranscriptionBackend, AudioTranscriptionRequest, AudioTranscriptionResponse,
        ImageEmbeddingBackend, ImageEmbeddingRequest, ImageEmbeddingResponse,
        ImageGenerationBackend, ImageGenerationRequest, ImageGenerationResponse,
        TextGenerationBackend, TextGenerationRequest, TextGenerationResponse,
    };
}

/// Backward-compatible alias: `RuntimeError` is now [`CoreError`].
pub type RuntimeError = CoreError;
