//! Deserialization types for the ONNX backend inference wire format.

use serde::Deserialize;
use std::collections::HashMap;

/// A single named input tensor in the wire format.
#[derive(Debug, Deserialize)]
pub(crate) struct TensorInput {
    pub shape: Vec<i64>,
    pub dtype: String,
    pub data_b64: String,
}

/// Input payload for the `inference` operation.
#[derive(Debug, Deserialize)]
pub(crate) struct OnnxInferenceInput {
    pub inputs: HashMap<String, TensorInput>,
}
