//! ONNX Runtime backend for slab-core.
//!
//! This module provides a backend that loads and runs ONNX models via
//! [ONNX Runtime](https://onnxruntime.ai/) through the
//! [`ort`](https://docs.rs/ort) Rust bindings.
//!
//! ## Design
//!
//! Unlike the GGML backends, the ONNX backend does **not** use a separate
//! dynamic library loading step (`lib.load`).  ONNX Runtime is linked
//! through the `ort` crate at build time (or via `ORT_DYLIB_PATH` when the
//! `load-dynamic` feature is active).  The only lifecycle operations are:
//!
//! | Op             | Description                                         |
//! |----------------|-----------------------------------------------------|
//! | `model.load`   | Open an `.onnx` file and create an inference session |
//! | `model.unload` | Drop the session and free model memory              |
//! | `inference`    | Run a forward pass with named tensor inputs         |
//!
//! ## Execution Providers
//!
//! Callers may request hardware-accelerated execution providers in the
//! `model.load` payload (e.g. `"CUDA"`, `"TensorRT"`, `"CoreML"`).  The
//! ONNX Runtime will fall back to `"CPU"` automatically when a requested
//! provider is unavailable.
//!
//! ## Tensor Wire Format
//!
//! Input and output tensors are serialised as JSON objects containing:
//! - `"shape"` – `[i64]` dimension array (e.g. `[1, 3, 224, 224]`)
//! - `"dtype"` – one of `"float32"`, `"float64"`, `"int32"`, `"int64"`,
//!   `"uint8"`
//! - `"data_b64"` – base-64 encoded little-endian binary tensor data

pub mod adapter;
pub mod backend;
pub(crate) mod config;

use thiserror::Error;

/// All errors the ONNX engine can surface.
#[derive(Debug, Error)]
pub enum OnnxEngineError {
    /// The model session has not been loaded yet.
    #[error("ONNX model session not loaded; call model.load first")]
    SessionNotLoaded,

    /// The supplied `model_path` could not be opened as an ONNX session.
    #[error("Failed to create ONNX session from '{path}': {source}")]
    SessionCreate {
        path: String,
        #[source]
        source: ort::Error,
    },

    /// An inference run failed.
    #[error("ONNX inference failed: {source}")]
    InferenceFailed {
        #[source]
        source: ort::Error,
    },

    /// The wire-format tensor data (base-64 + dtype) could not be decoded.
    #[error("Failed to decode input tensor '{name}': {reason}")]
    TensorDecode { name: String, reason: String },

    /// An output tensor could not be converted to the wire format.
    #[error("Failed to encode output tensor '{name}': {reason}")]
    TensorEncode { name: String, reason: String },

    /// The JSON payload for `model.load` or `inference` was malformed.
    #[error("Invalid ONNX backend payload: {0}")]
    InvalidPayload(String),
}
