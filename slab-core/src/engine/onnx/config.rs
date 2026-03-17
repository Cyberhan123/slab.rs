//! Deserialisation types for the ONNX backend.
//!
//! These structs map directly to the JSON payloads that callers send for
//! `model.load` and `inference` operations on the `onnx` backend.

use serde::Deserialize;
use std::collections::HashMap;

// ── model.load ────────────────────────────────────────────────────────────────

/// Input payload for the `model.load` operation.
///
/// ### Example
/// ```json
/// {
///   "model_path": "/models/resnet50.onnx",
///   "execution_providers": ["CUDA", "CPU"],
///   "intra_op_num_threads": 4,
///   "inter_op_num_threads": 1
/// }
/// ```
#[derive(Debug, Deserialize)]
pub(crate) struct OnnxModelLoadConfig {
    /// Path to the `.onnx` model file.
    pub model_path: String,

    /// Ordered list of ONNX Runtime execution providers to try.
    ///
    /// Valid values include `"CUDA"`, `"TensorRT"`, `"CoreML"`,
    /// `"DirectML"`, `"ROCm"`, `"CPU"`.  ONNX Runtime silently skips
    /// providers that are not available in the current build and falls
    /// back to the next one in the list.  `"CPU"` is always appended as
    /// the final fallback if not already present.
    #[serde(default = "default_execution_providers")]
    pub execution_providers: Vec<String>,

    /// Number of threads used for intra-operator parallelism.
    /// `0` means "use the default" (typically equal to the physical CPU
    /// core count).
    #[serde(default)]
    pub intra_op_num_threads: usize,

    /// Number of threads used for inter-operator parallelism.
    /// `0` means "use the default".
    #[serde(default)]
    pub inter_op_num_threads: usize,
}

fn default_execution_providers() -> Vec<String> {
    vec!["CPU".to_string()]
}

// ── inference ─────────────────────────────────────────────────────────────────

/// A single named input tensor in the wire format.
///
/// ### Example
/// ```json
/// {
///   "shape": [1, 3, 224, 224],
///   "dtype": "float32",
///   "data_b64": "<base64-encoded little-endian bytes>"
/// }
/// ```
#[derive(Debug, Deserialize)]
pub(crate) struct TensorInput {
    /// Tensor shape dimensions.
    pub shape: Vec<i64>,
    /// Element type: `"float32"`, `"float64"`, `"int32"`, `"int64"`,
    /// `"uint8"`, or `"bool"`.
    pub dtype: String,
    /// Base-64 encoded little-endian binary tensor data.
    pub data_b64: String,
}

/// Input payload for the `inference` operation.
///
/// ### Example
/// ```json
/// {
///   "inputs": {
///     "pixel_values": {
///       "shape": [1, 3, 224, 224],
///       "dtype": "float32",
///       "data_b64": "..."
///     }
///   }
/// }
/// ```
#[derive(Debug, Deserialize)]
pub(crate) struct OnnxInferenceInput {
    /// Map from input tensor name to tensor data.
    pub inputs: HashMap<String, TensorInput>,
}
