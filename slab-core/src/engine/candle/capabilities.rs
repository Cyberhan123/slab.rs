//! Candle backend implementations of the slab-core capability traits.
//!
//! Each struct adapts the Candle worker infrastructure to the high-level
//! capability interface, so that callers program against the trait without
//! knowing that the underlying engine is Candle.
//!
//! All implementations delegate to the global runtime via [`crate::api`].
//! [`crate::api::init`] **must** be called before invoking any method.

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

/// Text-generation capability adapter backed by the `candle.llama` worker.
///
/// The Candle LLaMA backend is statically linked (no shared library step).
/// It accepts GGUF model files identical to the GGML LLaMA backend.
///
/// # Example
///
/// ```rust,no_run
/// use slab_core::api;
/// use slab_core::capabilities::TextGenerationBackend;
/// use slab_core::engine::candle::capabilities::CandleTextGenerationBackend;
/// use std::collections::HashMap;
///
/// # tokio_test::block_on(async {
/// api::init(api::Config::default()).unwrap();
///
/// let mut backend = CandleTextGenerationBackend::new();
/// backend.load_model("/models/llama.gguf", &HashMap::new()).await.unwrap();
/// # });
/// ```
pub struct CandleTextGenerationBackend;

impl CandleTextGenerationBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CandleTextGenerationBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TextGenerationBackend for CandleTextGenerationBackend {
    /// Load a GGUF model into the `candle.llama` backend worker.
    ///
    /// Recognised `options` keys:
    /// - `"tokenizer_path"` – explicit tokenizer JSON path.
    /// - `"seed"` – RNG seed for sampling (default `0`).
    async fn load_model(
        &mut self,
        model_path: &str,
        options: &HashMap<String, String>,
    ) -> Result<(), CoreError> {
        let seed: u64 = options
            .get("seed")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        api::backend(Backend::CandleLlama)
            .load_model()
            .input(Payload::Json(serde_json::json!({
                "model_path": model_path,
                "tokenizer_path": options.get("tokenizer_path"),
                "seed": seed,
            })))
            .run()
            .await
    }

    async fn generate(
        &self,
        request: TextGenerationRequest,
    ) -> Result<TextGenerationResponse, CoreError> {
        let max_tokens = request.max_tokens.unwrap_or(256) as u64;

        let bytes = api::backend(Backend::CandleLlama)
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

    async fn generate_stream(
        &self,
        request: TextGenerationRequest,
    ) -> Result<StreamHandle, CoreError> {
        let max_tokens = request.max_tokens.unwrap_or(256) as u64;

        let mut engine_stream = api::backend(Backend::CandleLlama)
            .inference_stream()
            .input(Payload::Text(Arc::from(request.prompt.as_str())))
            .options(Payload::Json(serde_json::json!({
                "max_tokens": max_tokens,
                "session_key": request.session_key,
            })))
            .stream()
            .await?;

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
        api::backend(Backend::CandleLlama)
            .unload_model()
            .run()
            .await
    }
}

// ── Audio Transcription ────────────────────────────────────────────────────────

/// Transcription capability adapter backed by the `candle.whisper` worker.
pub struct CandleAudioTranscriptionBackend;

impl CandleAudioTranscriptionBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CandleAudioTranscriptionBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AudioTranscriptionBackend for CandleAudioTranscriptionBackend {
    /// Load a Whisper model into the `candle.whisper` backend worker.
    ///
    /// Recognised `options` keys:
    /// - `"tokenizer_path"` – explicit tokenizer JSON path.
    async fn load_model(
        &mut self,
        model_path: &str,
        options: &HashMap<String, String>,
    ) -> Result<(), CoreError> {
        api::backend(Backend::CandleWhisper)
            .load_model()
            .input(Payload::Json(serde_json::json!({
                "model_path": model_path,
                "tokenizer_path": options.get("tokenizer_path"),
            })))
            .run()
            .await
    }

    /// Transcribe the audio file at [`AudioTranscriptionRequest::path`].
    ///
    /// Audio is loaded as a WAV file and converted to 16 kHz mono f32 PCM
    /// before submission.  For other formats (MP3, FLAC, …) run an FFmpeg
    /// pre-process step at the call site.
    async fn transcribe(
        &self,
        request: AudioTranscriptionRequest,
    ) -> Result<AudioTranscriptionResponse, CoreError> {
        let pcm = crate::engine::audio_utils::load_pcm_from_wav(&request.path)?;

        let bytes = api::backend(Backend::CandleWhisper)
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
        api::backend(Backend::CandleWhisper)
            .unload_model()
            .run()
            .await
    }
}

// ── Image Generation ───────────────────────────────────────────────────────────

/// Image-generation capability adapter backed by the `candle.diffusion` worker.
pub struct CandleImageGenerationBackend;

impl CandleImageGenerationBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CandleImageGenerationBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ImageGenerationBackend for CandleImageGenerationBackend {
    /// Load a Stable Diffusion model into the `candle.diffusion` backend worker.
    ///
    /// Recognised `options` keys:
    /// - `"vae_path"` – optional path to a VAE weight file.
    /// - `"sd_version"` – `"v1-5"` or `"v2-1"` (default).
    async fn load_model(
        &mut self,
        model_path: &str,
        options: &HashMap<String, String>,
    ) -> Result<(), CoreError> {
        api::backend(Backend::CandleDiffusion)
            .load_model()
            .input(Payload::Json(serde_json::json!({
                "model_path": model_path,
                "vae_path": options.get("vae_path"),
                "sd_version": options.get("sd_version").cloned().unwrap_or_else(|| "v2-1".into()),
            })))
            .run()
            .await
    }

    async fn generate_image(
        &self,
        request: ImageGenerationRequest,
    ) -> Result<ImageGenerationResponse, CoreError> {
        let bytes = api::backend(Backend::CandleDiffusion)
            .inference()
            .input(Payload::Json(serde_json::json!({
                "prompt":          request.prompt,
                "negative_prompt": request.negative_prompt.unwrap_or_default(),
                "width":           request.width,
                "height":          request.height,
                "sample_steps":    request.steps,
                "cfg_scale":       request.guidance,
                "seed":            request.seed.unwrap_or(-1i64),
            })))
            .run_wait()
            .await?;

        Ok(ImageGenerationResponse {
            images: vec![Arc::from(bytes.as_ref())],
        })
    }

    async fn unload(&mut self) -> Result<(), CoreError> {
        api::backend(Backend::CandleDiffusion)
            .unload_model()
            .run()
            .await
    }
}
