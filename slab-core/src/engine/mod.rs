pub mod ggml;
//todo
pub mod candle;

/// Engine-layer error type alias.
///
/// Adapter code can reference `engine::EngineError` without being aware of the
/// `base` layer.
pub use crate::base::error::CoreError as EngineError;

// ── From impls: GGML error types → CoreError ──────────────────────────────────
//
// Kept here (not in `base`) so that the base domain layer remains
// free of any dependency on engine-specific types.

macro_rules! impl_ggml_from {
    ($($ty:path),+ $(,)?) => {
        $(
            impl From<$ty> for crate::base::error::CoreError {
                fn from(e: $ty) -> Self {
                    crate::base::error::CoreError::GGMLEngine(e.to_string())
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
