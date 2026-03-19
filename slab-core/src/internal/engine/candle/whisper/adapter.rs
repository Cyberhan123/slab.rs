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

use crate::internal::engine::EngineError;

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
    /// Whisper model config stored at `load_model` time.  Used in `inference`
    /// to derive mel-spectrogram dimensions instead of relying on hard-coded
    /// values.
    #[cfg(feature = "candle")]
    config: Option<candle_transformers::models::whisper::Config>,
    /// Mel filter bank stored at `load_model` time.
    ///
    /// Loaded from `mel_filters.npz` (key `mel_<n_mels>`) when present in
    /// the model directory.  Falls back to a unit filter bank when the file
    /// is absent — transcription quality degrades in that case.
    #[cfg(feature = "candle")]
    mel_filters: Option<Vec<f32>>,
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
                // Fall back to tiny-model defaults (80-mel, English, 384-dim).
                Config {
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
                }
            };

            let vb = unsafe {
                VarBuilder::from_mmaped_safetensors(&[path], candle_core::DType::F32, &device)
                    .map_err(|e| CandleWhisperEngineError::LoadModel {
                        model_path: model_path.to_owned(),
                        message: e.to_string(),
                    })?
            };

            let model = whisper::model::Whisper::load(&vb, config.clone()).map_err(|e| {
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

            // Load mel filter bank from `mel_filters.npz` if present, otherwise
            // fall back to a unit bank and warn.
            let n_fft_half = whisper::N_FFT / 2 + 1;
            let n_mels = config.num_mel_bins;
            let filters_path = path
                .parent()
                .unwrap_or(Path::new("."))
                .join("mel_filters.npz");
            let mel_filters: Vec<f32> = if filters_path.exists() {
                match candle_core::npy::NpzTensors::new(&filters_path).and_then(|npz| {
                    let key = format!("mel_{n_mels}");
                    npz.get(&key)?.ok_or_else(|| {
                        candle_core::Error::Msg(format!("mel_filters.npz missing key '{key}'"))
                    })
                }) {
                    Ok(tensor) => tensor
                        .flatten_all()
                        .and_then(|t| t.to_vec1::<f32>())
                        .unwrap_or_else(|e| {
                            tracing::warn!(?e, "failed to read mel filter tensor; using unit bank");
                            vec![1.0f32 / n_fft_half as f32; n_mels * n_fft_half]
                        }),
                    Err(e) => {
                        tracing::warn!(
                            ?e,
                            path = %filters_path.display(),
                            "failed to load mel_filters.npz; using unit filter bank"
                        );
                        vec![1.0f32 / n_fft_half as f32; n_mels * n_fft_half]
                    }
                }
            } else {
                tracing::warn!(
                    path = %filters_path.display(),
                    "mel_filters.npz not found; using unit mel filter bank \
                     (transcription quality will be degraded)"
                );
                vec![1.0f32 / n_fft_half as f32; n_mels * n_fft_half]
            };

            let mut state =
                self.inner
                    .write()
                    .map_err(|_| CandleWhisperEngineError::LockPoisoned {
                        operation: "write whisper model state",
                    })?;
            state.model = Some(model);
            state.tokenizer = Some(tok);
            state.config = Some(config);
            state.mel_filters = Some(mel_filters);

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
            #[cfg(feature = "candle")]
            {
                state.config = None;
                state.mel_filters = None;
            }
        }
    }

    /// Transcribe raw 16 kHz mono f32 PCM samples.
    ///
    /// Returns the transcribed text as a plain UTF-8 string produced by greedy
    /// token decoding.  No timestamps or segment markers are included in the
    /// output.
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

            // Borrow the stored config and mel filter bank.
            let (n_mels, n_fft_half, mel_filters_clone) = {
                let s = self
                    .inner
                    .read()
                    .map_err(|_| CandleWhisperEngineError::LockPoisoned {
                        operation: "read whisper state for config",
                    })?;
                if s.model.is_none() {
                    return Err(CandleWhisperEngineError::ModelNotLoaded.into());
                }
                let cfg = s
                    .config
                    .as_ref()
                    .ok_or(CandleWhisperEngineError::ModelNotLoaded)?;
                let n_mels = cfg.num_mel_bins;
                let n_fft_half = whisper::N_FFT / 2 + 1;
                let filters = s
                    .mel_filters
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| vec![1.0f32 / n_fft_half as f32; n_mels * n_fft_half]);
                (n_mels, n_fft_half, filters)
            };
            let _ = n_fft_half; // used only to build the fallback filter length above

            // Acquire a borrow of the stored config for pcm_to_mel.
            let cfg_clone = {
                let s = self
                    .inner
                    .read()
                    .map_err(|_| CandleWhisperEngineError::LockPoisoned {
                        operation: "read whisper config for mel",
                    })?;
                s.config
                    .clone()
                    .ok_or(CandleWhisperEngineError::ModelNotLoaded)?
            };

            let mel_data = audio::pcm_to_mel(&cfg_clone, samples, &mel_filters_clone);
            let n_frames = mel_data.len() / n_mels;

            let mel =
                Tensor::from_vec(mel_data, (1usize, n_mels, n_frames), &device).map_err(|e| {
                    CandleWhisperEngineError::Inference {
                        message: format!("mel tensor: {e}"),
                    }
                })?;

            let mut state =
                self.inner
                    .write()
                    .map_err(|_| CandleWhisperEngineError::LockPoisoned {
                        operation: "lock whisper state for inference",
                    })?;

            // Pre-extract special token IDs and clone the tokenizer to avoid
            // holding an immutable borrow of `state.tokenizer` simultaneously
            // with the mutable borrow of `state.model` that the decode loop needs.
            let (sot, eot, transcribe_tok, no_ts, tokenizer_clone) = {
                let tok = state
                    .tokenizer
                    .as_ref()
                    .ok_or(CandleWhisperEngineError::ModelNotLoaded)?;
                (
                    tok.token_to_id(whisper::SOT_TOKEN).unwrap_or(50258u32),
                    tok.token_to_id(whisper::EOT_TOKEN).unwrap_or(50256u32),
                    tok.token_to_id(whisper::TRANSCRIBE_TOKEN)
                        .unwrap_or(50359u32),
                    tok.token_to_id(whisper::NO_TIMESTAMPS_TOKEN)
                        .unwrap_or(50363u32),
                    tok.clone(),
                )
            };

            let model = state
                .model
                .as_mut()
                .ok_or(CandleWhisperEngineError::ModelNotLoaded)?;

            // Encode audio.
            let audio_features = model
                .encoder
                .forward(
                    &mel.squeeze(0)
                        .map_err(|e| CandleWhisperEngineError::Inference {
                            message: format!("squeeze: {e}"),
                        })?,
                    true,
                )
                .map_err(|e| CandleWhisperEngineError::Inference {
                    message: format!("encode: {e}"),
                })?;
            let audio_features =
                audio_features
                    .unsqueeze(0)
                    .map_err(|e| CandleWhisperEngineError::Inference {
                        message: format!("unsqueeze: {e}"),
                    })?;

            // Greedy decode using special token IDs.
            let mut tokens: Vec<u32> = vec![sot, transcribe_tok, no_ts];
            let mut result = String::new();

            for _ in 0..256u32 {
                let ids_tensor = Tensor::new(
                    tokens
                        .iter()
                        .map(|&t| t as i64)
                        .collect::<Vec<_>>()
                        .as_slice(),
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
                let last_logits = logits.i((.., tokens.len() - 1, ..)).map_err(|e| {
                    CandleWhisperEngineError::Inference {
                        message: format!("index logits: {e}"),
                    }
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
                if let Ok(piece) = tokenizer_clone.decode(&[next_token], true) {
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
