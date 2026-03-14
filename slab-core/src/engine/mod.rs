pub mod ggml;
//todo
pub mod candle;

/// Engine-layer error type.
///
/// This is a re-export of [`crate::base::error::CoreError`] so that engine
/// adapter code can reference `engine::EngineError` without being aware of the
/// `base` layer.
pub use crate::base::error::CoreError as EngineError;
