mod context;
mod error;
mod logging;
mod params;
mod upscaler;

use crate::params::InnerContextParams;
use slab_ggml::GGML;
use slab_ggml::load_runtime_with_ggml_sidecar;
use std::ffi::CStr;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

pub use context::Context;
pub use error::DiffusionError;
pub use logging::DiffusionLogLevel;
pub use params::*;
pub use upscaler::UpscalerContext;

/// A handle to the dynamically-loaded `stable-diffusion` shared library.
///
/// Cheap to clone; all clones share the same underlying [`Arc`].
///
/// # Example
/// ```no_run
/// use std::path::PathBuf;
///
/// use slab_diffusion::{ContextParams, Diffusion, ImgParams, SampleMethod, SampleParams};
///
/// let sd = Diffusion::new("/usr/lib").unwrap();
/// let ctx = sd
///     .new_context(ContextParams {
///         model_path: Some(PathBuf::from("model.gguf")),
///         ..Default::default()
///     })
///     .unwrap();
/// let images = ctx
///     .generate_image(ImgParams {
///         prompt: Some("A lovely cat".to_owned()),
///         width: Some(256),
///         height: Some(256),
///         sample_params: Some(SampleParams {
///             sample_steps: Some(15),
///             sample_method: Some(SampleMethod::DPM2),
///             ..Default::default()
///         }),
///         ..Default::default()
///     })
///     .unwrap();
/// println!("generated {} image(s)", images.len());
/// ```
#[derive(Clone)]
pub struct Diffusion {
    pub(crate) lib: Arc<slab_diffusion_sys::DiffusionLib>,
    pub(crate) _ggml_lib: Option<Arc<GGML>>,
}

impl Diffusion {
    /// Load the `stable-diffusion` shared library from the given runtime library directory.
    ///
    /// # Errors
    /// Returns a [`libloading::Error`] when the library cannot be opened or a
    /// required symbol is missing.
    pub fn new<P: AsRef<Path>>(lib_dir: P) -> Result<Self, ::libloading::Error> {
        let (diffusion_lib, ggml) = load_runtime_with_ggml_sidecar::<
            _,
            slab_diffusion_sys::DiffusionLib,
        >(lib_dir, "stable-diffusion")?;

        let diffusion = Self { lib: Arc::new(diffusion_lib), _ggml_lib: ggml };
        diffusion.install_logging_hook();
        Ok(diffusion)
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

    /// Create a new [`Context`] from the given parameters.
    ///
    /// Loading the model files may take several seconds.
    ///
    /// # Errors
    /// Returns [`DiffusionError::ContextCreationFailed`] when the native
    /// `new_sd_ctx` call returns a null pointer (e.g. invalid model path).
    pub fn new_context(&self, params: ContextParams) -> Result<Context, DiffusionError> {
        let inner = InnerContextParams::from_canonical(self.lib.as_ref(), &params);
        let ctx = unsafe { self.lib.new_sd_ctx(&*inner.fp) };
        if ctx.is_null() {
            return Err(DiffusionError::ContextCreationFailed);
        }
        Ok(Context { ctx, lib: self.lib.clone(), _params: params })
    }

    pub fn backend_list_size(&self) -> Result<usize, DiffusionError> {
        let size = unsafe { self.lib.backend_list_size() };
        if size == 0 {
            return Err(DiffusionError::BackendListUnavailable);
        }
        Ok(size)
    }
}

impl fmt::Debug for Diffusion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Diffusion").finish()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::sync::OnceLock;

    type DllDirectoryCookie = *mut std::ffi::c_void;

    unsafe extern "system" {
        fn AddDllDirectory(new_directory: *const u16) -> DllDirectoryCookie;
    }

    static DLL_DIRS_INIT: OnceLock<Result<(), String>> = OnceLock::new();

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace root should resolve")
    }

    fn vendored_runtime_dir(artifact: &str) -> PathBuf {
        workspace_root().join("vendor").join(artifact).join("bin")
    }

    fn add_dll_directory(path: &Path) -> Result<(), String> {
        if !path.is_dir() {
            return Err(format!("runtime directory does not exist: {}", path.display()));
        }

        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        wide.push(0);

        let cookie = unsafe { AddDllDirectory(wide.as_ptr()) };
        if cookie.is_null() {
            return Err(format!("AddDllDirectory failed for {}", path.display()));
        }

        Ok(())
    }

    fn ensure_vendored_runtime_dirs_registered() {
        let init = DLL_DIRS_INIT.get_or_init(|| {
            add_dll_directory(&vendored_runtime_dir("diffusion"))?;
            add_dll_directory(&vendored_runtime_dir("ggml"))?;
            Ok(())
        });

        if let Err(error) = init {
            panic!("failed to register vendored runtime directories: {error}");
        }
    }

    fn load_vendored_diffusion() -> Diffusion {
        ensure_vendored_runtime_dirs_registered();

        Diffusion::new(vendored_runtime_dir("diffusion"))
            .unwrap_or_else(|error| panic!("failed to load vendored diffusion runtime: {error}"))
    }

    #[test]
    fn vendored_ffi_loads_and_reports_metadata() {
        let diffusion = load_vendored_diffusion();

        assert!(diffusion.get_num_physical_cores() >= 1);
        let _ = diffusion.get_system_info();
        assert!(
            !diffusion.get_version().trim().is_empty() || !diffusion.get_commit().trim().is_empty()
        );
    }

    #[test]
    fn vendored_ffi_serializes_param_structs() {
        let diffusion = load_vendored_diffusion();

        let context_params = ContextParams {
            model_path: Some(PathBuf::from("missing-model.gguf")),
            ..Default::default()
        };
        assert!(
            diffusion
                .context_params_to_str(&context_params)
                .is_some_and(|text| !text.trim().is_empty())
        );

        let sample_params = SampleParams { sample_steps: Some(8), ..Default::default() };
        assert!(
            diffusion
                .sample_params_to_str(&sample_params)
                .is_some_and(|text| !text.trim().is_empty())
        );

        let image_params =
            ImgParams { prompt: Some("test prompt".to_owned()), ..Default::default() };
        assert!(
            diffusion
                .image_params_to_str(&image_params)
                .is_some_and(|text| !text.trim().is_empty())
        );
    }

    #[test]
    fn vendored_ffi_reports_upscaler_creation_failure_for_missing_model() {
        let diffusion = load_vendored_diffusion();
        let result = diffusion.new_upscaler_context(
            "definitely-missing-upscaler-model.pth",
            false,
            false,
            1,
            64,
            None,
        );

        match result {
            Ok(_) => panic!("expected missing upscaler model to fail"),
            Err(error) => assert!(matches!(error, DiffusionError::ContextCreationFailed)),
        }
    }
}
