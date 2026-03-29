use std::ffi::CStr;
use std::fmt;
use std::path::Path;
use std::ptr;
use std::sync::Arc;

mod context;
mod error;
mod params;

pub use context::SdContext;
pub use error::DiffusionError;
pub use params::*;

/// A handle to the dynamically-loaded `stable-diffusion` shared library.
///
/// Cheap to clone; all clones share the same underlying [`Arc`].
///
/// # Example
/// ```no_run
/// use slab_diffusion::{Diffusion, SdContextParams, SdImgGenParams};
///
/// let sd = Diffusion::new("/usr/lib/libstable-diffusion.so").unwrap();
/// let ctx = sd.new_context(
///     &SdContextParams::with_model("/models/my-model.gguf")
/// ).unwrap();
/// let images = ctx.generate_image(
///     &SdImgGenParams::with_prompt("a lovely cat sitting on a roof")
/// ).unwrap();
/// println!("generated {} image(s)", images.len());
/// ```
#[derive(Clone)]
pub struct Diffusion {
    pub(crate) lib: Arc<slab_diffusion_sys::DiffusionLib>,
}

impl Diffusion {
    /// Load the `stable-diffusion` shared library from `path`.
    ///
    /// # Errors
    /// Returns a [`libloading::Error`] when the library cannot be opened or a
    /// required symbol is missing.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, ::libloading::Error> {
        #[cfg(windows)]
        {
            use libloading::os::windows::{
                Library, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
            };
            let lib = unsafe {
                Library::load_with_flags(
                    path.as_ref(),
                    LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
                )?
            };
            let diffusion_lib = unsafe { slab_diffusion_sys::DiffusionLib::from_library(lib)? };
            Ok(Self { lib: Arc::new(diffusion_lib) })
        }

        #[cfg(not(windows))]
        {
            let lib = unsafe { slab_diffusion_sys::DiffusionLib::new(path.as_ref())? };
            Ok(Self { lib: Arc::new(lib) })
        }
    }

    /// Return a string describing the capabilities of the loaded build
    /// (e.g. which backends are compiled in).
    pub fn get_system_info(&self) -> &'static str {
        let ptr = unsafe { self.lib.sd_get_system_info() };
        if ptr.is_null() {
            return "";
        }
        unsafe { CStr::from_ptr(ptr) }.to_str().unwrap_or("")
    }

    /// Return the number of physical CPU cores available.
    pub fn get_num_physical_cores(&self) -> i32 {
        unsafe { self.lib.sd_get_num_physical_cores() }
    }

    /// Return the stable-diffusion.cpp commit hash baked into the library.
    pub fn get_commit(&self) -> &'static str {
        let ptr = unsafe { self.lib.sd_commit() };
        if ptr.is_null() {
            return "";
        }
        unsafe { CStr::from_ptr(ptr) }.to_str().unwrap_or("")
    }

    /// Return the stable-diffusion.cpp version string.
    pub fn get_version(&self) -> &'static str {
        let ptr = unsafe { self.lib.sd_version() };
        if ptr.is_null() {
            return "";
        }
        unsafe { CStr::from_ptr(ptr) }.to_str().unwrap_or("")
    }

    /// Set a callback that receives log messages from the native library.
    ///
    /// # Safety
    /// The callback must be safe to call from C (no unwinding, etc.).
    pub unsafe fn set_log_callback(
        &self,
        cb: slab_diffusion_sys::sd_log_cb_t,
        data: *mut std::ffi::c_void,
    ) {
        unsafe { self.lib.sd_set_log_callback(cb, data) };
    }

    /// Set a callback that receives denoising-step progress updates.
    ///
    /// # Safety
    /// The callback must be safe to call from C.
    pub unsafe fn set_progress_callback(
        &self,
        cb: slab_diffusion_sys::sd_progress_cb_t,
        data: *mut std::ffi::c_void,
    ) {
        unsafe { self.lib.sd_set_progress_callback(cb, data) };
    }

    /// Create a new [`SdContext`] from the given parameters.
    ///
    /// Loading the model files may take several seconds.
    ///
    /// # Errors
    /// Returns [`DiffusionError::ContextCreationFailed`] when the native
    /// `new_sd_ctx` call returns a null pointer (e.g. invalid model path).
    pub fn new_context(&self, params: &SdContextParams) -> Result<SdContext, DiffusionError> {
        let model_cs = params::opt_cstring(&params.model_path);
        let diffusion_model_cs = params::opt_cstring(&params.diffusion_model_path);
        let clip_l_cs = params::opt_cstring(&params.clip_l_path);
        let clip_g_cs = params::opt_cstring(&params.clip_g_path);
        let t5xxl_cs = params::opt_cstring(&params.t5xxl_path);
        let llm_cs = params::opt_cstring(&params.llm_path);
        let llm_vision_cs = params::opt_cstring(&params.llm_vision_path);
        let clip_vision_cs = params::opt_cstring(&params.clip_vision_path);
        let high_noise_cs = params::opt_cstring(&params.high_noise_diffusion_model_path);
        let vae_cs = params::opt_cstring(&params.vae_path);
        let taesd_cs = params::opt_cstring(&params.taesd_path);
        let control_net_cs = params::opt_cstring(&params.control_net_path);
        let photo_maker_cs = params::opt_cstring(&params.photo_maker_path);

        let mut c_params: slab_diffusion_sys::sd_ctx_params_t = unsafe { std::mem::zeroed() };
        unsafe { self.lib.sd_ctx_params_init(&mut c_params) };
        c_params.model_path = params::ptr_or_null(&model_cs);
        c_params.diffusion_model_path = params::ptr_or_null(&diffusion_model_cs);
        c_params.clip_l_path = params::ptr_or_null(&clip_l_cs);
        c_params.clip_g_path = params::ptr_or_null(&clip_g_cs);
        c_params.t5xxl_path = params::ptr_or_null(&t5xxl_cs);
        c_params.llm_path = params::ptr_or_null(&llm_cs);
        c_params.llm_vision_path = params::ptr_or_null(&llm_vision_cs);
        c_params.clip_vision_path = params::ptr_or_null(&clip_vision_cs);
        c_params.high_noise_diffusion_model_path = params::ptr_or_null(&high_noise_cs);
        c_params.vae_path = params::ptr_or_null(&vae_cs);
        c_params.taesd_path = params::ptr_or_null(&taesd_cs);
        c_params.control_net_path = params::ptr_or_null(&control_net_cs);
        c_params.photo_maker_path = params::ptr_or_null(&photo_maker_cs);
        c_params.embeddings = ptr::null();
        c_params.embedding_count = 0;
        c_params.tensor_type_rules = ptr::null();
        c_params.n_threads = params.n_threads;
        c_params.wtype = params.weight_type;
        c_params.rng_type = params.rng_type;
        c_params.sampler_rng_type = slab_diffusion_sys::rng_type_t_RNG_TYPE_COUNT;
        c_params.prediction = params.prediction;
        c_params.lora_apply_mode = params.lora_apply_mode;
        c_params.offload_params_to_cpu = params.offload_params_to_cpu;
        c_params.enable_mmap = params.enable_mmap;
        c_params.keep_clip_on_cpu = params.keep_clip_on_cpu;
        c_params.keep_control_net_on_cpu = params.keep_control_net_on_cpu;
        c_params.keep_vae_on_cpu = params.keep_vae_on_cpu;
        c_params.vae_decode_only = params.vae_decode_only;
        c_params.free_params_immediately = false;
        c_params.tae_preview_only = params.taesd_preview_only;
        c_params.flash_attn = params.flash_attn;
        c_params.diffusion_flash_attn = params.diffusion_flash_attn;
        c_params.diffusion_conv_direct = false;
        c_params.vae_conv_direct = false;
        c_params.circular_x = false;
        c_params.circular_y = false;
        c_params.force_sdxl_vae_conv_scale = false;
        c_params.chroma_use_dit_mask = true;
        c_params.chroma_use_t5_mask = false;
        c_params.chroma_t5_mask_pad = 1;
        c_params.qwen_image_zero_cond_t = false;

        let ctx = unsafe { self.lib.new_sd_ctx(&c_params) };
        if ctx.is_null() {
            return Err(DiffusionError::ContextCreationFailed);
        }

        Ok(SdContext { ctx, lib: self.lib.clone() })
    }
}

impl fmt::Debug for Diffusion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Diffusion").finish()
    }
}
