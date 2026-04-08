use crate::infra::backends::ggml::internal::engine;
use slab_subtitle::{
    SubtitleEntry,
    timetypes::{TimePoint, TimeSpan},
};
use slab_utils::loader::load_library_from_dir;
use slab_whisper::{ContextParams, FullParams, Whisper, WhisperContext, WhisperError};
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

// SAFETY: GGMLWhisperEngine is owned exclusively by its worker task.
// `instance: Arc<Whisper>` is an immutable library handle safe to move
// between threads.  `ctx: Option<WhisperContext>` wraps Arc<WhisperInnerContext>
// which is Send + Sync per the upstream whisper.cpp thread-safety guarantees.
unsafe impl Send for GGMLWhisperEngine {}
unsafe impl Sync for GGMLWhisperEngine {}

impl GGMLWhisperEngine {
    /// Create a new engine from the shared runtime library directory at `path`
    /// **without** registering any process-wide singleton.
    ///
    /// Call [`new_context`] afterwards to load a model.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, engine::EngineError> {
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

    pub fn new_context(&mut self, params: ContextParams) -> Result<(), engine::EngineError> {
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
    ) -> Result<Vec<SubtitleEntry>, engine::EngineError> {
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
