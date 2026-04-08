pub mod onnx;

pub use slab_runtime_core::CoreError as EngineError;

impl From<onnx::OnnxEngineError> for slab_runtime_core::CoreError {
    fn from(error: onnx::OnnxEngineError) -> Self {
        slab_runtime_core::CoreError::OnnxEngine(error.to_string())
    }
}
