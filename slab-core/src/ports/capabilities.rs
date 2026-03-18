//! High-level capability traits for AI inference.
//!
//! This module defines the capability-oriented abstraction layer that
//! decouples the higher application layers (e.g. `slab-server`) from the
//! concrete inference backends (GGML, Candle, ONNX).
//!
//! # Design
//!
//! Each capability is represented by:
//! - A typed `*Request` input struct carrying standard parameters.
//! - A typed `*Response` output struct.
//! - An `async_trait` that the concrete engine must implement.
//!
//! ## `backend_options` passthrough
//!
//! All request structs carry a `backend_options: HashMap<String, String>` field
//! that **must not** be interpreted by upper layers (e.g. `slab-server`).
//! It exists as a transparent passthrough for engine-specific load-time or
//! inference-time tuning knobs (e.g. GGML's `n_gpu_layers`, ONNX's
//! `execution_providers`).  Each concrete adapter documents which keys it
//! recognises.
//!
//! ## Advisory inference parameters
//!
//! Some request fields (such as `temperature`, `top_p`, `language`) represent
//! sampling or decoding hints.  Not every backend honours every field –
//! unsupported fields are silently ignored.  Consult the adapter's doc comment
//! for the precise set of honoured parameters.
//!
//! # Example
//!
//! ```rust,no_run
//! use slab_core::capabilities::{TextGenerationBackend, TextGenerationRequest};
//! use std::collections::HashMap;
//!
//! async fn generate_with_any_backend(
//!     backend: &dyn TextGenerationBackend,
//!     prompt: &str,
//! ) -> String {
//!     let req = TextGenerationRequest {
//!         prompt: prompt.to_owned(),
//!         max_tokens: Some(256),
//!         temperature: Some(0.7),
//!         top_p: None,
//!         session_key: None,
//!         backend_options: HashMap::new(),
//!     };
//!     backend.generate(req).await.unwrap().text
//! }
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::base::error::CoreError;
use crate::base::types::StreamHandle;

// ── Text Generation ────────────────────────────────────────────────────────────

/// Input for a text-generation (chat / completion) request.
///
/// The `prompt` field carries the full text to complete.  Callers that work
/// with multi-turn conversations should serialise the conversation history into
/// the prompt before submission (the backend does not manage history).
#[derive(Debug, Clone)]
pub struct TextGenerationRequest {
    /// The full prompt or serialised message history.
    pub prompt: String,
    /// Maximum number of tokens to generate.  Uses the backend default when `None`.
    pub max_tokens: Option<usize>,
    /// Sampling temperature (advisory).  `0.0` = greedy/deterministic, `1.0` = fully random.
    ///
    /// Not all backends honour this field; consult the concrete adapter's documentation.
    pub temperature: Option<f32>,
    /// Top-p nucleus sampling cutoff (advisory).  Backend default when `None`.
    ///
    /// Not all backends honour this field; consult the concrete adapter's documentation.
    pub top_p: Option<f32>,
    /// Opaque session key for KV-cache reuse (stateful backends only).
    ///
    /// Backends that do not support session continuity should ignore this field.
    pub session_key: Option<String>,
    /// Transparent passthrough map for engine-specific knobs.
    ///
    /// The server **must not** interpret this map.  Examples:
    /// - GGML: `{"n_gpu_layers": "33", "use_mmap": "true"}`
    /// - ONNX: `{"execution_providers": "CUDA,CPU"}`
    pub backend_options: HashMap<String, String>,
}

/// Output from a text-generation request.
#[derive(Debug, Clone)]
pub struct TextGenerationResponse {
    /// The generated text continuation.
    pub text: String,
    /// Number of tokens consumed (prompt + generated), if the backend reports it.
    pub tokens_used: Option<usize>,
}

/// Capability for text/chat generation.
///
/// Implement this trait on any struct that wraps a concrete text-generation
/// backend (GGML LLaMA, Candle LLaMA, ONNX text decoder, …).  The caller
/// always programs against this trait and is therefore completely unaware of
/// the underlying engine.
#[async_trait]
pub trait TextGenerationBackend: Send + Sync + 'static {
    /// Load a model from `model_path` into the backend.
    ///
    /// `options` may carry engine-specific load-time configuration such as
    /// quantisation level or the number of GPU layers to offload.  Backends
    /// must ignore unknown keys.
    async fn load_model(
        &mut self,
        model_path: &str,
        options: &HashMap<String, String>,
    ) -> Result<(), CoreError>;

    /// Run a blocking (non-streaming) generation pass.
    async fn generate(
        &self,
        request: TextGenerationRequest,
    ) -> Result<TextGenerationResponse, CoreError>;

    /// Run a streaming generation pass.
    ///
    /// The returned [`StreamHandle`] yields [`crate::base::types::StreamChunk`]
    /// items.  The stream terminates with `StreamChunk::Done` on success or
    /// `StreamChunk::Error` on failure.
    async fn generate_stream(
        &self,
        request: TextGenerationRequest,
    ) -> Result<StreamHandle, CoreError>;

    /// Unload the current model and release all associated resources.
    async fn unload(&mut self) -> Result<(), CoreError>;
}

// ── Audio Transcription ────────────────────────────────────────────────────────

/// Input for a speech-to-text transcription request.
#[derive(Debug, Clone)]
pub struct AudioTranscriptionRequest {
    /// Filesystem path (or URI) to the audio file to transcribe.
    pub path: String,
    /// BCP-47 language tag hint (e.g. `"en"`, `"zh"`) — advisory.
    ///
    /// Backends that support language detection may ignore this and auto-detect.
    /// Not all backends honour this field; consult the concrete adapter's documentation.
    pub language: Option<String>,
    /// Transparent passthrough map for engine-specific knobs.
    pub backend_options: HashMap<String, String>,
}

/// Output from a transcription request.
#[derive(Debug, Clone)]
pub struct AudioTranscriptionResponse {
    /// The full transcribed text.
    pub text: String,
    /// Language tag of the detected or forced language, if reported.
    pub language: Option<String>,
}

/// Capability for audio transcription (speech-to-text).
#[async_trait]
pub trait AudioTranscriptionBackend: Send + Sync + 'static {
    /// Load a transcription model from `model_path`.
    async fn load_model(
        &mut self,
        model_path: &str,
        options: &HashMap<String, String>,
    ) -> Result<(), CoreError>;

    /// Transcribe the audio file described by `request`.
    async fn transcribe(
        &self,
        request: AudioTranscriptionRequest,
    ) -> Result<AudioTranscriptionResponse, CoreError>;

    /// Unload the model and free all resources.
    async fn unload(&mut self) -> Result<(), CoreError>;
}

// ── Image Generation ───────────────────────────────────────────────────────────

/// Input for an image-generation request.
///
/// Supports both text-to-image and image-to-image workflows.  Set
/// `init_image` to `Some(bytes)` to enable img2img; otherwise the backend
/// runs a text-to-image pass.
#[derive(Debug, Clone)]
pub struct ImageGenerationRequest {
    /// Positive text prompt describing the desired image.
    pub prompt: String,
    /// Negative text prompt (things to suppress in the output).
    pub negative_prompt: Option<String>,
    /// Output image width in pixels.
    pub width: u32,
    /// Output image height in pixels.
    pub height: u32,
    /// Number of denoising/diffusion steps.
    pub steps: u32,
    /// Classifier-free guidance scale.
    pub guidance: f32,
    /// Optional RNG seed for deterministic generation.  Backend chooses a
    /// random seed when `None`.
    pub seed: Option<i64>,
    /// Optional initialisation image for img2img workflows (raw PNG/JPEG bytes).
    pub init_image: Option<Arc<[u8]>>,
    /// Transparent passthrough map for engine-specific knobs.
    pub backend_options: HashMap<String, String>,
}

/// Output from an image-generation request.
#[derive(Debug, Clone)]
pub struct ImageGenerationResponse {
    /// One or more generated images encoded as PNG/JPEG bytes.
    pub images: Vec<Arc<[u8]>>,
}

/// Capability for image generation (text-to-image / image-to-image).
#[async_trait]
pub trait ImageGenerationBackend: Send + Sync + 'static {
    /// Load a diffusion model from `model_path`.
    async fn load_model(
        &mut self,
        model_path: &str,
        options: &HashMap<String, String>,
    ) -> Result<(), CoreError>;

    /// Generate one or more images from the given request.
    async fn generate_image(
        &self,
        request: ImageGenerationRequest,
    ) -> Result<ImageGenerationResponse, CoreError>;

    /// Unload the model and free all resources.
    async fn unload(&mut self) -> Result<(), CoreError>;
}

// ── Image Embedding ────────────────────────────────────────────────────────────

/// Input for an image-embedding request.
#[derive(Debug, Clone)]
pub struct ImageEmbeddingRequest {
    /// Raw image bytes (PNG, JPEG, or any format the backend accepts).
    pub image: Arc<[u8]>,
    /// Transparent passthrough map for engine-specific knobs.
    pub backend_options: HashMap<String, String>,
}

/// Output from an image-embedding request.
#[derive(Debug, Clone)]
pub struct ImageEmbeddingResponse {
    /// The dense embedding vector produced by the backend.
    pub embedding: Vec<f32>,
}

/// Capability for image embedding (image → dense vector).
///
/// Common backends: ONNX CLIP, Candle CLIP, or any vision encoder.
#[async_trait]
pub trait ImageEmbeddingBackend: Send + Sync + 'static {
    /// Load an embedding model from `model_path`.
    async fn load_model(
        &mut self,
        model_path: &str,
        options: &HashMap<String, String>,
    ) -> Result<(), CoreError>;

    /// Compute a dense embedding vector for the supplied image.
    async fn embed_image(
        &self,
        request: ImageEmbeddingRequest,
    ) -> Result<ImageEmbeddingResponse, CoreError>;

    /// Unload the model and free all resources.
    async fn unload(&mut self) -> Result<(), CoreError>;
}
