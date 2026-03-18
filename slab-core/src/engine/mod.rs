#[cfg(feature = "ggml")]
pub mod ggml;
pub mod candle;
#[cfg(feature = "onnx")]
pub mod onnx;

/// Engine-layer error type alias.
///
/// Adapter code can reference `engine::EngineError` without being aware of the
/// `base` layer.
pub use crate::base::error::CoreError as EngineError;

// ── From impls: GGML error types → CoreError ──────────────────────────────────
//
// Kept here (not in `base`) so that the base domain layer remains
// free of any dependency on engine-specific types.

#[cfg(feature = "ggml")]
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

#[cfg(feature = "ggml")]
impl_ggml_from!(
    ggml::GGMLEngineError,
    ggml::whisper::GGMLWhisperEngineError,
    ggml::llama::GGMLLlamaEngineError,
    ggml::diffusion::GGMLDiffusionEngineError,
);

// ── From impls: ONNX error types → CoreError ─────────────────────────────────

#[cfg(feature = "onnx")]
impl From<onnx::OnnxEngineError> for crate::base::error::CoreError {
    fn from(e: onnx::OnnxEngineError) -> Self {
        crate::base::error::CoreError::OnnxEngine(e.to_string())
    }
}
// ── From impls: Candle error types → CoreError ────────────────────────────────

macro_rules! impl_candle_from {
    ($($ty:path),+ $(,)?) => {
        $(
            impl From<$ty> for crate::base::error::CoreError {
                fn from(e: $ty) -> Self {
                    crate::base::error::CoreError::CandleEngine(e.to_string())
                }
            }
        )+
    };
}

impl_candle_from!(
    candle::CandleEngineError,
    candle::llama::CandleLlamaEngineError,
    candle::whisper::CandleWhisperEngineError,
    candle::diffusion::CandleDiffusionEngineError,
);
