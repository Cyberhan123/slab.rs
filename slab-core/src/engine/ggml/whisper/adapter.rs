use crate::engine;
use slab_whisper::{SamplingStrategy, Whisper, WhisperContext, WhisperContextParameters};
use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use subparse::{
    timetypes::{TimePoint, TimeSpan},
    SubtitleEntry,
};
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
        source: anyhow::Error,
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
        source: anyhow::Error,
    },

    #[error("Failed to create GGMLWhisperEngine context with model: {model_path}")]
    CreateContext {
        model_path: String,
        #[source]
        source: anyhow::Error,
    },

    #[error("Failed to create GGMLWhisperEngine inference state")]
    CreateInferenceState {
        #[source]
        source: anyhow::Error,
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
    fn resolve_lib_path<P: AsRef<Path>>(path: P) -> Result<PathBuf, engine::EngineError> {
        let whisper_lib_name = format!("{}whisper{}", DLL_PREFIX, DLL_SUFFIX);

        let mut lib_path = path.as_ref().to_path_buf();
        if lib_path.file_name() != Some(OsStr::new(&whisper_lib_name)) {
            lib_path.push(&whisper_lib_name);
        }

        std::fs::canonicalize(&lib_path).map_err(|source| {
            GGMLWhisperEngineError::CanonicalizeLibraryPath {
                path: lib_path,
                source,
            }
            .into()
        })
    }

    fn build_engine(normalized_path: &Path) -> Result<Self, engine::EngineError> {
        info!("current whisper path is: {}", normalized_path.display());
        let whisper = Whisper::new(normalized_path.to_path_buf()).map_err(|source| {
            GGMLWhisperEngineError::InitializeDynamicLibrary {
                path: normalized_path.to_path_buf(),
                source: source.into(),
            }
        })?;

        Ok(Self {
            instance: Arc::new(whisper),
            ctx: None,
        })
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
        let path = path_to_model
            .as_ref()
            .to_str()
            .ok_or(GGMLWhisperEngineError::InvalidModelPathUtf8)?;

        let ctx = self
            .instance
            .new_context_with_params(path, params)
            .map_err(|source| GGMLWhisperEngineError::CreateContext {
                model_path: path.to_string(),
                source: source.into(),
            })?;
        self.ctx = Some(ctx);
        Ok(())
    }

    pub fn inference<P: AsRef<Path>>(
        &self,
        audio_data: &[f32],
    ) -> Result<Vec<SubtitleEntry>, engine::EngineError> {
        let ctx = self
            .ctx
            .as_ref()
            .ok_or(GGMLWhisperEngineError::ContextNotInitialized)?;

        let params = self.instance.new_full_params(SamplingStrategy::BeamSearch {
            beam_size: 5,
            patience: -1.0,
        });

        let mut state =
            ctx.create_state()
                .map_err(|source| GGMLWhisperEngineError::CreateInferenceState {
                    source: source.into(),
                })?;
        state.full(params, &audio_data[..]).map_err(|source| {
            GGMLWhisperEngineError::InferenceFailed {
                source: source.into(),
            }
        })?;

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
        Self {
            instance: Arc::clone(&self.instance),
            ctx: None,
        }
    }
}
