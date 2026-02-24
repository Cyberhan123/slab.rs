use crate::engine;
use slab_whisper::{SamplingStrategy, Whisper, WhisperContext, WhisperContextParameters};
use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use subparse::{
    timetypes::{TimePoint, TimeSpan},
    SubtitleEntry,
};
use thiserror::Error;
use tracing::info;

struct WhisperGlobal {
    engine: Arc<GGMLWhisperEngine>,
    lib_path: PathBuf,
}

static INSTANCE: OnceLock<RwLock<Option<WhisperGlobal>>> = OnceLock::new();

#[derive(Debug, Error)]
pub enum GGMLWhisperEngineError {
    #[error(
        "GGMLWhisperEngine already initialized with different library path: {existing} (requested: {requested})"
    )]
    LibraryPathMismatch { existing: PathBuf, requested: PathBuf },

    #[error("GGMLWhisperEngine global storage not initialized")]
    GlobalStorageNotInitialized,

    #[error("GGMLWhisperEngine instance not initialized")]
    InstanceNotInitialized,

    #[error("Lock poisoned while trying to {operation}")]
    LockPoisoned { operation: &'static str },

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

#[derive(Debug)]
pub struct GGMLWhisperEngine {
    instance: Arc<Whisper>,
    ctx: Arc<Mutex<Option<WhisperContext>>>,
}

// SAFETY: GGMLWhisperEngine is only accessed through Arc<Mutex<...>> for mutable state.
// The `instance: Arc<Whisper>` field wraps a dynamically loaded library handle which is
// immutable after creation (contexts and params are created from it, not mutated).
// All mutable inference state is guarded by the `ctx: Arc<Mutex<...>>` field.
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

    fn build_service(normalized_path: &Path) -> Result<Self, engine::EngineError> {
        info!("current whisper path is: {}", normalized_path.display());
        let whisper = Whisper::new(normalized_path.to_path_buf()).map_err(|source| {
            GGMLWhisperEngineError::InitializeDynamicLibrary {
                path: normalized_path.to_path_buf(),
                source: source.into(),
            }
        })?;

        Ok(Self {
            instance: Arc::new(whisper),
            ctx: Arc::new(Mutex::new(None)),
        })
    }

    /// Create a new engine from the library at `path` **without** registering
    /// any process-wide singleton.
    ///
    /// Call [`new_context`] afterwards to load a model.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Arc<Self>, engine::EngineError> {
        let normalized = Self::resolve_lib_path(path)?;
        let engine = Self::build_service(&normalized)?;
        Ok(Arc::new(engine))
    }

    pub fn init<P: AsRef<Path>>(path: P) -> Result<Arc<Self>, engine::EngineError> {
        let normalized_path = Self::resolve_lib_path(path)?;
        let global_lock = INSTANCE.get_or_init(|| RwLock::new(None));

        {
            let read_guard = global_lock
                .read()
                .map_err(|_| GGMLWhisperEngineError::LockPoisoned {
                    operation: "read whisper global state",
                })?;
            if let Some(global) = read_guard.as_ref() {
                if global.lib_path != normalized_path {
                    return Err(GGMLWhisperEngineError::LibraryPathMismatch {
                        existing: global.lib_path.clone(),
                        requested: normalized_path.clone(),
                    }
                    .into());
                }
                return Ok(global.engine.clone());
            }
        }

        let engine = Arc::new(Self::build_service(&normalized_path)?);
        let mut write_guard = global_lock
            .write()
            .map_err(|_| GGMLWhisperEngineError::LockPoisoned {
                operation: "write whisper global state",
            })?;

        if let Some(global) = write_guard.as_ref() {
            if global.lib_path != normalized_path {
                return Err(GGMLWhisperEngineError::LibraryPathMismatch {
                    existing: global.lib_path.clone(),
                    requested: normalized_path.clone(),
                }
                .into());
            }
            return Ok(global.engine.clone());
        }

        *write_guard = Some(WhisperGlobal {
            engine: engine.clone(),
            lib_path: normalized_path,
        });

        Ok(engine)
    }

    pub fn reload<P: AsRef<Path>>(path: P) -> Result<Arc<Self>, engine::EngineError> {
        let normalized_path = Self::resolve_lib_path(path)?;
        let engine = Arc::new(Self::build_service(&normalized_path)?);
        let global_lock = INSTANCE.get_or_init(|| RwLock::new(None));
        let mut write_guard = global_lock
            .write()
            .map_err(|_| GGMLWhisperEngineError::LockPoisoned {
                operation: "write whisper global state",
            })?;

        let previous = write_guard
            .as_ref()
            .map(|g| g.lib_path.display().to_string())
            .unwrap_or_else(|| "<uninitialized>".to_string());

        *write_guard = Some(WhisperGlobal {
            engine: engine.clone(),
            lib_path: normalized_path.clone(),
        });

        info!(
            "whisper engine reloaded: {} -> {}",
            previous,
            normalized_path.display()
        );

        Ok(engine)
    }

    pub fn current() -> Result<Arc<Self>, engine::EngineError> {
        let global_lock = INSTANCE
            .get()
            .ok_or(GGMLWhisperEngineError::GlobalStorageNotInitialized)?;
        let read_guard = global_lock
            .read()
            .map_err(|_| GGMLWhisperEngineError::LockPoisoned {
                operation: "read whisper global state",
            })?;
        read_guard
            .as_ref()
            .map(|global| global.engine.clone())
            .ok_or(GGMLWhisperEngineError::InstanceNotInitialized.into())
    }

    pub fn new_context<P: AsRef<Path>>(
        &self,
        path_to_model: P,
        params: WhisperContextParameters,
    ) -> Result<(), engine::EngineError> {
        let mut ctx_lock = self
            .ctx
            .lock()
            .map_err(|_| GGMLWhisperEngineError::LockPoisoned {
                operation: "lock whisper context",
            })?;
        *ctx_lock = None;

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
        *ctx_lock = Some(ctx);

        Ok(())
    }

    pub fn inference<P: AsRef<Path>>(
        &self,
        audio_data: &[f32],
    ) -> Result<Vec<SubtitleEntry>, engine::EngineError> {

        let ctx_lock = self
            .ctx
            .lock()
            .map_err(|_| GGMLWhisperEngineError::LockPoisoned {
                operation: "lock whisper context",
            })?;

        let ctx = ctx_lock
            .as_ref()
            .ok_or(GGMLWhisperEngineError::ContextNotInitialized)?;

        let params = self.instance.new_full_params(SamplingStrategy::BeamSearch {
            beam_size: 5,
            patience: -1.0,
        });

        let mut state = ctx
            .create_state()
            .map_err(|source| GGMLWhisperEngineError::CreateInferenceState {
                source: source.into(),
            })?;
        state
            .full(params, &audio_data[..])
            .map_err(|source| GGMLWhisperEngineError::InferenceFailed {
                source: source.into(),
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
}

#[cfg(test)]
mod test {

    use hf_hub::api::sync::Api;
    use tokio;
    use super::*;

     fn ensure_whisper_dir() -> PathBuf {
        let mut test_data_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        test_data_path.push("../testdata");
        test_data_path.join("whisper")
    }

    #[tokio::test]
    async fn test_whisper_current_and_reload() {
        let whisper_dir = ensure_whisper_dir();

        let initial = GGMLWhisperEngine::init(whisper_dir.as_path())
            .expect("failed to initialize whisper service");
        let current = GGMLWhisperEngine::current().expect("failed to get current whisper service");
        assert!(Arc::ptr_eq(&initial, &current));

        let reloaded = GGMLWhisperEngine::reload(whisper_dir.as_path())
            .expect("failed to reload whisper service");
        let current_after_reload =
            GGMLWhisperEngine::current().expect("failed to get current whisper service after reload");

        assert!(Arc::ptr_eq(&reloaded, &current_after_reload));
        assert!(!Arc::ptr_eq(&initial, &reloaded));
    }

    #[tokio::test]
    async fn test_whisper() {
        let mut test_data_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        test_data_path.push("../testdata");
        println!("Current executable path: {:?}", test_data_path);

        let path = ensure_whisper_dir();

        let ws = GGMLWhisperEngine::init(path.as_path()).expect("failed to initialize whisper service");

        let api = Api::new().expect("fail to init hf-api");
        let model_path = api
            .model("Aratako/anime-whisper-ggml".into())
            .get("ggml-anime-whisper.bin")
            .expect("error download model");

        let mut params = WhisperContextParameters::default();
        params.flash_attn(true).use_gpu(true);

        ws.new_context(model_path.as_path(), params)
            .expect("load model failed");
        // let jfk_audio_path = test_data_path.join("samples/jfk.wav");
        // let srt_entries = ws
        //     .inference(jfk_audio_path)
        //     .await
        //     .expect("Inference failed");

        // let srt_services = SubtitleService::new();

        // let file_path = test_data_path.join("whisper_test.srt");
        // srt_services
        //     .to_srt_file(file_path, srt_entries)
        //     .expect("srt failed")
    }
}
