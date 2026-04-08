pub mod ggml;

pub use slab_runtime_core::CoreError as EngineError;

macro_rules! impl_ggml_from {
    ($($ty:path),+ $(,)?) => {
        $(
            impl From<$ty> for slab_runtime_core::CoreError {
                fn from(error: $ty) -> Self {
                    slab_runtime_core::CoreError::GGMLEngine(error.to_string())
                }
            }
        )+
    };
}

impl_ggml_from!(
    ggml::GGMLEngineError,
    ggml::whisper::GGMLWhisperEngineError,
    ggml::llama::GGMLLlamaEngineError,
    ggml::diffusion::GGMLDiffusionEngineError,
);
