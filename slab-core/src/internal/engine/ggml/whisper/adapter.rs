use crate::internal::engine;
use slab_subtitle::{
    timetypes::{TimePoint, TimeSpan},
    SubtitleEntry,
};
use slab_whisper::{
    SamplingStrategy, Whisper, WhisperContext, WhisperContextParameters, WhisperError,
    WhisperVadParams,
};
use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum GGMLWhisperEngineError {
    #[error("Model path contains invalid UTF-8")]
    InvalidModelPathUtf8,

    #[error("GGMLWhisperEngine context not initialized")]
    ContextNotInitialized,

    #[error("Failed to run GGMLWhisperEngine model inference")]
    InferenceFailed {
        #[source]
        source: WhisperError,
    },

    #[error("Failed to canonicalize GGMLWhisperEngine library path: {path}")]
    CanonicalizeLibraryPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to initialize GGMLWhisperEngine dynamic library at: {path}")]
    InitializeDynamicLibrary {
        path: PathBuf,
        #[source]
        source: WhisperError,
    },

    #[error("Failed to create GGMLWhisperEngine context with model: {model_path}")]
    CreateContext {
        model_path: String,
        #[source]
        source: WhisperError,
    },

    #[error("Failed to create GGMLWhisperEngine inference state")]
    CreateInferenceState {
        #[source]
        source: WhisperError,
    },

    #[error("VAD model path must not be empty")]
    InvalidVadModelPath,

    #[error("Failed to configure VAD for whisper inference")]
    ConfigureVad {
        #[source]
        source: WhisperError,
    },
}

#[derive(Debug, Clone)]
pub struct WhisperVadConfig {
    pub model_path: String,
    pub threshold: Option<f32>,
    pub min_speech_duration_ms: Option<i32>,
    pub min_silence_duration_ms: Option<i32>,
    pub max_speech_duration_s: Option<f32>,
    pub speech_pad_ms: Option<i32>,
    pub samples_overlap: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct WhisperDecodeConfig {
    pub offset_ms: Option<i32>,
    pub duration_ms: Option<i32>,
    pub no_context: Option<bool>,
    pub no_timestamps: Option<bool>,
    pub token_timestamps: Option<bool>,
    pub split_on_word: Option<bool>,
    pub suppress_nst: Option<bool>,
    pub word_thold: Option<f32>,
    pub max_len: Option<i32>,
    pub max_tokens: Option<i32>,
    pub temperature: Option<f32>,
    pub temperature_inc: Option<f32>,
    pub entropy_thold: Option<f32>,
    pub logprob_thold: Option<f32>,
    pub no_speech_thold: Option<f32>,
    pub tdrz_enable: Option<bool>,
}

/// Engine wrapping a Whisper shared library handle.
///
/// Each instance owns its own model context (`ctx`).  There is no shared
/// mutable state between separate `GGMLWhisperEngine` instances, so no
/// `Mutex` is needed.  The backend worker owns the engine exclusively and
/// mutates it via `&mut self`.
#[derive(Debug)]
pub struct GGMLWhisperEngine {
    instance: Arc<Whisper>,
    // Owned per-engine context; not shared across instances.
    ctx: Option<WhisperContext>,
}

// SAFETY: GGMLWhisperEngine is owned exclusively by its worker task.
// `instance: Arc<Whisper>` is an immutable library handle safe to move
// between threads.  `ctx: Option<WhisperContext>` wraps Arc<WhisperInnerContext>
// which is Send + Sync per the upstream whisper.cpp thread-safety guarantees.
unsafe impl Send for GGMLWhisperEngine {}
unsafe impl Sync for GGMLWhisperEngine {}

impl GGMLWhisperEngine {
    fn resolve_lib_path<P: AsRef<Path>>(path: P) -> Result<PathBuf, engine::EngineError> {
        let whisper_lib_name = format!("{}whisper{}", DLL_PREFIX, DLL_SUFFIX);

        let mut lib_path = path.as_ref().to_path_buf();
        if lib_path.file_name() != Some(OsStr::new(&whisper_lib_name)) {
            lib_path.push(&whisper_lib_name);
        }

        std::fs::canonicalize(&lib_path).map_err(|source| {
            GGMLWhisperEngineError::CanonicalizeLibraryPath { path: lib_path, source }.into()
        })
    }

    fn build_engine(normalized_path: &Path) -> Result<Self, engine::EngineError> {
        info!("current whisper path is: {}", normalized_path.display());
        let whisper = Whisper::new(normalized_path).map_err(|source| {
            GGMLWhisperEngineError::InitializeDynamicLibrary {
                path: normalized_path.to_path_buf(),
                source,
            }
        })?;

        Ok(Self { instance: Arc::new(whisper), ctx: None })
    }

    /// Create a new engine from the library at `path` **without** registering
    /// any process-wide singleton.
    ///
    /// Call [`new_context`] afterwards to load a model.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, engine::EngineError> {
        let normalized = Self::resolve_lib_path(path)?;
        Self::build_engine(&normalized)
    }

    pub fn new_context<P: AsRef<Path>>(
        &mut self,
        path_to_model: P,
        params: WhisperContextParameters,
    ) -> Result<(), engine::EngineError> {
        let path =
            path_to_model.as_ref().to_str().ok_or(GGMLWhisperEngineError::InvalidModelPathUtf8)?;

        let ctx = self.instance.new_context_with_params(path, params).map_err(|source| {
            GGMLWhisperEngineError::CreateContext { model_path: path.to_string(), source }
        })?;
        self.ctx = Some(ctx);
        Ok(())
    }

    /// Run Whisper inference on the provided audio samples.
    ///
    /// # Arguments
    /// * `audio_data` - PCM audio samples as f32 values (typically 16 kHz mono)
    ///
    /// # Returns
    /// Vector of subtitle entries with transcribed text and timestamps
    ///
    /// # Errors
    /// Returns `GGMLWhisperEngineError::ContextNotInitialized` if no model is loaded.
    /// Returns `GGMLWhisperEngineError::CreateInferenceState` if state creation fails.
    /// Returns `GGMLWhisperEngineError::InferenceFailed` if transcription fails.
    pub fn inference(
        &self,
        audio_data: &[f32],
        vad: Option<&WhisperVadConfig>,
        decode: Option<&WhisperDecodeConfig>,
    ) -> Result<Vec<SubtitleEntry>, engine::EngineError> {
        let ctx = self.ctx.as_ref().ok_or(GGMLWhisperEngineError::ContextNotInitialized)?;

        let mut params = self
            .instance
            .new_full_params(SamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 });

        if let Some(vad) = vad {
            let model_path = vad.model_path.trim();
            if model_path.is_empty() {
                return Err(GGMLWhisperEngineError::InvalidVadModelPath.into());
            }

            params.set_vad_model_path(Some(model_path));

            let mut vad_params = WhisperVadParams::new();
            if let Some(threshold) = vad.threshold {
                vad_params.set_threshold(threshold);
            }
            if let Some(min_speech_duration_ms) = vad.min_speech_duration_ms {
                vad_params.set_min_speech_duration(min_speech_duration_ms);
            }
            if let Some(min_silence_duration_ms) = vad.min_silence_duration_ms {
                vad_params.set_min_silence_duration(min_silence_duration_ms);
            }
            if let Some(max_speech_duration_s) = vad.max_speech_duration_s {
                vad_params.set_max_speech_duration(max_speech_duration_s);
            }
            if let Some(speech_pad_ms) = vad.speech_pad_ms {
                vad_params.set_speech_pad(speech_pad_ms);
            }
            if let Some(samples_overlap) = vad.samples_overlap {
                vad_params.set_samples_overlap(samples_overlap);
            }

            params.set_vad_params(vad_params);
            params
                .try_enable_vad(true)
                .map_err(|source| GGMLWhisperEngineError::ConfigureVad { source })?;
        }

        if let Some(decode) = decode {
            if let Some(offset_ms) = decode.offset_ms {
                params.set_offset_ms(offset_ms);
            }
            if let Some(duration_ms) = decode.duration_ms {
                params.set_duration_ms(duration_ms);
            }
            if let Some(no_context) = decode.no_context {
                params.set_no_context(no_context);
            }
            if let Some(no_timestamps) = decode.no_timestamps {
                params.set_no_timestamps(no_timestamps);
            }
            if let Some(token_timestamps) = decode.token_timestamps {
                params.set_token_timestamps(token_timestamps);
            }
            if let Some(split_on_word) = decode.split_on_word {
                params.set_split_on_word(split_on_word);
            }
            if let Some(suppress_nst) = decode.suppress_nst {
                params.set_suppress_nst(suppress_nst);
            }
            if let Some(word_thold) = decode.word_thold {
                params.set_thold_pt(word_thold);
            }
            if let Some(max_len) = decode.max_len {
                params.set_max_len(max_len);
            }
            if let Some(max_tokens) = decode.max_tokens {
                params.set_max_tokens(max_tokens);
            }
            if let Some(temperature) = decode.temperature {
                params.set_temperature(temperature);
            }
            if let Some(temperature_inc) = decode.temperature_inc {
                params.set_temperature_inc(temperature_inc);
            }
            if let Some(entropy_thold) = decode.entropy_thold {
                params.set_entropy_thold(entropy_thold);
            }
            if let Some(logprob_thold) = decode.logprob_thold {
                params.set_logprob_thold(logprob_thold);
            }
            if let Some(no_speech_thold) = decode.no_speech_thold {
                params.set_no_speech_thold(no_speech_thold);
            }
            if let Some(tdrz_enable) = decode.tdrz_enable {
                params.set_tdrz_enable(tdrz_enable);
            }
        }

        let mut state = ctx
            .create_state()
            .map_err(|source| GGMLWhisperEngineError::CreateInferenceState { source })?;
        state
            .full(params, audio_data)
            .map_err(|source| GGMLWhisperEngineError::InferenceFailed { source })?;

        let srt_entries: Vec<SubtitleEntry> = state
            .as_iter()
            .map(|segment| {
                SubtitleEntry {
                    timespan: TimeSpan::new(
                        // 从厘秒转换为毫秒
                        TimePoint::from_msecs(segment.start_timestamp() * 10),
                        TimePoint::from_msecs(segment.end_timestamp() * 10),
                    ),
                    line: Some(segment.to_string().trim().to_string()),
                }
            })
            .collect();
        Ok(srt_entries)
    }

    // unload the model. free ctx
    pub fn unload(&mut self) {
        self.ctx = None;
    }

    /// Returns `true` if a model context has been loaded.
    pub fn is_model_loaded(&self) -> bool {
        self.ctx.is_some()
    }

    /// Create a new engine that shares the same library handle but has no
    /// model context loaded.
    ///
    /// Used when spawning additional workers so each worker has its own
    /// `ctx` slot (loaded independently) while all workers share the same
    /// dynamic-library `Arc`.
    pub fn fork_library(&self) -> Self {
        Self { instance: Arc::clone(&self.instance), ctx: None }
    }
}
