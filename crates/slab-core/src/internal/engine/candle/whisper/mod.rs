mod adapter;
mod backend;

pub use adapter::CandleWhisperEngineError;
pub(crate) use backend::spawn_backend;
