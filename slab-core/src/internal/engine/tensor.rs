//! Unified dense-tensor representation for the slab-core engine layer.
//!
//! `Tensor` is the common currency between engine backends: callers pass token
//! IDs in and get back logit distributions, regardless of whether the
//! underlying backend is GGML/llama.cpp or Candle.
//!
//! # Data variants
//!
//! | Variant | Element | Typical use |
//! |---------|---------|-------------|
//! | [`TensorData::U32`] | `u32` | Input token ID sequences |
//! | [`TensorData::F32`] | `f32` | Output logit distributions |
//!
//! Both variants store data as a flat `Vec` accompanied by a `shape` field
//! (row-major, innermost dimension last) so callers can interpret the data
//! correctly without extra metadata.

/// Flat data payload for a [`Tensor`].
#[derive(Debug, Clone)]
pub(crate) enum TensorData {
    /// Unsigned 32-bit integers, typically token IDs fed into the model.
    U32(Vec<u32>),
    /// Single-precision floats, typically logit distributions produced by the model.
    F32(Vec<f32>),
}

impl TensorData {
    fn len(&self) -> usize {
        match self {
            Self::U32(v) => v.len(),
            Self::F32(v) => v.len(),
        }
    }
}

/// A dense tensor whose element type is determined at runtime.
///
/// # Layout
///
/// `data` is stored in row-major (C-contiguous) order.  `shape` follows the
/// same convention as NumPy: `shape[0]` is the outermost dimension, and
/// `shape[shape.len()-1]` is the innermost.  The product of `shape` values
/// equals `data.len()`.
#[derive(Debug, Clone)]
pub(crate) struct Tensor {
    data: TensorData,
    shape: Vec<usize>,
}

impl Tensor {
    // ── Constructors ──────────────────────────────────────────────────────────

    /// Build a token-ID tensor from a slice; shape = `[ids.len()]`.
    pub(crate) fn from_token_ids(ids: &[u32]) -> Self {
        Self {
            data: TensorData::U32(ids.to_vec()),
            shape: vec![ids.len()],
        }
    }

    /// Build a logit tensor from an already-computed `Vec<f32>`; shape =
    /// `[data.len()]` (i.e., a 1-D vocabulary distribution).
    pub(crate) fn from_logits(data: Vec<f32>) -> Self {
        let n = data.len();
        Self {
            data: TensorData::F32(data),
            shape: vec![n],
        }
    }

    // ── Accessors ─────────────────────────────────────────────────────────────

    /// Return the token-ID slice if this tensor holds `U32` data, else `None`.
    pub(crate) fn as_token_ids(&self) -> Option<&[u32]> {
        match &self.data {
            TensorData::U32(v) => Some(v),
            _ => None,
        }
    }

    /// Return the logit slice if this tensor holds `F32` data, else `None`.
    pub(crate) fn as_logits(&self) -> Option<&[f32]> {
        match &self.data {
            TensorData::F32(v) => Some(v),
            _ => None,
        }
    }

    /// Logical shape of this tensor (row-major).
    pub(crate) fn shape(&self) -> &[usize] {
        &self.shape
    }

    /// Total number of elements.
    pub(crate) fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` when the tensor has no elements.
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ── Candle interop ────────────────────────────────────────────────────────────

#[cfg(feature = "candle")]
impl Tensor {
    /// Flatten a 1-D `candle_core::Tensor` of `f32` values into a logit
    /// [`Tensor`].
    ///
    /// The caller is responsible for ensuring `t` has been squeezed to a 1-D
    /// shape (e.g., `[vocab_size]`) before calling this method.
    pub(crate) fn from_candle_logits(
        t: &candle_core::Tensor,
    ) -> Result<Self, crate::base::error::CoreError> {
        let logits = t
            .to_vec1::<f32>()
            .map_err(|e| crate::base::error::CoreError::CandleEngine(e.to_string()))?;
        Ok(Self::from_logits(logits))
    }
}
