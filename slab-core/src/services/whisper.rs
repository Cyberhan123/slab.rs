use crate::services::{self, ffmpeg};
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
    service: Arc<WhisperService>,
    lib_path: PathBuf,
}

static INSTANCE: OnceLock<RwLock<Option<WhisperGlobal>>> = OnceLock::new();

#[derive(Debug, Error)]
pub enum WhisperServiceError {
    #[error(
        "WhisperService already initialized with different library path: {existing} (requested: {requested})"
    )]
    LibraryPathMismatch { existing: PathBuf, requested: PathBuf },

    #[error("WhisperService global storage not initialized")]
    GlobalStorageNotInitialized,

    #[error("WhisperService instance not initialized")]
    InstanceNotInitialized,

    #[error("Lock poisoned while trying to {operation}")]
    LockPoisoned { operation: &'static str },

    #[error("Model path contains invalid UTF-8")]
    InvalidModelPathUtf8,

    #[error("Whisper context not initialized")]
    ContextNotInitialized,

    #[error("Failed to run whisper model inference")]
    InferenceFailed {
        #[source]
        source: anyhow::Error,
    },

    #[error("Failed to canonicalize whisper library path: {path}")]
    CanonicalizeLibraryPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to initialize whisper dynamic library at: {path}")]
    InitializeDynamicLibrary {
        path: PathBuf,
        #[source]
        source: anyhow::Error,
    },

    #[error("Failed to create whisper context with model: {model_path}")]
    CreateContext {
        model_path: String,
        #[source]
        source: anyhow::Error,
    },

    #[error("Failed to read audio data for whisper inference from: {path}")]
    ReadAudioData {
        path: PathBuf,
        #[source]
        source: anyhow::Error,
    },

    #[error("Failed to create whisper inference state")]
    CreateInferenceState {
        #[source]
        source: anyhow::Error,
    },
}

#[derive(Debug)]
pub struct WhisperService {
    instance: Arc<Whisper>,
    ctx: Arc<Mutex<Option<WhisperContext>>>,
}

// SAFETY: WhisperService is only accessed through Arc<Mutex<...>> for mutable state.
// The `instance: Arc<Whisper>` field wraps a dynamically loaded library handle which is
// immutable after creation (contexts and params are created from it, not mutated).
// All mutable inference state is guarded by the `ctx: Arc<Mutex<...>>` field.
unsafe impl Send for WhisperService {}
unsafe impl Sync for WhisperService {}

impl WhisperService {
    fn resolve_lib_path<P: AsRef<Path>>(path: P) -> Result<PathBuf, services::ServiceError> {
        let whisper_lib_name = format!("{}whisper{}", DLL_PREFIX, DLL_SUFFIX);

        let mut lib_path = path.as_ref().to_path_buf();
        if lib_path.file_name() != Some(OsStr::new(&whisper_lib_name)) {
            lib_path.push(&whisper_lib_name);
        }

        std::fs::canonicalize(&lib_path).map_err(|source| {
            WhisperServiceError::CanonicalizeLibraryPath {
                path: lib_path,
                source,
            }
            .into()
        })
    }

    fn build_service(normalized_path: &Path) -> Result<Self, services::ServiceError> {
        info!("current whisper path is: {}", normalized_path.display());
        let whisper = Whisper::new(normalized_path.to_path_buf()).map_err(|source| {
            WhisperServiceError::InitializeDynamicLibrary {
                path: normalized_path.to_path_buf(),
                source: source.into(),
            }
        })?;

        Ok(Self {
            instance: Arc::new(whisper),
            ctx: Arc::new(Mutex::new(None)),
        })
    }

    pub fn init<P: AsRef<Path>>(path: P) -> Result<Arc<Self>, services::ServiceError> {
        let normalized_path = Self::resolve_lib_path(path)?;
        let global_lock = INSTANCE.get_or_init(|| RwLock::new(None));

        {
            let read_guard = global_lock
                .read()
                .map_err(|_| WhisperServiceError::LockPoisoned {
                    operation: "read whisper global state",
                })?;
            if let Some(global) = read_guard.as_ref() {
                if global.lib_path != normalized_path {
                    return Err(WhisperServiceError::LibraryPathMismatch {
                        existing: global.lib_path.clone(),
                        requested: normalized_path.clone(),
                    }
                    .into());
                }
                return Ok(global.service.clone());
            }
        }

        let service = Arc::new(Self::build_service(&normalized_path)?);
        let mut write_guard = global_lock
            .write()
            .map_err(|_| WhisperServiceError::LockPoisoned {
                operation: "write whisper global state",
            })?;

        if let Some(global) = write_guard.as_ref() {
            if global.lib_path != normalized_path {
                return Err(WhisperServiceError::LibraryPathMismatch {
                    existing: global.lib_path.clone(),
                    requested: normalized_path.clone(),
                }
                .into());
            }
            return Ok(global.service.clone());
        }

        *write_guard = Some(WhisperGlobal {
            service: service.clone(),
            lib_path: normalized_path,
        });

        Ok(service)
    }

    pub fn reload<P: AsRef<Path>>(path: P) -> Result<Arc<Self>, services::ServiceError> {
        let normalized_path = Self::resolve_lib_path(path)?;
        let service = Arc::new(Self::build_service(&normalized_path)?);
        let global_lock = INSTANCE.get_or_init(|| RwLock::new(None));
        let mut write_guard = global_lock
            .write()
            .map_err(|_| WhisperServiceError::LockPoisoned {
                operation: "write whisper global state",
            })?;

        let previous = write_guard
            .as_ref()
            .map(|g| g.lib_path.display().to_string())
            .unwrap_or_else(|| "<uninitialized>".to_string());

        *write_guard = Some(WhisperGlobal {
            service: service.clone(),
            lib_path: normalized_path.clone(),
        });

        info!(
            "whisper service reloaded: {} -> {}",
            previous,
            normalized_path.display()
        );

        Ok(service)
    }

    pub fn current() -> Result<Arc<Self>, services::ServiceError> {
        let global_lock = INSTANCE
            .get()
            .ok_or(WhisperServiceError::GlobalStorageNotInitialized)?;
        let read_guard = global_lock
            .read()
            .map_err(|_| WhisperServiceError::LockPoisoned {
                operation: "read whisper global state",
            })?;
        read_guard
            .as_ref()
            .map(|global| global.service.clone())
            .ok_or(WhisperServiceError::InstanceNotInitialized.into())
    }

    pub fn new_context<P: AsRef<Path>>(
        &self,
        path_to_model: P,
        params: WhisperContextParameters,
    ) -> Result<(), services::ServiceError> {
        let mut ctx_lock = self
            .ctx
            .lock()
            .map_err(|_| WhisperServiceError::LockPoisoned {
                operation: "lock whisper context",
            })?;
        *ctx_lock = None;

        let path = path_to_model
            .as_ref()
            .to_str()
            .ok_or(WhisperServiceError::InvalidModelPathUtf8)?;

        let ctx = self
            .instance
            .new_context_with_params(path, params)
            .map_err(|source| WhisperServiceError::CreateContext {
                model_path: path.to_string(),
                source: source.into(),
            })?;
        *ctx_lock = Some(ctx);

        Ok(())
    }

    pub async fn inference<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<Vec<SubtitleEntry>, services::ServiceError> {
        let input_path = path.as_ref().to_path_buf();
        let audio_data = ffmpeg::FfmpegService::read_audio_data(&input_path)
            .await
            .map_err(|source| WhisperServiceError::ReadAudioData {
                path: input_path.clone(),
                source: source.into(),
            })?;

        let ctx_lock = self
            .ctx
            .lock()
            .map_err(|_| WhisperServiceError::LockPoisoned {
                operation: "lock whisper context",
            })?;

        let ctx = ctx_lock
            .as_ref()
            .ok_or(WhisperServiceError::ContextNotInitialized)?;

        let params = self.instance.new_full_params(SamplingStrategy::BeamSearch {
            beam_size: 5,
            patience: -1.0,
        });

        let mut state = ctx
            .create_state()
            .map_err(|source| WhisperServiceError::CreateInferenceState {
                source: source.into(),
            })?;
        state
            .full(params, &audio_data[..])
            .map_err(|source| WhisperServiceError::InferenceFailed {
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

    use super::*;
    use crate::services::dylib::DylibService;
    use crate::services::subtitle::SubtitleService;
    use hf_hub::api::sync::Api;
    use tokio;

    async fn ensure_whisper_dir() -> PathBuf {
        let mut test_data_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        test_data_path.push("../testdata");

        DylibService::new()
            .with_prefix_path(&test_data_path)
            .download_whisper()
            .await
            .expect("Failed to download whisper")
    }

    #[tokio::test]
    async fn test_whisper_current_and_reload() {
        let whisper_dir = ensure_whisper_dir().await;

        let initial = WhisperService::init(whisper_dir.as_path())
            .expect("failed to initialize whisper service");
        let current = WhisperService::current().expect("failed to get current whisper service");
        assert!(Arc::ptr_eq(&initial, &current));

        let reloaded = WhisperService::reload(whisper_dir.as_path())
            .expect("failed to reload whisper service");
        let current_after_reload =
            WhisperService::current().expect("failed to get current whisper service after reload");

        assert!(Arc::ptr_eq(&reloaded, &current_after_reload));
        assert!(!Arc::ptr_eq(&initial, &reloaded));
    }

    #[tokio::test]
    async fn test_whisper() {
        let mut test_data_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        test_data_path.push("../testdata");
        println!("Current executable path: {:?}", test_data_path);

        let path = ensure_whisper_dir().await;

        let ws = WhisperService::init(path.as_path()).expect("failed to initialize whisper service");

        let api = Api::new().expect("fail to init hf-api");
        let model_path = api
            .model("Aratako/anime-whisper-ggml".into())
            .get("ggml-anime-whisper.bin")
            .expect("error download model");

        let mut params = WhisperContextParameters::default();
        params.flash_attn(true).use_gpu(true);

        ws.new_context(model_path.as_path(), params)
            .expect("load model failed");
        let jfk_audio_path = test_data_path.join("samples/jfk.wav");
        let srt_entries = ws
            .inference(jfk_audio_path)
            .await
            .expect("Inference failed");

        let srt_services = SubtitleService::new();

        let file_path = test_data_path.join("whisper_test.srt");
        srt_services
            .to_srt_file(file_path, srt_entries)
            .expect("srt failed")
    }
}
