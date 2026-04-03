use crate::internal::engine;
use slab_diffusion::{Context, ContextParams, Diffusion, DiffusionError, Image, ImgParams};
use slab_utils::loader::load_library_from_dir;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum GGMLDiffusionEngineError {
    #[error("GGMLDiffusionEngine context not initialized")]
    ContextNotInitialized,

    #[error("Failed to initialize GGMLDiffusionEngine dynamic library at: {path}")]
    InitializeDynamicLibrary {
        path: PathBuf,
        #[source]
        source: libloading::Error,
    },

    #[error("Failed to create GGMLDiffusionEngine context")]
    CreateContext {
        #[source]
        source: DiffusionError,
    },

    #[error("Failed to run GGMLDiffusionEngine image generation")]
    InferenceFailed {
        #[source]
        source: DiffusionError,
    },
}

/// Engine wrapping a Stable Diffusion shared library handle.
///
/// Each instance owns its own model context (`ctx`).  There is no shared
/// mutable state between separate `GGMLDiffusionEngine` instances, so no
/// `Mutex` is needed.  The backend worker owns the engine exclusively and
/// mutates it via `&mut self`.
#[derive(Debug)]
pub struct GGMLDiffusionEngine {
    instance: Arc<Diffusion>,
    // Owned per-engine context; not shared across instances.
    ctx: Option<Context>,
}

// SAFETY: GGMLDiffusionEngine is owned exclusively by its worker task.
// `instance: Arc<Diffusion>` is an immutable library handle safe to move
// between threads.  `ctx: Option<Context>` implements Send + Sync per
// the upstream stable-diffusion.cpp documentation (one context per thread).
unsafe impl Send for GGMLDiffusionEngine {}
unsafe impl Sync for GGMLDiffusionEngine {}

impl GGMLDiffusionEngine {
    /// Create a new engine from the shared runtime library directory at `path`
    /// **without** registering any process-wide singleton.
    ///
    /// Call [`new_context`] afterwards to load a model.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, engine::EngineError> {
        load_library_from_dir(path, "stable-diffusion", |lib_dir, diffusion_path| {
            info!("current diffusion path is: {}", diffusion_path.display());
            let diffusion = Diffusion::new(lib_dir).map_err(|source| {
                GGMLDiffusionEngineError::InitializeDynamicLibrary {
                    path: diffusion_path.to_path_buf(),
                    source,
                }
            })?;

            Ok(Self { instance: Arc::new(diffusion), ctx: None })
        })
    }

    /// Create (or replace) the Stable Diffusion inference context.
    ///
    /// Loading the model files specified in `params` may take several seconds.
    pub fn new_context(&mut self, params: ContextParams) -> Result<(), engine::EngineError> {
        self.ctx = None;

        let ctx = self
            .instance
            .new_context(params)
            .map_err(|source| GGMLDiffusionEngineError::CreateContext { source })?;
        self.ctx = Some(ctx);

        Ok(())
    }

    /// Generate one or more images from the supplied parameters.
    ///
    /// The returned `Vec` contains exactly `params.batch_count` images.
    pub fn generate_image(&self, params: ImgParams) -> Result<Vec<Image>, engine::EngineError> {
        let ctx = self.ctx.as_ref().ok_or(GGMLDiffusionEngineError::ContextNotInitialized)?;

        ctx.generate_image(params)
            .map_err(|source| GGMLDiffusionEngineError::InferenceFailed { source }.into())
    }

    /// Unload the current context and release its resources.
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
    #[ignore = "requires local diffusion test artifacts"]
    async fn test_diffusion_generate_image() {
        let mut test_data_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        test_data_path.push("../testdata");

        let diffusion_dir = ensure_diffusion_dir().await;

        let mut ds = GGMLDiffusionEngine::from_path(diffusion_dir.as_path())
            .expect("failed to initialize diffusion service");

        // Use a tiny FLUX-dev GGUF for the test.  The test is skipped
        // (compilation-only) when the model cannot be obtained.
        let model_path = test_data_path.join("sd-models/flux1-schnell-q2_k.gguf");
        if !model_path.exists() {
            println!("skipping diffusion test: model not found at {model_path:?}");
            return;
        }

        let ctx_params =
            ContextParams { model_path: Some(model_path.clone()), ..Default::default() };
        ds.new_context(ctx_params).expect("failed to create diffusion context");

        let sample_params =
            slab_diffusion::SampleParams { sample_steps: Some(4), ..Default::default() };

        let image_params = ImgParams {
            prompt: Some("a lovely cat sitting on a roof".to_owned()),
            width: Some(256),
            height: Some(256),
            sample_params: Some(sample_params),
            ..Default::default()
        };

        let images = ds.generate_image(image_params).expect("generate_image failed");

        assert_eq!(images.len(), 1);
        assert!(!images[0].data.is_empty());

        let out = test_data_path.join("diffusion_test.png");
        println!("Generated image saved to {out:?}");
    }
}
