#[cfg(feature = "ggml")]
pub mod ggml {
    pub use crate::internal::engine::audio_utils;
    pub use crate::internal::engine::ggml::{
        GGMLEngineError,
        diffusion::{DiffusionWorker, GGMLDiffusionEngine, GGMLDiffusionEngineError},
        llama::{GGMLLlamaEngine, GGMLLlamaEngineError, spawn_backend_with_engine},
        whisper::{GGMLWhisperEngine, GGMLWhisperEngineError, WhisperWorker},
    };
}

pub mod candle {
    pub use crate::internal::engine::candle::{
        CandleEngineError,
        diffusion::{CandleDiffusionEngineError, spawn_backend as spawn_diffusion_backend},
        llama::{CandleLlamaEngineError, spawn_backend_with_engine as spawn_llama_backend},
        whisper::{CandleWhisperEngineError, spawn_backend as spawn_whisper_backend},
    };
}

#[cfg(feature = "onnx")]
pub mod onnx {
    pub use crate::internal::engine::onnx::{OnnxEngineError, backend::OnnxWorker};
}
