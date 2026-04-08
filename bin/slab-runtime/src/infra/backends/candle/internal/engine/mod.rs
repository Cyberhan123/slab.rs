pub mod candle;

pub use slab_runtime_core::CoreError as EngineError;

macro_rules! impl_candle_from {
    ($($ty:path),+ $(,)?) => {
        $(
            impl From<$ty> for slab_runtime_core::CoreError {
                fn from(error: $ty) -> Self {
                    slab_runtime_core::CoreError::CandleEngine(error.to_string())
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
