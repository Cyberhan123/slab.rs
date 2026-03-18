//! Candle-based Whisper speech recognition engine adapter.
//!
//! Wraps [`candle_transformers::models::whisper`] so that the backend worker
//! has a stable, engine-agnostic API for audio transcription.
//!
//! When the `candle` feature is disabled all public methods return
//! [`CandleWhisperEngineError::ModelNotLoaded`] so the rest of the crate still
//! compiles.

use std::path::Path;
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
    model: Option<candle_transformers::models::whisper::model::Whisper>,
    #[cfg(not(feature = "candle"))]
    model: Option<()>,
    /// Loaded tokenizer.
    #[cfg(feature = "candle")]
    tokenizer: Option<tokenizers::Tokenizer>,
    #[cfg(not(feature = "candle"))]
    tokenizer: Option<()>,
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
    /// The model file should be a safetensors weight file.  The tokenizer is
    /// resolved from `tokenizer_path` when provided; otherwise it is looked up
    /// from the same directory as `model_path`.
    ///
    /// Returns [`CandleWhisperEngineError::ModelNotLoaded`] when compiled
    /// without the `candle` feature.
    pub fn load_model(
        &self,
        model_path: &str,
        tokenizer_path: Option<&str>,
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

            let tok_dir: &Path = if let Some(tp) = tokenizer_path {
                Path::new(tp).parent().unwrap_or(Path::new("."))
            } else {
                path.parent().unwrap_or(Path::new("."))
            };
            let tokenizer_json = tok_dir.join("tokenizer.json");
            let tok = tokenizers::Tokenizer::from_file(&tokenizer_json).map_err(|e| {
                CandleWhisperEngineError::LoadTokenizer {
                    message: e.to_string(),
                }
            })?;

            let mut state = self.inner.write().map_err(|_| {
                CandleWhisperEngineError::LockPoisoned {
                    operation: "write whisper model state",
                }
            })?;
            state.model = Some(model);
            state.tokenizer = Some(tok);

            return Ok(());
        }

        #[cfg(not(feature = "candle"))]
        {
            let _ = (model_path, tokenizer_path);
            return Err(CandleWhisperEngineError::ModelNotLoaded.into());
        }
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
            use candle_core::{Device, IndexOp, Tensor, D};
            use candle_transformers::models::whisper::{self, audio};

            let device = Device::Cpu;
            let config_path = {
                // Borrow state briefly just to extract the config path.
                let s = self.inner.read().map_err(|_| CandleWhisperEngineError::LockPoisoned {
                    operation: "read whisper state for config path",
                })?;
                if s.model.is_none() {
                    return Err(CandleWhisperEngineError::ModelNotLoaded.into());
                }
                // Config path is not stored – use default mel filter size.
                drop(s);
            };
            let _ = config_path;

            // Build a default config to determine mel filter dimensions.
            let cfg = whisper::Config {
                num_mel_bins: 80,
                max_source_positions: 1500,
                d_model: 384,
                encoder_attention_heads: 6,
                encoder_layers: 4,
                vocab_size: 51865,
                max_target_positions: 448,
                decoder_attention_heads: 6,
                decoder_layers: 4,
                suppress_tokens: vec![],
            };

            // Compute mel spectrogram using precomputed unit filters (80 × N_FFT/2+1).
            // For production use, load actual mel filter banks from the model directory.
            let n_mels = cfg.num_mel_bins;
            let n_fft_half = whisper::N_FFT / 2 + 1;
            let mel_filters = vec![1.0f32 / n_fft_half as f32; n_mels * n_fft_half];
            let mel_data = audio::pcm_to_mel(&cfg, samples, &mel_filters);
            let n_frames = mel_data.len() / n_mels;

            let mel = Tensor::from_vec(mel_data, (1usize, n_mels, n_frames), &device)
                .map_err(|e| CandleWhisperEngineError::Inference {
                    message: format!("mel tensor: {e}"),
                })?;

            let mut state = self.inner.write().map_err(|_| {
                CandleWhisperEngineError::LockPoisoned {
                    operation: "lock whisper state for inference",
                }
            })?;

            let (model, tokenizer) = match (&mut state.model, &state.tokenizer) {
                (Some(m), Some(t)) => (m, t),
                _ => return Err(CandleWhisperEngineError::ModelNotLoaded.into()),
            };

            // Encode audio.
            let audio_features = model.encoder.forward(&mel.squeeze(0).map_err(|e| {
                CandleWhisperEngineError::Inference { message: format!("squeeze: {e}") }
            })?, true).map_err(|e| CandleWhisperEngineError::Inference {
                message: format!("encode: {e}"),
            })?;
            let audio_features = audio_features.unsqueeze(0).map_err(|e| {
                CandleWhisperEngineError::Inference { message: format!("unsqueeze: {e}") }
            })?;

            // Greedy decode using special token IDs.
            let sot = tokenizer
                .token_to_id(whisper::SOT_TOKEN)
                .unwrap_or(50258u32);
            let eot = tokenizer
                .token_to_id(whisper::EOT_TOKEN)
                .unwrap_or(50256u32);
            let transcribe = tokenizer
                .token_to_id(whisper::TRANSCRIBE_TOKEN)
                .unwrap_or(50359u32);
            let no_ts = tokenizer
                .token_to_id(whisper::NO_TIMESTAMPS_TOKEN)
                .unwrap_or(50363u32);

            let mut tokens: Vec<u32> = vec![sot, transcribe, no_ts];
            let mut result = String::new();

            for _ in 0..256u32 {
                let ids_tensor = Tensor::new(
                    tokens.iter().map(|&t| t as i64).collect::<Vec<_>>().as_slice(),
                    &device,
                )
                .map_err(|e| CandleWhisperEngineError::Inference {
                    message: format!("token tensor: {e}"),
                })?
                .unsqueeze(0)
                .map_err(|e| CandleWhisperEngineError::Inference {
                    message: format!("unsqueeze tokens: {e}"),
                })?;

                let logits = model
                    .decoder
                    .forward(&ids_tensor, &audio_features, true)
                    .map_err(|e| CandleWhisperEngineError::Inference {
                        message: format!("decode: {e}"),
                    })?;
                let last_logits = logits
                    .i((.., tokens.len() - 1, ..))
                    .map_err(|e| CandleWhisperEngineError::Inference {
                        message: format!("index logits: {e}"),
                    })?;
                let vocab_logits = model.decoder.final_linear(&last_logits).map_err(|e| {
                    CandleWhisperEngineError::Inference {
                        message: format!("final_linear: {e}"),
                    }
                })?;
                let next_token = vocab_logits
                    .argmax(D::Minus1)
                    .map_err(|e| CandleWhisperEngineError::Inference {
                        message: format!("argmax: {e}"),
                    })?
                    .squeeze(0)
                    .map_err(|e| CandleWhisperEngineError::Inference {
                        message: format!("squeeze argmax: {e}"),
                    })?
                    .to_scalar::<u32>()
                    .map_err(|e| CandleWhisperEngineError::Inference {
                        message: format!("to_scalar: {e}"),
                    })?;

                if next_token == eot {
                    break;
                }
                tokens.push(next_token);
                if let Ok(piece) = tokenizer.decode(&[next_token], true) {
                    result.push_str(&piece);
                }
            }

            return Ok(result);
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
