mod adapter;
mod backend;

pub use adapter::GGMLWhisperEngineError;
pub use backend::{spawn_backend, spawn_backend_with_path};
