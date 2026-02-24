use crate::engine;
use slab_diffusion::{Diffusion, SdContextParams, SdImage, SdImgGenParams};
use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use thiserror::Error;
use tracing::info;

struct DiffusionGlobal {
    service: Arc<GGMLDiffusionEngine>,
    lib_path: PathBuf,
}

static INSTANCE: OnceLock<RwLock<Option<DiffusionGlobal>>> = OnceLock::new();

#[derive(Debug, Error)]
pub enum GGMLDiffusionEngineError {
    #[error(
        "GGMLDiffusionEngine already initialized with different library path: {existing} (requested: {requested})"
    )]
    LibraryPathMismatch {
        existing: PathBuf,
        requested: PathBuf,
    },

    #[allow(unused)]
    #[error("GGMLDiffusionEngine global storage not initialized")]
    GlobalStorageNotInitialized,

    #[allow(unused)]
    #[error("GGMLDiffusionEngine instance not initialized")]
    InstanceNotInitialized,

    #[error("Lock poisoned while trying to {operation}")]
    LockPoisoned { operation: &'static str },

    #[error("GGMLDiffusionEngine context not initialized")]
    ContextNotInitialized,

    #[error("Failed to canonicalize GGMLDiffusionEngine library path: {path}")]
    CanonicalizeLibraryPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to initialize GGMLDiffusionEngine dynamic library at: {path}")]
    InitializeDynamicLibrary {
        path: PathBuf,
        #[source]
        source: anyhow::Error,
    },

    #[error("Failed to create GGMLDiffusionEngine context")]
    CreateContext {
        #[source]
        source: anyhow::Error,
    },

    #[error("Failed to run GGMLDiffusionEngine image generation")]
    InferenceFailed {
        #[source]
        source: anyhow::Error,
    },
}

#[derive(Debug)]
pub struct GGMLDiffusionEngine {
    instance: Arc<Diffusion>,
    ctx: Arc<Mutex<Option<slab_diffusion::SdContext>>>,
}

// SAFETY: GGMLDiffusionEngine is only accessed through Arc<Mutex<...>> for mutable state.
// The `instance: Arc<Diffusion>` field wraps a dynamically loaded library handle which is
// immutable after creation (contexts are created from it, not mutated).
// All mutable inference state is guarded by the `ctx: Arc<Mutex<...>>` field.
// See: https://github.com/leejet/stable-diffusion.cpp (README / architecture)
unsafe impl Send for GGMLDiffusionEngine {}
unsafe impl Sync for GGMLDiffusionEngine {}

impl GGMLDiffusionEngine {
    fn resolve_lib_path<P: AsRef<Path>>(path: P) -> Result<PathBuf, engine::EngineError> {
        let sd_lib_name = format!("{}stable-diffusion{}", DLL_PREFIX, DLL_SUFFIX);

        let mut lib_path = path.as_ref().to_path_buf();
        if lib_path.file_name() != Some(OsStr::new(&sd_lib_name)) {
            lib_path.push(&sd_lib_name);
        }

        std::fs::canonicalize(&lib_path).map_err(|source| {
            GGMLDiffusionEngineError::CanonicalizeLibraryPath {
                path: lib_path,
                source,
            }
            .into()
        })
    }

    fn build_service(normalized_path: &Path) -> Result<Self, engine::EngineError> {
        info!("current diffusion path is: {}", normalized_path.display());
        let diffusion = Diffusion::new(normalized_path).map_err(|source| {
            GGMLDiffusionEngineError::InitializeDynamicLibrary {
                path: normalized_path.to_path_buf(),
                source: source.into(),
            }
        })?;

        Ok(Self {
            instance: Arc::new(diffusion),
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
            let read_guard =
                global_lock
                    .read()
                    .map_err(|_| GGMLDiffusionEngineError::LockPoisoned {
                        operation: "read diffusion global state",
                    })?;
            if let Some(global) = read_guard.as_ref() {
                if global.lib_path != normalized_path {
                    return Err(GGMLDiffusionEngineError::LibraryPathMismatch {
                        existing: global.lib_path.clone(),
                        requested: normalized_path.clone(),
                    }
                    .into());
                }
                return Ok(global.service.clone());
            }
        }

        let service = Arc::new(Self::build_service(&normalized_path)?);
        let mut write_guard =
            global_lock
                .write()
                .map_err(|_| GGMLDiffusionEngineError::LockPoisoned {
                    operation: "write diffusion global state",
                })?;

        if let Some(global) = write_guard.as_ref() {
            if global.lib_path != normalized_path {
                return Err(GGMLDiffusionEngineError::LibraryPathMismatch {
                    existing: global.lib_path.clone(),
                    requested: normalized_path.clone(),
                }
                .into());
            }
            return Ok(global.service.clone());
        }

        *write_guard = Some(DiffusionGlobal {
            service: service.clone(),
            lib_path: normalized_path,
        });

        Ok(service)
    }

    pub fn reload<P: AsRef<Path>>(path: P) -> Result<Arc<Self>, engine::EngineError> {
        let normalized_path = Self::resolve_lib_path(path)?;
        let service = Arc::new(Self::build_service(&normalized_path)?);
        let global_lock = INSTANCE.get_or_init(|| RwLock::new(None));
        let mut write_guard =
            global_lock
                .write()
                .map_err(|_| GGMLDiffusionEngineError::LockPoisoned {
                    operation: "write diffusion global state",
                })?;

        let previous = write_guard
            .as_ref()
            .map(|g| g.lib_path.display().to_string())
            .unwrap_or_else(|| "<uninitialized>".to_string());

        *write_guard = Some(DiffusionGlobal {
            service: service.clone(),
            lib_path: normalized_path.clone(),
        });

        info!(
            "diffusion service reloaded: {} -> {}",
            previous,
            normalized_path.display()
        );

        Ok(service)
    }

    pub fn current() -> Result<Arc<Self>, engine::EngineError> {
        let global_lock = INSTANCE
            .get()
            .ok_or(GGMLDiffusionEngineError::GlobalStorageNotInitialized)?;
        let read_guard =
            global_lock
                .read()
                .map_err(|_| GGMLDiffusionEngineError::LockPoisoned {
                    operation: "read diffusion global state",
                })?;
        read_guard
            .as_ref()
            .map(|global| global.service.clone())
            .ok_or(GGMLDiffusionEngineError::InstanceNotInitialized.into())
    }

    /// Create (or replace) the Stable Diffusion inference context.
    ///
    /// Loading the model files specified in `params` may take several seconds.
    pub fn new_context(&self, params: &SdContextParams) -> Result<(), engine::EngineError> {
        let mut ctx_lock = self
            .ctx
            .lock()
            .map_err(|_| GGMLDiffusionEngineError::LockPoisoned {
                operation: "lock diffusion context",
            })?;
        *ctx_lock = None;

        let ctx = self.instance.new_context(params).map_err(|source| {
            GGMLDiffusionEngineError::CreateContext {
                source: source.into(),
            }
        })?;
        *ctx_lock = Some(ctx);

        Ok(())
    }

    /// Generate one or more images from the supplied parameters.
    ///
    /// The returned `Vec` contains exactly `params.batch_count` images.
    pub fn generate_image(
        &self,
        params: &SdImgGenParams,
    ) -> Result<Vec<SdImage>, engine::EngineError> {
        let ctx_lock = self
            .ctx
            .lock()
            .map_err(|_| GGMLDiffusionEngineError::LockPoisoned {
                operation: "lock diffusion context",
            })?;

        let ctx = ctx_lock
            .as_ref()
            .ok_or(GGMLDiffusionEngineError::ContextNotInitialized)?;

        ctx.generate_image(params).map_err(|source| {
            GGMLDiffusionEngineError::InferenceFailed {
                source: source.into(),
            }
            .into()
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::path::PathBuf;
    use tokio;

    async fn ensure_diffusion_dir() -> PathBuf {
        let mut test_data_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        test_data_path.push("../testdata");
        test_data_path.join("diffusion")
    }

    #[tokio::test]
    async fn test_diffusion_current_and_reload() {
        let diffusion_dir = ensure_diffusion_dir().await;

        let initial = GGMLDiffusionEngine::init(diffusion_dir.as_path())
            .expect("failed to initialize diffusion service");
        let current =
            GGMLDiffusionEngine::current().expect("failed to get current diffusion service");
        assert!(Arc::ptr_eq(&initial, &current));

        let reloaded = GGMLDiffusionEngine::reload(diffusion_dir.as_path())
            .expect("failed to reload diffusion service");
        let current_after_reload = GGMLDiffusionEngine::current()
            .expect("failed to get current diffusion service after reload");

        assert!(Arc::ptr_eq(&reloaded, &current_after_reload));
        assert!(!Arc::ptr_eq(&initial, &reloaded));
    }

    #[tokio::test]
    async fn test_diffusion_generate_image() {
        let mut test_data_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        test_data_path.push("../testdata");

        let diffusion_dir = ensure_diffusion_dir().await;

        let ds = GGMLDiffusionEngine::init(diffusion_dir.as_path())
            .expect("failed to initialize diffusion service");

        // Use a tiny FLUX-dev GGUF for the test.  The test is skipped
        // (compilation-only) when the model cannot be obtained.
        let model_path = test_data_path.join("sd-models/flux1-schnell-q2_k.gguf");
        if !model_path.exists() {
            println!("skipping diffusion test: model not found at {model_path:?}");
            return;
        }

        let ctx_params = SdContextParams::with_model(model_path.to_str().unwrap());
        ds.new_context(&ctx_params)
            .expect("failed to create diffusion context");

        let gen_params = SdImgGenParams {
            prompt: "a lovely cat sitting on a roof".to_string(),
            width: 256,
            height: 256,
            sample_steps: 4,
            ..SdImgGenParams::default()
        };

        let images = ds
            .generate_image(&gen_params)
            .expect("generate_image failed");

        assert_eq!(images.len(), 1);
        assert!(!images[0].data.is_empty());

        let out = test_data_path.join("diffusion_test.png");
        println!("Generated image saved to {out:?}");
    }
}
