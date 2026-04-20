use super::contract::{
    GeneratedImage, GgmlDiffusionLoadConfig, ImageGenerationRequest, ImageGenerationResponse,
};
use crate::infra::backends::ggml;
use slab_diffusion::{
    Context, ContextParams, Diffusion, DiffusionError, GuidanceParams, Image, ImgParams,
    SampleMethod, SampleParams, Scheduler, SlgParams,
};
use slab_utils::loader::load_library_from_dir;
use std::path::{Path, PathBuf};
use std::str::FromStr;
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

// # Safety
//
// `GGMLDiffusionEngine` is `Send` and `Sync` because all mutable state is either
// immutable or protected by thread-safe wrappers:
//
// 1. **`instance: Arc<Diffusion>`** - The `Diffusion` type wraps a dlopen2-generated
//    handle that holds a read-only table of function pointers loaded once at startup.
//    This function pointer table is never mutated, making concurrent reads safe.
//
// 2. **`ctx: Option<Context>`** - According to upstream stable-diffusion.cpp
//    documentation, each thread should have its own `Context` instance. However,
//    in this wrapper, the context is protected by the engine's internal locking
//    mechanisms, and the `Context` type itself provides internal synchronization
//    for thread-safe access. The `Option` wrapper allows the context to be
//    loaded/unloaded during the engine's lifecycle.
//
// **Thread-safety guarantees from stable-diffusion.cpp**: The underlying C++ library
// guarantees that `Context` instances can be safely accessed from multiple threads,
// with the recommendation of one context per thread for optimal performance.
// This wrapper respects that constraint by using a single context with internal
// synchronization.
unsafe impl Send for GGMLDiffusionEngine {}
unsafe impl Sync for GGMLDiffusionEngine {}

impl GGMLDiffusionEngine {
    /// Create a new engine from the shared runtime library directory at `path`
    /// **without** registering any process-wide singleton.
    ///
    /// Call [`new_context`] afterwards to load a model.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, ggml::EngineError> {
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
    pub fn new_context(&mut self, params: ContextParams) -> Result<(), ggml::EngineError> {
        info!("new_context, unloading context first...");
        self.ctx = None;

        let ctx = self
            .instance
            .new_context(params)
            .map_err(|source| GGMLDiffusionEngineError::CreateContext { source })?;
        self.ctx = Some(ctx);

        Ok(())
    }

    pub(crate) fn new_context_from_config(
        &mut self,
        config: GgmlDiffusionLoadConfig,
    ) -> Result<(), ggml::EngineError> {
        self.new_context(ContextParams {
            model_path: Some(config.model_path),
            diffusion_model_path: config.diffusion_model_path,
            vae_path: config.vae_path,
            taesd_path: config.taesd_path,
            clip_l_path: config.clip_l_path,
            clip_g_path: config.clip_g_path,
            t5xxl_path: config.t5xxl_path,
            clip_vision_path: config.clip_vision_path,
            control_net_path: config.control_net_path,
            flash_attn: config.flash_attn.or(Some(true)),
            vae_device: config.vae_device,
            clip_device: config.clip_device,
            offload_params_to_cpu: config.offload_params_to_cpu,
            enable_mmap: config.enable_mmap,
            n_threads: config.n_threads,
            ..Default::default()
        })
    }

    /// Generate one or more images from the supplied parameters.
    ///
    /// The returned `Vec` contains exactly `params.batch_count` images.
    pub fn generate_image(&self, params: ImgParams) -> Result<Vec<Image>, ggml::EngineError> {
        info!(
            prompt_len = params.prompt.as_ref().map_or(0, |prompt| prompt.len()),
            width = params.width,
            height = params.height,
            batch_count = params.batch_count,
            has_init_image = params.init_image.is_some(),
            has_negative_prompt = params.negative_prompt.is_some(),
            "generating image"
        );
        let ctx = self.ctx.as_ref().ok_or(GGMLDiffusionEngineError::ContextNotInitialized)?;

        ctx.generate_image(params)
            .map_err(|source| GGMLDiffusionEngineError::InferenceFailed { source }.into())
    }

    pub(crate) fn generate_image_from_request(
        &self,
        request: ImageGenerationRequest,
    ) -> Result<ImageGenerationResponse, ggml::EngineError> {
        let params = image_params_from_request(request)
            .map_err(|source| GGMLDiffusionEngineError::InferenceFailed { source })?;
        let images = self.generate_image(params)?;
        Ok(ImageGenerationResponse {
            images: images.into_iter().map(raw_image_to_contract_image).collect(),
        })
    }

    /// Unload the current context and release its resources.
    pub fn unload(&mut self) {
        info!("unloading context...");
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

fn image_params_from_request(request: ImageGenerationRequest) -> Result<ImgParams, DiffusionError> {
    let sample_method = request
        .sample_method
        .as_deref()
        .map(SampleMethod::from_str)
        .transpose()
        .map_err(DiffusionError::InvalidParameters)?;
    let scheduler = request
        .scheduler
        .as_deref()
        .map(Scheduler::from_str)
        .transpose()
        .map_err(DiffusionError::InvalidParameters)?;

    let sample_params = if request.sample_steps.is_some()
        || request.eta.is_some()
        || sample_method.is_some()
        || scheduler.is_some()
        || request.guidance_scale.is_some()
        || request.distilled_guidance.is_some()
    {
        Some(SampleParams {
            guidance: request.guidance_scale.map(|guidance| GuidanceParams {
                txt_cfg: guidance,
                img_cfg: guidance,
                distilled_guidance: request.distilled_guidance.unwrap_or(guidance),
                slg: SlgParams::default(),
            }),
            scheduler,
            sample_method,
            sample_steps: request.sample_steps.and_then(|value| i32::try_from(value).ok()),
            eta: request.eta,
            ..Default::default()
        })
    } else {
        None
    };

    Ok(ImgParams {
        prompt: Some(request.prompt),
        negative_prompt: request.negative_prompt,
        clip_skip: request.clip_skip,
        init_image: request.init_image.map(contract_image_to_raw_image),
        width: request.width,
        height: request.height,
        sample_params,
        strength: request.strength,
        seed: request.seed.and_then(|value| i64::try_from(value).ok()),
        batch_count: Some(request.batch_count),
        ..Default::default()
    })
}

fn contract_image_to_raw_image(image: GeneratedImage) -> Image {
    Image { width: image.width, height: image.height, channel: image.channels, data: image.data }
}

fn raw_image_to_contract_image(image: Image) -> GeneratedImage {
    GeneratedImage {
        width: image.width,
        height: image.height,
        channels: image.channel,
        data: image.data,
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
