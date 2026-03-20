//! ONNX engine adapter: wraps an [`ort::session::Session`] for use by [`OnnxWorker`].
//!
//! [`OnnxEngine`] is owned exclusively by its backend worker task, so no
//! locking is required for inference.  Model loading and unloading are
//! performed through `&mut self`, as is inference (the ONNX Runtime session
//! requires a mutable reference for `run`).

use std::collections::HashMap;

use base64::Engine as _;
use ort::{
    ep::ExecutionProviderDispatch,
    session::{builder::GraphOptimizationLevel, Session},
    value::{DynValue, Tensor, TensorElementType, ValueType},
};
use serde_json::{json, Value as JsonValue};
use tracing::{info, warn};

use super::{
    config::{OnnxInferenceInput, OnnxModelLoadConfig, TensorInput},
    OnnxEngineError,
};

// ── OnnxEngine ────────────────────────────────────────────────────────────────

/// Engine adapter wrapping an ONNX Runtime session.
///
/// Lifecycle:
/// - `None` → no model loaded
/// - `Some(session)` → model ready for inference
pub(crate) struct OnnxEngine {
    session: Option<Session>,
}

impl OnnxEngine {
    /// Create an engine with no model loaded.
    pub(crate) fn new() -> Self {
        Self { session: None }
    }

    // ── model lifecycle ───────────────────────────────────────────────────────

    /// Load the ONNX model at `config.model_path` and create a session.
    ///
    /// Any previously loaded session is replaced.
    pub(crate) fn load_model(
        &mut self,
        config: OnnxModelLoadConfig,
    ) -> Result<(), OnnxEngineError> {
        info!(
            model = %config.model_path,
            providers = ?config.execution_providers,
            "ONNX: loading model"
        );

        // Build execution provider list from the config.
        let mut ep_list: Vec<ExecutionProviderDispatch> = Vec::new();
        let mut has_cpu = false;
        for ep in &config.execution_providers {
            match ep.to_uppercase().as_str() {
                "CUDA" => ep_list.push(ort::ep::CUDA::default().build()),
                "TENSORRT" => ep_list.push(ort::ep::TensorRT::default().build()),
                "COREML" => ep_list.push(ort::ep::CoreML::default().build()),
                "DIRECTML" => ep_list.push(ort::ep::DirectML::default().build()),
                "CPU" => {
                    has_cpu = true;
                    ep_list.push(ort::ep::CPU::default().build());
                }
                other => {
                    warn!(provider = other, "ONNX: unrecognised execution provider; skipping");
                }
            }
        }
        // Always ensure CPU is available as final fallback.
        if !has_cpu {
            ep_list.push(ort::ep::CPU::default().build());
        }

        let mut builder = Session::builder()
            .map_err(|e| OnnxEngineError::SessionCreate {
                path: config.model_path.clone(),
                source: e,
            })?
            .with_optimization_level(GraphOptimizationLevel::All)
            .map_err(|e| OnnxEngineError::SessionCreate {
                path: config.model_path.clone(),
                source: e.into(),
            })?;

        if config.intra_op_num_threads > 0 {
            builder = builder.with_intra_threads(config.intra_op_num_threads).map_err(|e| {
                OnnxEngineError::SessionCreate { path: config.model_path.clone(), source: e.into() }
            })?;
        }

        if config.inter_op_num_threads > 0 {
            builder = builder.with_inter_threads(config.inter_op_num_threads).map_err(|e| {
                OnnxEngineError::SessionCreate { path: config.model_path.clone(), source: e.into() }
            })?;
        }

        builder = builder.with_execution_providers(ep_list).map_err(|e| {
            OnnxEngineError::SessionCreate { path: config.model_path.clone(), source: e.into() }
        })?;

        let session = builder.commit_from_file(&config.model_path).map_err(|e| {
            OnnxEngineError::SessionCreate { path: config.model_path.clone(), source: e }
        })?;

        info!(model = %config.model_path, "ONNX: model loaded");
        self.session = Some(session);
        Ok(())
    }

    /// Drop the current session and free model resources.
    pub(crate) fn unload(&mut self) {
        if self.session.take().is_some() {
            info!("ONNX: model unloaded");
        }
    }

    // ── inference ─────────────────────────────────────────────────────────────

    /// Run a synchronous inference pass.
    ///
    /// `input` is an [`OnnxInferenceInput`] containing named tensors.
    /// Returns a JSON object `{ "outputs": { name: { shape, dtype, data_b64 } } }`.
    pub(crate) fn inference(
        &mut self,
        input: OnnxInferenceInput,
    ) -> Result<JsonValue, OnnxEngineError> {
        let session = self.session.as_mut().ok_or(OnnxEngineError::SessionNotLoaded)?;

        // Build the ort inputs map (HashMap is accepted by the session's Into<SessionInputs>).
        let mut ort_inputs: HashMap<String, DynValue> = HashMap::new();
        for (name, ti) in input.inputs {
            let dyn_val = tensor_input_to_ort(&name, ti)?;
            ort_inputs.insert(name, dyn_val);
        }

        let outputs =
            session.run(ort_inputs).map_err(|e| OnnxEngineError::InferenceFailed { source: e })?;

        // Serialise outputs to JSON wire format.
        let mut result = serde_json::Map::new();
        for (name, value) in outputs.iter() {
            // ValueRef<'_> derefs to Value; pass as &DynValue.
            let wire = ort_value_to_json(name, &value)?;
            result.insert(name.to_string(), wire);
        }

        Ok(json!({ "outputs": result }))
    }
}

// ── Tensor helpers ────────────────────────────────────────────────────────────

/// Validate and convert a shape slice from `i64` to `usize`.
///
/// Returns an error if any dimension is negative or exceeds `usize::MAX`.
fn validate_shape(name: &str, shape: &[i64]) -> Result<Vec<usize>, OnnxEngineError> {
    shape
        .iter()
        .enumerate()
        .map(|(i, &d)| {
            usize::try_from(d).map_err(|_| OnnxEngineError::TensorDecode {
                name: name.to_string(),
                reason: format!(
                    "shape dimension [{}] is {} which is invalid (must be >= 0 and fit in usize)",
                    i, d
                ),
            })
        })
        .collect()
}

/// Decode a [`TensorInput`] into an `ort::DynValue`.
fn tensor_input_to_ort(name: &str, ti: TensorInput) -> Result<DynValue, OnnxEngineError> {
    let raw = base64::engine::general_purpose::STANDARD.decode(&ti.data_b64).map_err(|e| {
        OnnxEngineError::TensorDecode {
            name: name.to_string(),
            reason: format!("base64 decode error: {e}"),
        }
    })?;

    let shape = validate_shape(name, &ti.shape)?;

    macro_rules! make_tensor {
        ($ty:ty) => {{
            let elem_size = std::mem::size_of::<$ty>();
            if raw.len() % elem_size != 0 {
                return Err(OnnxEngineError::TensorDecode {
                    name: name.to_string(),
                    reason: format!("byte length {} is not a multiple of {}", raw.len(), elem_size),
                });
            }
            let data: Vec<$ty> = raw
                .chunks_exact(elem_size)
                .map(|b| <$ty>::from_le_bytes(b.try_into().unwrap()))
                .collect();
            Tensor::<$ty>::from_array((shape, data)).map(|t| t.into_dyn()).map_err(|e| {
                OnnxEngineError::TensorDecode { name: name.to_string(), reason: e.to_string() }
            })
        }};
    }

    match ti.dtype.as_str() {
        "float32" | "f32" => make_tensor!(f32),
        "float64" | "f64" => make_tensor!(f64),
        "int32" | "i32" => make_tensor!(i32),
        "int64" | "i64" => make_tensor!(i64),
        "uint8" | "u8" => {
            let expected = shape.iter().product::<usize>();
            if raw.len() != expected {
                return Err(OnnxEngineError::TensorDecode {
                    name: name.to_string(),
                    reason: format!(
                        "byte length {} does not match shape product {} for uint8",
                        raw.len(),
                        expected
                    ),
                });
            }
            Tensor::<u8>::from_array((shape, raw))
                .map(|t| t.into_dyn())
                .map_err(|e| OnnxEngineError::TensorDecode {
                    name: name.to_string(),
                    reason: e.to_string(),
                })
        }
        other => Err(OnnxEngineError::TensorDecode {
            name: name.to_string(),
            reason: format!("unsupported dtype '{other}'; supported dtypes: float32, float64, int32, int64, uint8"),
        }),
    }
}

/// Serialise an `ort` output value to the JSON wire format.
fn ort_value_to_json(name: &str, value: &DynValue) -> Result<JsonValue, OnnxEngineError> {
    let encode_err =
        |reason: String| OnnxEngineError::TensorEncode { name: name.to_string(), reason };

    // dtype() returns &ValueType directly (not Result).
    let value_type = value.dtype().clone();

    match value_type {
        ValueType::Tensor { ty, shape, .. } => {
            let shape_vec: Vec<i64> = shape.to_vec();
            let (dtype_str, data_b64) =
                encode_tensor_to_base64(name, value, ty).map_err(|e| encode_err(e.to_string()))?;

            Ok(json!({
                "shape": shape_vec,
                "dtype": dtype_str,
                "data_b64": data_b64,
            }))
        }
        other => Err(encode_err(format!("unsupported output value type: {other:?}"))),
    }
}

/// Encode a typed tensor's raw bytes to base64, returning `(dtype_str, base64)`.
fn encode_tensor_to_base64(
    name: &str,
    value: &DynValue,
    ty: TensorElementType,
) -> Result<(&'static str, String), OnnxEngineError> {
    let encode_err =
        |reason: String| OnnxEngineError::TensorEncode { name: name.to_string(), reason };

    macro_rules! extract_and_encode {
        ($rust_ty:ty, $dtype_str:expr) => {{
            let (_shape, data) =
                value.try_extract_tensor::<$rust_ty>().map_err(|e| encode_err(e.to_string()))?;
            let bytes: Vec<u8> = data.iter().flat_map(|v| v.to_le_bytes()).collect();
            ($dtype_str, base64::engine::general_purpose::STANDARD.encode(&bytes))
        }};
    }

    let result = match ty {
        TensorElementType::Float32 => extract_and_encode!(f32, "float32"),
        TensorElementType::Float64 => extract_and_encode!(f64, "float64"),
        TensorElementType::Int32 => extract_and_encode!(i32, "int32"),
        TensorElementType::Int64 => extract_and_encode!(i64, "int64"),
        TensorElementType::Uint8 => {
            let (_shape, data) =
                value.try_extract_tensor::<u8>().map_err(|e| encode_err(e.to_string()))?;
            let bytes: Vec<u8> = data.to_vec();
            ("uint8", base64::engine::general_purpose::STANDARD.encode(&bytes))
        }
        other => {
            return Err(encode_err(format!("unsupported output tensor element type: {other:?}")))
        }
    };

    Ok(result)
}
