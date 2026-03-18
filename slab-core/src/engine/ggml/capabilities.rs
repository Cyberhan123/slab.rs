//! GGML backend implementations of the slab-core capability traits.
//!
//! Each struct adapts the GGML worker infrastructure to the high-level
//! capability interface, so that callers program against the trait without
//! knowing that the underlying engine is GGML.
//!
//! All implementations delegate to the global runtime via [`crate::api`].
//! [`crate::api::init`] **must** be called before invoking any method.
//!
//! # Feature gate
//!
//! This module requires the `ggml` crate feature.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::api::{self, Backend};
use crate::base::error::CoreError;
use crate::base::types::{Payload, StreamChunk, StreamHandle};
use crate::ports::capabilities::{
    AudioTranscriptionBackend, AudioTranscriptionRequest, AudioTranscriptionResponse,
    ImageGenerationBackend, ImageGenerationRequest, ImageGenerationResponse,
    TextGenerationBackend, TextGenerationRequest, TextGenerationResponse,
};

// ── Text Generation ────────────────────────────────────────────────────────────

/// Text-generation capability adapter backed by the `ggml.llama` worker.
///
/// Translates [`TextGenerationRequest`] parameters into the JSON payload
/// format expected by the GGML LLaMA backend worker, and converts the
/// returned bytes back into a [`TextGenerationResponse`].
///
/// # Example
///
/// ```rust,no_run
/// use slab_core::api;
/// use slab_core::capabilities::TextGenerationBackend;
/// use slab_core::engine::ggml::capabilities::GgmlTextGenerationBackend;
/// use std::collections::HashMap;
///
/// # tokio_test::block_on(async {
/// // 1. Initialize the global runtime once.
/// api::init(api::Config {
///     llama_lib_dir: Some("/usr/local/lib".into()),
///     ..Default::default()
/// }).unwrap();
///
/// // 2. Load a model.
/// let mut backend = GgmlTextGenerationBackend::new();
/// backend.load_model("/models/qwen.gguf", &HashMap::new()).await.unwrap();
///
/// // 3. Generate text using only the capability trait.
/// use slab_core::capabilities::TextGenerationRequest;
/// let resp = backend.generate(TextGenerationRequest {
///     prompt: "Hello, world!".into(),
///     max_tokens: Some(128),
///     temperature: None,
///     top_p: None,
///     session_key: None,
///     backend_options: HashMap::new(),
/// }).await.unwrap();
/// println!("{}", resp.text);
/// # });
/// ```
pub struct GgmlTextGenerationBackend;

impl GgmlTextGenerationBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GgmlTextGenerationBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TextGenerationBackend for GgmlTextGenerationBackend {
    /// Load a GGUF model into the `ggml.llama` backend worker.
    ///
    /// Recognised `options` keys:
    /// - `"num_workers"` – number of inference threads (default `1`).
    /// - `"context_length"` – KV-cache context window in tokens (default `4096`).
    async fn load_model(
        &mut self,
        model_path: &str,
        options: &HashMap<String, String>,
    ) -> Result<(), CoreError> {
        let num_workers: u64 = options
            .get("num_workers")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        let context_length: u64 = options
            .get("context_length")
            .and_then(|v| v.parse().ok())
            .unwrap_or(4096);

        api::backend(Backend::GGMLLlama)
            .load_model()
            .input(Payload::Json(serde_json::json!({
                "model_path": model_path,
                "num_workers": num_workers,
                "context_length": context_length,
            })))
            .run()
            .await
    }

    /// Generate text from `request.prompt`.
    ///
    /// The `session_key` in the request is forwarded as an op option so that
    /// the GGML LLaMA worker can maintain a KV-cache across calls.
    async fn generate(
        &self,
        request: TextGenerationRequest,
    ) -> Result<TextGenerationResponse, CoreError> {
        let max_tokens = request.max_tokens.unwrap_or(256) as u64;

        let bytes = api::backend(Backend::GGMLLlama)
            .inference()
            .input(Payload::Text(Arc::from(request.prompt.as_str())))
            .options(Payload::Json(serde_json::json!({
                "max_tokens": max_tokens,
                "session_key": request.session_key,
            })))
            .run_wait()
            .await?;

        Ok(TextGenerationResponse {
            text: String::from_utf8_lossy(&bytes).into_owned(),
            tokens_used: None,
        })
    }

    /// Stream generated text tokens from the `ggml.llama` backend.
    async fn generate_stream(
        &self,
        request: TextGenerationRequest,
    ) -> Result<StreamHandle, CoreError> {
        let max_tokens = request.max_tokens.unwrap_or(256) as u64;

        let mut engine_stream = api::backend(Backend::GGMLLlama)
            .inference_stream()
            .input(Payload::Text(Arc::from(request.prompt.as_str())))
            .options(Payload::Json(serde_json::json!({
                "max_tokens": max_tokens,
                "session_key": request.session_key,
            })))
            .stream()
            .await?;

        // Wrap the `bytes::Bytes` stream in the standard `StreamHandle`.
        let (tx, rx) = tokio::sync::mpsc::channel::<StreamChunk>(128);
        tokio::spawn(async move {
            use futures::StreamExt;
            // Pin the stream so it can be polled inside an async block.
            tokio::pin!(engine_stream);
            while let Some(item) = engine_stream.next().await {
                let chunk = match item {
                    Ok(bytes) => StreamChunk::Token(String::from_utf8_lossy(&bytes).into_owned()),
                    Err(e) => StreamChunk::Error(e.to_string()),
                };
                let is_error = matches!(chunk, StreamChunk::Error(_));
                let _ = tx.send(chunk).await;
                // After an error chunk, stop without emitting Done.
                if is_error {
                    return;
                }
            }
            // Clean completion: signal Done.
            let _ = tx.send(StreamChunk::Done).await;
        });

        Ok(rx)
    }

    async fn unload(&mut self) -> Result<(), CoreError> {
        api::backend(Backend::GGMLLlama)
            .unload_model()
            .run()
            .await
    }
}

// ── Audio Transcription ────────────────────────────────────────────────────────

/// Transcription capability adapter backed by the `ggml.whisper` worker.
///
/// Accepts a file path via [`AudioTranscriptionRequest::path`].  The adapter
/// reads the file, parses it as an uncompressed PCM WAV, and submits the
/// resulting f32 samples to the `ggml.whisper` backend.  For compressed
/// formats (MP3, FLAC, …) apply an FFmpeg pre-process stage before calling
/// this adapter.
pub struct GgmlAudioTranscriptionBackend;

impl GgmlAudioTranscriptionBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GgmlAudioTranscriptionBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AudioTranscriptionBackend for GgmlAudioTranscriptionBackend {
    /// Load a Whisper model into the `ggml.whisper` backend worker.
    async fn load_model(
        &mut self,
        model_path: &str,
        _options: &HashMap<String, String>,
    ) -> Result<(), CoreError> {
        api::backend(Backend::GGMLWhisper)
            .load_model()
            .input(Payload::Json(serde_json::json!({
                "model_path": model_path,
            })))
            .run()
            .await
    }

    /// Transcribe the audio file at [`AudioTranscriptionRequest::path`].
    ///
    /// The file must be an uncompressed PCM WAV.  It is loaded, decoded to
    /// f32 samples, and submitted to the `ggml.whisper` backend.  For
    /// compressed formats (MP3, FLAC, …) run an FFmpeg pre-process step at
    /// the call site before invoking this method.
    async fn transcribe(
        &self,
        request: AudioTranscriptionRequest,
    ) -> Result<AudioTranscriptionResponse, CoreError> {
        // Load and convert the audio file to 16 kHz mono f32 PCM.
        let pcm = load_audio_as_pcm(&request.path)?;

        let bytes = api::backend(Backend::GGMLWhisper)
            .inference()
            .input(Payload::F32(Arc::from(pcm)))
            .run_wait()
            .await?;

        Ok(AudioTranscriptionResponse {
            text: String::from_utf8_lossy(&bytes).into_owned(),
            language: None,
        })
    }

    async fn unload(&mut self) -> Result<(), CoreError> {
        api::backend(Backend::GGMLWhisper)
            .unload_model()
            .run()
            .await
    }
}

/// Load a WAV file from `path` and return f32 PCM samples.
///
/// Re-exported from [`crate::engine::audio_utils`] for use by other
/// capability adapter modules that cannot take a direct feature-gated
/// dependency on this module.
pub use crate::engine::audio_utils::load_pcm_from_wav;

/// Load and convert the audio file at `path` to f32 PCM samples.
fn load_audio_as_pcm(path: &str) -> Result<Vec<f32>, CoreError> {
    crate::engine::audio_utils::load_pcm_from_wav(path)
}

// ── Image Generation ───────────────────────────────────────────────────────────

/// Image-generation capability adapter backed by the `ggml.diffusion` worker.
///
/// Translates [`ImageGenerationRequest`] parameters into the JSON payload
/// expected by the GGML Stable Diffusion backend.
pub struct GgmlImageGenerationBackend;

impl GgmlImageGenerationBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GgmlImageGenerationBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ImageGenerationBackend for GgmlImageGenerationBackend {
    /// Load a Stable Diffusion model into the `ggml.diffusion` backend worker.
    ///
    /// Recognised `options` keys (all optional):
    /// - `"vae_path"` – path to a separate VAE weight file.
    /// - `"taesd_path"` – path to a TAESD VAE (faster decoding).
    /// - `"flash_attn"` – `"true"` to enable flash attention.
    async fn load_model(
        &mut self,
        model_path: &str,
        options: &HashMap<String, String>,
    ) -> Result<(), CoreError> {
        api::backend(Backend::GGMLDiffusion)
            .load_model()
            .input(Payload::Json(serde_json::json!({
                "model_path": model_path,
                "vae_path": options.get("vae_path").cloned().unwrap_or_default(),
                "taesd_path": options.get("taesd_path").cloned().unwrap_or_default(),
                "flash_attn": options.get("flash_attn")
                    .map(|v| v == "true")
                    .unwrap_or(false),
            })))
            .run()
            .await
    }

    /// Generate an image from the request parameters.
    ///
    /// The backend returns raw PNG bytes which are wrapped in
    /// [`ImageGenerationResponse::images`].
    async fn generate_image(
        &self,
        request: ImageGenerationRequest,
    ) -> Result<ImageGenerationResponse, CoreError> {
        let init_image_b64: Option<String> = request.init_image.as_deref().map(|bytes| {
            use base64::Engine as _;
            base64::engine::general_purpose::STANDARD.encode(bytes)
        });

        let bytes = api::backend(Backend::GGMLDiffusion)
            .inference()
            .input(Payload::Json(serde_json::json!({
                "prompt":           request.prompt,
                "negative_prompt":  request.negative_prompt.unwrap_or_default(),
                "width":            request.width,
                "height":           request.height,
                "sample_steps":     request.steps,
                "guidance":         request.guidance,
                "seed":             request.seed.unwrap_or(-1i64),
                "init_image_b64":   init_image_b64,
            })))
            .run_wait()
            .await?;

        Ok(ImageGenerationResponse {
            images: vec![Arc::from(bytes.as_ref())],
        })
    }

    async fn unload(&mut self) -> Result<(), CoreError> {
        api::backend(Backend::GGMLDiffusion)
            .unload_model()
            .run()
            .await
    }
}
