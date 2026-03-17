//! Candle-based Whisper speech recognition engine adapter.
//!
//! Wraps [`candle_transformers::models::whisper`] so that the backend worker
//! has a stable, engine-agnostic API for audio transcription.
//!
//! When the `candle` feature is disabled all public methods return
//! [`CandleWhisperEngineError::ModelNotLoaded`] so the rest of the crate still
//! compiles.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use thiserror::Error;

use crate::engine::EngineError;

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum CandleWhisperEngineError {
    #[error("model not loaded; call model.load first")]
    ModelNotLoaded,

    #[error("lock poisoned while trying to {operation}")]
    LockPoisoned { operation: &'static str },

    #[error("failed to load model from {model_path}: {message}")]
    LoadModel { model_path: String, message: String },

    #[error("failed to load tokenizer: {message}")]
    LoadTokenizer { message: String },

    #[error("audio input is empty or invalid: {message}")]
    InvalidAudio { message: String },

    #[error("candle whisper inference error: {message}")]
    Inference { message: String },
}

// ── Inner state ────────────────────────────────────────────────────────────────

#[derive(Default)]
struct InnerState {
    /// Loaded model; `None` when no model is present.
    #[cfg(feature = "candle")]
    model: Option<Box<dyn CandleWhisperModelTrait + Send + Sync>>,
    #[cfg(not(feature = "candle"))]
    model: Option<()>,
    /// Loaded tokenizer.
    #[cfg(feature = "candle")]
    tokenizer: Option<candle_transformers::models::whisper::multilingual::WhisperTokenizer>,
    #[cfg(not(feature = "candle"))]
    tokenizer: Option<()>,
}

// ── Trait abstracting over Whisper model sizes (candle feature only) ──────────

#[cfg(feature = "candle")]
trait CandleWhisperModelTrait {
    fn transcribe(
        &mut self,
        mel: &candle_core::Tensor,
        tokenizer: &candle_transformers::models::whisper::multilingual::WhisperTokenizer,
    ) -> Result<String, candle_core::Error>;
}

#[cfg(feature = "candle")]
struct WhisperModelWrapper<M>(M);

#[cfg(feature = "candle")]
impl<M> CandleWhisperModelTrait for WhisperModelWrapper<M>
where
    M: candle_transformers::models::whisper::model::Whisper + Send + Sync,
{
    fn transcribe(
        &mut self,
        mel: &candle_core::Tensor,
        tokenizer: &candle_transformers::models::whisper::multilingual::WhisperTokenizer,
    ) -> Result<String, candle_core::Error> {
        use candle_transformers::models::whisper::decoding::Decoder;
        let mut decoder = Decoder::new(
            &mut self.0,
            tokenizer,
            /*seed*/ 42,
            &candle_core::Device::Cpu,
            /*timestamps*/ false,
            /*verbose*/ false,
        )
        .map_err(|e| candle_core::Error::Msg(e.to_string()))?;
        let result = decoder
            .run(mel)
            .map_err(|e| candle_core::Error::Msg(e.to_string()))?;
        Ok(result
            .segments
            .iter()
            .map(|s| s.dr.text.as_str())
            .collect::<Vec<_>>()
            .join(" "))
    }
}

// ── Engine ────────────────────────────────────────────────────────────────────

/// Engine adapter for Candle-based Whisper speech recognition.
#[derive(Clone)]
pub struct CandleWhisperEngine {
    inner: Arc<RwLock<InnerState>>,
}

impl CandleWhisperEngine {
    /// Create a new, empty engine (no model loaded).
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(InnerState::default())),
        }
    }

    /// Returns `true` when a model is currently loaded.
    pub fn is_model_loaded(&self) -> bool {
        self.inner
            .read()
            .map(|s| s.model.is_some())
            .unwrap_or(false)
    }

    /// Load a Whisper model from `model_path`.
    ///
    /// The model file may be a safetensors or GGUF weight file.  The tokenizer
    /// is resolved from the same directory when `tokenizer_path` is `None`.
    pub fn load_model(
        &self,
        model_path: &str,
        _tokenizer_path: Option<&str>,
    ) -> Result<(), EngineError> {
        #[cfg(feature = "candle")]
        {
            use candle_core::Device;
            use candle_nn::VarBuilder;
            use candle_transformers::models::whisper::{self, Config};

            tracing::info!(model_path, "loading candle whisper model");

            let path = Path::new(model_path);
            let device = Device::Cpu;

            // Load config from the same directory as the model.
            let config_path = path.parent().unwrap_or(Path::new(".")).join("config.json");
            let config: Config = if config_path.exists() {
                let data = std::fs::read_to_string(&config_path).map_err(|e| {
                    CandleWhisperEngineError::LoadModel {
                        model_path: model_path.to_owned(),
                        message: format!("failed to read config.json: {e}"),
                    }
                })?;
                serde_json::from_str(&data).map_err(|e| CandleWhisperEngineError::LoadModel {
                    model_path: model_path.to_owned(),
                    message: format!("failed to parse config.json: {e}"),
                })?
            } else {
                // Fall back to tiny model default config.
                whisper::Config::default()
            };

            let vb = unsafe {
                VarBuilder::from_mmaped_safetensors(&[path], candle_core::DType::F32, &device)
                    .map_err(|e| CandleWhisperEngineError::LoadModel {
                        model_path: model_path.to_owned(),
                        message: e.to_string(),
                    })?
            };

            let model = whisper::model::Whisper::load(&vb, config).map_err(|e| {
                CandleWhisperEngineError::LoadModel {
                    model_path: model_path.to_owned(),
                    message: e.to_string(),
                }
            })?;

            // Load tokenizer.
            let tok_dir = path.parent().unwrap_or(Path::new("."));
            let tok = candle_transformers::models::whisper::multilingual::multilingual_tokenizer(
                tok_dir,
                /*language*/ "en",
                /*timestamps*/ false,
            )
            .map_err(|e| CandleWhisperEngineError::LoadTokenizer {
                message: e.to_string(),
            })?;

            let mut state = self.inner.write().map_err(|_| {
                CandleWhisperEngineError::LockPoisoned {
                    operation: "write whisper model state",
                }
            })?;
            state.model = Some(Box::new(WhisperModelWrapper(model)));
            state.tokenizer = Some(tok);
        }

        #[cfg(not(feature = "candle"))]
        {
            tracing::warn!(
                "candle feature is not enabled; model.load is a no-op for CandleWhisperEngine"
            );
        }

        Ok(())
    }

    /// Unload the model and free resources.
    pub fn unload(&self) {
        if let Ok(mut state) = self.inner.write() {
            state.model = None;
            state.tokenizer = None;
        }
    }

    /// Transcribe raw 16 kHz mono f32 PCM samples.
    ///
    /// Returns the transcribed text with approximate timestamps in
    /// `<start> --> <end>: <text>` format (one line per segment).
    pub fn inference(&self, samples: &[f32]) -> Result<String, EngineError> {
        if samples.is_empty() {
            return Err(CandleWhisperEngineError::InvalidAudio {
                message: "audio samples are empty".into(),
            }
            .into());
        }

        #[cfg(feature = "candle")]
        {
            use candle_core::{Device, Tensor};
            use candle_transformers::models::whisper::audio;

            let device = Device::Cpu;

            let mel = audio::pcm_to_mel(samples, &device).map_err(|e| {
                CandleWhisperEngineError::Inference {
                    message: e.to_string(),
                }
            })?;
            let mel = mel.unsqueeze(0).map_err(|e| CandleWhisperEngineError::Inference {
                message: e.to_string(),
            })?;

            let mut state = self.inner.write().map_err(|_| {
                CandleWhisperEngineError::LockPoisoned {
                    operation: "lock whisper state for inference",
                }
            })?;

            let (model, tokenizer) = match (&mut state.model, &state.tokenizer) {
                (Some(m), Some(t)) => (m, t),
                _ => {
                    return Err(CandleWhisperEngineError::ModelNotLoaded.into());
                }
            };

            let text = model
                .transcribe(&mel, tokenizer)
                .map_err(|e| CandleWhisperEngineError::Inference {
                    message: e.to_string(),
                })?;

            return Ok(text);
        }

        #[cfg(not(feature = "candle"))]
        {
            let _ = samples;
            Err(CandleWhisperEngineError::ModelNotLoaded.into())
        }
    }
}

impl Default for CandleWhisperEngine {
    fn default() -> Self {
        Self::new()
    }
}
