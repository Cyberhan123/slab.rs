use crate::domain::models::{AudioTranscriptionOptions, GgmlParakeetLoadConfig};
use crate::infra::backends::ggml;
use slab_parakeet::{ContextParams, FullParams, Parakeet, ParakeetContext, ParakeetError};
use slab_subtitle::{
    SubtitleEntry,
    timetypes::{TimePoint, TimeSpan},
};
use slab_utils::loader::load_library_from_dir;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum GGMLParakeetEngineError {
    #[error("GGMLParakeetEngine context parameters are missing model_path")]
    MissingModelPath,

    #[error("GGMLParakeetEngine context not initialized")]
    ContextNotInitialized,

    #[error("Failed to run GGMLParakeetEngine model inference")]
    InferenceFailed {
        #[source]
        source: ParakeetError,
    },

    #[error("Failed to initialize GGMLParakeetEngine dynamic library at: {path}")]
    InitializeDynamicLibrary {
        path: PathBuf,
        #[source]
        source: ParakeetError,
    },

    #[error("Failed to create GGMLParakeetEngine context with model: {model_path}")]
    CreateContext {
        model_path: String,
        #[source]
        source: ParakeetError,
    },

    #[error("Failed to create GGMLParakeetEngine inference state")]
    CreateInferenceState {
        #[source]
        source: ParakeetError,
    },
}

/// Engine wrapping a parakeet shared library handle.
///
/// Each instance owns its own model context (`ctx`). There is no shared mutable
/// state between separate `GGMLParakeetEngine` instances, so no `Mutex` is
/// needed. The backend worker owns the engine exclusively and mutates it via
/// `&mut self`.
#[derive(Debug)]
pub struct GGMLParakeetEngine {
    instance: Arc<Parakeet>,
    // Owned per-engine context; not shared across instances.
    ctx: Option<ParakeetContext>,
}

// # Safety
//
// `GGMLParakeetEngine` is `Send` and `Sync` because all mutable state is either
// immutable or protected by thread-safe wrappers:
//
// 1. **`instance: Arc<Parakeet>`** - the handle wraps a read-only table of
//    function pointers loaded once at startup. This table is never mutated,
//    making concurrent reads safe.
//
// 2. **`ctx: Option<ParakeetContext>`** - `ParakeetContext` wraps
//    `Arc<ParakeetInnerContext>`, which (like whisper.cpp) is safe to share
//    across threads; the context provides internal synchronization for
//    operations that modify the loaded model state.
//
// The `Option` wrapper allows the context to be loaded/unloaded during the
// engine's lifecycle, but all accesses to the context are exclusive through the
// worker's `&mut self`.
unsafe impl Send for GGMLParakeetEngine {}
unsafe impl Sync for GGMLParakeetEngine {}

impl GGMLParakeetEngine {
    /// Create a new engine from the shared runtime library directory at `path`.
    ///
    /// Call [`new_context`] afterwards to load a model.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, ggml::EngineError> {
        load_library_from_dir(path, "parakeet", |lib_dir, parakeet_path| {
            info!("current parakeet path is: {}", parakeet_path.display());
            let parakeet = Parakeet::new(lib_dir).map_err(|source| {
                GGMLParakeetEngineError::InitializeDynamicLibrary {
                    path: parakeet_path.to_path_buf(),
                    source,
                }
            })?;

            Ok(Self { instance: Arc::new(parakeet), ctx: None })
        })
    }

    pub fn new_context(&mut self, params: ContextParams) -> Result<(), ggml::EngineError> {
        let model_path = params
            .model_path
            .as_ref()
            .ok_or(GGMLParakeetEngineError::MissingModelPath)?
            .to_string_lossy()
            .into_owned();

        let ctx = self
            .instance
            .new_context(params)
            .map_err(|source| GGMLParakeetEngineError::CreateContext { model_path, source })?;
        self.ctx = Some(ctx);
        Ok(())
    }

    pub(crate) fn new_context_from_config(
        &mut self,
        config: GgmlParakeetLoadConfig,
    ) -> Result<(), ggml::EngineError> {
        self.new_context(ContextParams::new(config.model_path))
    }

    /// Run parakeet inference on the provided audio samples.
    ///
    /// # Arguments
    /// * `audio_data` - PCM audio samples as f32 values (typically 16 kHz mono)
    ///
    /// # Returns
    /// Vector of subtitle entries with transcribed text and timestamps.
    pub fn inference(
        &self,
        audio_data: &[f32],
        params: &FullParams,
    ) -> Result<Vec<SubtitleEntry>, ggml::EngineError> {
        let ctx = self.ctx.as_ref().ok_or(GGMLParakeetEngineError::ContextNotInitialized)?;

        let mut state = ctx
            .create_state()
            .map_err(|source| GGMLParakeetEngineError::CreateInferenceState { source })?;
        state
            .full(params.clone(), audio_data)
            .map_err(|source| GGMLParakeetEngineError::InferenceFailed { source })?;

        let srt_entries: Vec<SubtitleEntry> = state
            .as_iter()
            .map(|segment| {
                SubtitleEntry {
                    timespan: TimeSpan::new(
                        // centiseconds -> milliseconds
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

    /// Create a new engine that shares the same library handle but has no model
    /// context loaded. Used when spawning additional workers.
    pub fn fork_library(&self) -> Self {
        Self { instance: Arc::clone(&self.instance), ctx: None }
    }
}

/// Map the runtime transcription options onto parakeet's (greedy-only) full
/// params. Parakeet exposes only a subset of whisper's decode knobs and has no
/// language detection / prompt / VAD, so those fields are ignored.
fn full_params_from_options(options: &AudioTranscriptionOptions) -> FullParams {
    let mut params = FullParams::default();

    if let Some(decode) = options.decode.as_ref() {
        params.offset_ms = decode.offset_ms;
        params.duration_ms = decode.duration_ms;
        params.no_context = decode.no_context;
    }

    params
}
