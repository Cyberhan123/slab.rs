//! ONNX backend implementations of the slab-core capability traits.
//!
//! The ONNX backend is a general-purpose model runner that supports any model
//! exported in the Open Neural Network Exchange (`.onnx`) format.  Common uses:
//! - Image classification and embedding (e.g. CLIP, ResNet, ViT).
//! - Text classification and embedding (e.g. BERT-based encoders).
//! - Light-weight text generation (small seq2seq models).
//!
//! All implementations delegate to the global runtime via [`crate::api`].
//! [`crate::api::init`] **must** be called and `Config::onnx_enabled` must be
//! `true` before invoking any method.
//!
//! # Feature gate
//!
//! This module requires the `onnx` crate feature.

use std::collections::HashMap;

use async_trait::async_trait;

use crate::api::{self, Backend};
use crate::base::error::CoreError;
use crate::base::types::Payload;
use crate::ports::capabilities::{
    ImageEmbeddingBackend, ImageEmbeddingRequest, ImageEmbeddingResponse,
    TextGenerationBackend, TextGenerationRequest, TextGenerationResponse,
};

// ── Image Embedding ────────────────────────────────────────────────────────────

/// Image-embedding capability adapter backed by the `onnx` worker.
///
/// Suitable for vision encoder models (CLIP image tower, DINO, ViT, …).
/// The model must accept a `[B, C, H, W]` float32 tensor named `"input"` and
/// produce an `[B, D]` embedding tensor named `"output"` or `"pooler_output"`.
///
/// # Tensor wire format
///
/// The input image is encoded as a JSON object following the ONNX backend's
/// tensor wire format (see [`crate::engine::onnx`] module docs):
/// ```json
/// {
///   "inputs": {
///     "input": {
///       "shape": [1, 3, 224, 224],
///       "dtype": "float32",
///       "data_b64": "<base64>"
///     }
///   }
/// }
/// ```
///
/// # Example
///
/// ```rust,no_run
/// use slab_core::api;
/// use slab_core::capabilities::{ImageEmbeddingBackend, ImageEmbeddingRequest};
/// use slab_core::engine::onnx::capabilities::OnnxImageEmbeddingBackend;
/// use std::collections::HashMap;
/// use std::sync::Arc;
///
/// # tokio_test::block_on(async {
/// api::init(api::Config {
///     onnx_enabled: true,
///     ..Default::default()
/// }).unwrap();
///
/// let mut backend = OnnxImageEmbeddingBackend::new("input", "output");
/// backend.load_model("/models/clip_vision.onnx", &HashMap::new()).await.unwrap();
/// # });
/// ```
pub struct OnnxImageEmbeddingBackend {
    /// Name of the input tensor expected by the ONNX model (e.g. `"input"` or
    /// `"pixel_values"`).
    input_tensor_name: String,
    /// Name of the output embedding tensor (e.g. `"output"` or
    /// `"pooler_output"`).
    output_tensor_name: String,
    /// ONNX execution providers requested at load time.
    execution_providers: Vec<String>,
}

impl OnnxImageEmbeddingBackend {
    /// Create a new backend.
    ///
    /// # Arguments
    /// - `input_tensor_name` – name of the input tensor in the ONNX graph.
    /// - `output_tensor_name` – name of the output embedding tensor.
    pub fn new(input_tensor_name: impl Into<String>, output_tensor_name: impl Into<String>) -> Self {
        Self {
            input_tensor_name: input_tensor_name.into(),
            output_tensor_name: output_tensor_name.into(),
            execution_providers: vec!["CPU".into()],
        }
    }

    /// Set the preferred execution providers (e.g. `["CUDA", "CPU"]`).
    pub fn with_providers(mut self, providers: Vec<String>) -> Self {
        self.execution_providers = providers;
        self
    }
}

#[async_trait]
impl ImageEmbeddingBackend for OnnxImageEmbeddingBackend {
    /// Load an ONNX model into the `onnx` backend worker.
    ///
    /// Recognised `options` keys:
    /// - `"execution_providers"` – comma-separated list (default `"CPU"`).
    /// - `"intra_op_num_threads"` – thread count for intra-op parallelism.
    async fn load_model(
        &mut self,
        model_path: &str,
        options: &HashMap<String, String>,
    ) -> Result<(), CoreError> {
        // Allow per-call override of execution providers.
        if let Some(providers) = options.get("execution_providers") {
            self.execution_providers = providers.split(',').map(str::trim).map(String::from).collect();
        }

        let intra_threads: i64 = options
            .get("intra_op_num_threads")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);

        api::backend(Backend::Onnx)
            .load_model()
            .input(Payload::Json(serde_json::json!({
                "model_path": model_path,
                "execution_providers": self.execution_providers,
                "intra_op_num_threads": intra_threads,
            })))
            .run()
            .await
    }

    /// Compute an embedding vector for the supplied raw image bytes.
    ///
    /// The image is decoded, resized to 224×224, normalised to `[0, 1]` and
    /// encoded as a `[1, 3, 224, 224]` float32 tensor before being submitted
    /// to the ONNX backend.
    async fn embed_image(
        &self,
        request: ImageEmbeddingRequest,
    ) -> Result<ImageEmbeddingResponse, CoreError> {
        let tensor_json = encode_image_tensor(&request.image, &self.input_tensor_name)?;

        let result_bytes = api::backend(Backend::Onnx)
            .inference()
            .input(Payload::Json(tensor_json))
            .run_wait()
            .await?;

        let embedding = decode_embedding_tensor(&result_bytes, &self.output_tensor_name)?;

        Ok(ImageEmbeddingResponse { embedding })
    }

    async fn unload(&mut self) -> Result<(), CoreError> {
        api::backend(Backend::Onnx)
            .unload_model()
            .run()
            .await
    }
}

// ── Text Generation (ONNX) ─────────────────────────────────────────────────────

/// Text-generation capability adapter backed by the `onnx` worker.
///
/// Suitable for lightweight seq2seq or decoder-only models exported to ONNX
/// (e.g. GPT-2 ONNX, T5-small ONNX).  Input tokens are provided as a JSON
/// tensor following the ONNX backend's wire format; the raw output JSON is
/// returned as-is in [`TextGenerationResponse::text`].
///
/// For conversational LLMs prefer the GGML or Candle backends which support
/// KV-cache sessions.  This adapter is best used for classification,
/// summarisation, or other sequence-to-sequence tasks with short contexts.
///
/// # Prompt format
///
/// Because ONNX models operate on token IDs (not raw text), the `prompt`
/// field of [`TextGenerationRequest`] must be a JSON-encoded tensor object
/// following the ONNX backend's tensor wire format.  Apply a tokenizer to
/// convert text to token IDs and encode the resulting tensor before calling
/// [`TextGenerationBackend::generate`].
pub struct OnnxTextGenerationBackend;

impl OnnxTextGenerationBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OnnxTextGenerationBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TextGenerationBackend for OnnxTextGenerationBackend {
    async fn load_model(
        &mut self,
        model_path: &str,
        options: &HashMap<String, String>,
    ) -> Result<(), CoreError> {
        let providers: Vec<&str> = options
            .get("execution_providers")
            .map(|s| s.split(',').map(str::trim).collect())
            .unwrap_or_else(|| vec!["CPU"]);

        api::backend(Backend::Onnx)
            .load_model()
            .input(Payload::Json(serde_json::json!({
                "model_path": model_path,
                "execution_providers": providers,
            })))
            .run()
            .await
    }

    /// Submit a raw JSON tensor payload as the prompt.
    ///
    /// Because ONNX models operate on token IDs (not raw text), the `prompt`
    /// field of the request must be a JSON-encoded tensor object following the
    /// ONNX backend wire format.  Use a tokenizer to convert text → token IDs
    /// before calling this method.
    async fn generate(
        &self,
        request: TextGenerationRequest,
    ) -> Result<TextGenerationResponse, CoreError> {
        let input_json: serde_json::Value =
            serde_json::from_str(&request.prompt).map_err(|e| CoreError::GpuStageFailed {
                stage_name: "onnx.generate".into(),
                message: format!(
                    "prompt must be a JSON tensor object for the ONNX backend: {e}"
                ),
            })?;

        let result_bytes = api::backend(Backend::Onnx)
            .inference()
            .input(Payload::Json(input_json))
            .run_wait()
            .await?;

        Ok(TextGenerationResponse {
            text: String::from_utf8_lossy(&result_bytes).into_owned(),
            tokens_used: None,
        })
    }

    /// Streaming generation is not supported by the ONNX backend.
    ///
    /// Returns [`CoreError::UnsupportedOperation`].
    async fn generate_stream(
        &self,
        _request: TextGenerationRequest,
    ) -> Result<crate::base::types::StreamHandle, CoreError> {
        Err(CoreError::UnsupportedOperation {
            backend: "onnx".into(),
            op: "inference.stream".into(),
        })
    }

    async fn unload(&mut self) -> Result<(), CoreError> {
        api::backend(Backend::Onnx)
            .unload_model()
            .run()
            .await
    }
}

// ── Tensor encoding helpers ────────────────────────────────────────────────────

/// Decode raw image bytes (PNG/JPEG), resize to 224×224, and encode as a
/// `[1, 3, 224, 224]` float32 tensor in the ONNX backend wire format.
fn encode_image_tensor(
    image_bytes: &[u8],
    input_name: &str,
) -> Result<serde_json::Value, CoreError> {
    use base64::Engine as _;
    use image::{imageops::FilterType, DynamicImage, GenericImageView};

    let img = image::load_from_memory(image_bytes).map_err(|e| CoreError::GpuStageFailed {
        stage_name: "onnx.embed_image".into(),
        message: format!("image decode failed: {e}"),
    })?;

    // Resize to the standard 224×224 vision encoder input.
    let img: DynamicImage = img.resize_exact(224, 224, FilterType::Lanczos3);

    // Convert to RGB f32 in CHW layout, normalised to [0, 1].
    let mut data = Vec::with_capacity(3 * 224 * 224);
    for c in 0..3usize {
        for y in 0..224 {
            for x in 0..224 {
                let px = img.get_pixel(x as u32, y as u32);
                data.push(px.0[c] as f32 / 255.0);
            }
        }
    }

    let raw_bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();
    let data_b64 = base64::engine::general_purpose::STANDARD.encode(&raw_bytes);

    Ok(serde_json::json!({
        "inputs": {
            input_name: {
                "shape": [1i64, 3i64, 224i64, 224i64],
                "dtype": "float32",
                "data_b64": data_b64,
            }
        }
    }))
}

/// Decode the ONNX backend's JSON output and extract the named tensor as f32.
fn decode_embedding_tensor(bytes: &[u8], output_name: &str) -> Result<Vec<f32>, CoreError> {
    use base64::Engine as _;

    let json: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| CoreError::GpuStageFailed {
            stage_name: "onnx.embed_image".into(),
            message: format!("failed to parse ONNX output JSON: {e}"),
        })?;

    let tensor = json
        .get("outputs")
        .and_then(|o| o.get(output_name))
        .or_else(|| {
            // Fallback: accept a response where the output is the root object.
            json.get(output_name)
        })
        .ok_or_else(|| CoreError::GpuStageFailed {
            stage_name: "onnx.embed_image".into(),
            message: format!(
                "output tensor '{output_name}' not found in ONNX response; \
                 available keys: {}",
                json.as_object()
                    .map(|m| m.keys().cloned().collect::<Vec<_>>().join(", "))
                    .unwrap_or_default()
            ),
        })?;

    let data_b64 = tensor
        .get("data_b64")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CoreError::GpuStageFailed {
            stage_name: "onnx.embed_image".into(),
            message: "output tensor missing 'data_b64' field".into(),
        })?;

    let raw = base64::engine::general_purpose::STANDARD
        .decode(data_b64)
        .map_err(|e| CoreError::GpuStageFailed {
            stage_name: "onnx.embed_image".into(),
            message: format!("base64 decode failed: {e}"),
        })?;

    let embedding: Vec<f32> = raw
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();

    Ok(embedding)
}
