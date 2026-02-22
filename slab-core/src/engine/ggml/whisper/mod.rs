mod adapter;
mod backend;

pub use adapter::{GGMLWhisperEngine, GGMLWhisperEngineError};
pub use backend::spawn_backend;