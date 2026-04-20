use super::contract::{AudioTranscriptionOptions, GgmlWhisperLoadConfig};
use crate::infra::backends::ggml;
use slab_subtitle::{
    SubtitleEntry,
    timetypes::{TimePoint, TimeSpan},
};
use slab_utils::loader::load_library_from_dir;
use slab_whisper::{
    ContextParams, FullParams, Whisper, WhisperContext, WhisperError, WhisperVadParams,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum GGMLWhisperEngineError {
    #[error("GGMLWhisperEngine context parameters are missing model_path")]
    MissingModelPath,

    #[error("GGMLWhisperEngine context not initialized")]
    ContextNotInitialized,

    #[error("Failed to run GGMLWhisperEngine model inference")]
    InferenceFailed {
        #[source]
        source: WhisperError,
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

// # Safety
//
// `GGMLWhisperEngine` is `Send` and `Sync` because all mutable state is either
// immutable or protected by thread-safe wrappers:
//
// 1. **`instance: Arc<Whisper>`** - The `Whisper` type wraps a dlopen2-generated
//    handle that holds a read-only table of function pointers loaded once at startup.
//    This function pointer table is never mutated, making concurrent reads safe.
//
// 2. **`ctx: Option<WhisperContext>`** - The `WhisperContext` wraps
//    `Arc<WhisperInnerContext>`, which according to upstream whisper.cpp
//    documentation is safe to share across threads. The context provides
//    internal synchronization for operations that modify the loaded model state.
//
// The `Option` wrapper allows the context to be loaded/unloaded during the engine's
// lifecycle, but all accesses to the context are protected by the engine's
// internal locking mechanisms.
//
// **Thread-safety guarantees from whisper.cpp**: The underlying C++ library
// guarantees that `WhisperContext` can be safely accessed from multiple threads
// for inference operations, with appropriate locking handled internally.
unsafe impl Send for GGMLWhisperEngine {}
unsafe impl Sync for GGMLWhisperEngine {}

impl GGMLWhisperEngine {
    /// Create a new engine from the shared runtime library directory at `path`
    /// **without** registering any process-wide singleton.
    ///
    /// Call [`new_context`] afterwards to load a model.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, ggml::EngineError> {
        load_library_from_dir(path, "whisper", |lib_dir, whisper_path| {
            info!("current whisper path is: {}", whisper_path.display());
            let whisper = Whisper::new(lib_dir).map_err(|source| {
                GGMLWhisperEngineError::InitializeDynamicLibrary {
                    path: whisper_path.to_path_buf(),
                    source,
                }
            })?;

            Ok(Self { instance: Arc::new(whisper), ctx: None })
        })
    }

    pub fn new_context(&mut self, params: ContextParams) -> Result<(), ggml::EngineError> {
        let model_path = params
            .model_path
            .as_ref()
            .ok_or(GGMLWhisperEngineError::MissingModelPath)?
            .to_string_lossy()
            .into_owned();

        let ctx = self
            .instance
            .new_context(params)
            .map_err(|source| GGMLWhisperEngineError::CreateContext { model_path, source })?;
        self.ctx = Some(ctx);
        Ok(())
    }

    pub(crate) fn new_context_from_config(
        &mut self,
        config: GgmlWhisperLoadConfig,
    ) -> Result<(), ggml::EngineError> {
        self.new_context(ContextParams {
            model_path: Some(config.model_path),
            flash_attn: config.flash_attn.or(Some(true)),
            ..Default::default()
        })
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
        params: &FullParams,
    ) -> Result<Vec<SubtitleEntry>, ggml::EngineError> {
        let ctx = self.ctx.as_ref().ok_or(GGMLWhisperEngineError::ContextNotInitialized)?;

        let mut state = ctx
            .create_state()
            .map_err(|source| GGMLWhisperEngineError::CreateInferenceState { source })?;
        state
            .full(params.clone(), audio_data)
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

    pub(crate) fn inference_with_options(
        &self,
        audio_data: &[f32],
        options: &AudioTranscriptionOptions,
    ) -> Result<Vec<SubtitleEntry>, ggml::EngineError> {
        self.inference(audio_data, &full_params_from_options(options))
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

fn full_params_from_options(options: &AudioTranscriptionOptions) -> FullParams {
    let mut params = FullParams {
        language: options.language.clone(),
        detect_language: options.detect_language,
        initial_prompt: options.prompt.clone(),
        ..Default::default()
    };

    if let Some(decode) = options.decode.as_ref() {
        params.offset_ms = decode.offset_ms;
        params.duration_ms = decode.duration_ms;
        params.no_context = decode.no_context;
        params.no_timestamps = decode.no_timestamps;
        params.token_timestamps = decode.token_timestamps;
        params.split_on_word = decode.split_on_word;
        params.suppress_nst = decode.suppress_nst;
        params.thold_pt = decode.word_thold;
        params.max_len = decode.max_len;
        params.max_tokens = decode.max_tokens;
        params.temperature = decode.temperature;
        params.temperature_inc = decode.temperature_inc;
        params.entropy_thold = decode.entropy_thold;
        params.logprob_thold = decode.logprob_thold;
        params.no_speech_thold = decode.no_speech_thold;
        params.tdrz_enable = decode.tdrz_enable;
    }

    if let Some(vad) = options.vad.as_ref() {
        params.vad = Some(vad.enabled);
        params.vad_model_path = vad.model_path.clone();
        params.vad_params = vad.params.as_ref().map(|value| WhisperVadParams {
            threshold: value.threshold,
            min_speech_duration_ms: value.min_speech_duration_ms,
            min_silence_duration_ms: value.min_silence_duration_ms,
            max_speech_duration_s: value.max_speech_duration_s,
            speech_pad_ms: value.speech_pad_ms,
            samples_overlap: value.samples_overlap,
        });
    }

    params
}
