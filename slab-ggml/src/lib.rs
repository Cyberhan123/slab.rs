mod backend;
mod error;
pub use backend::GGMLBackendReg;
pub use error::GGMLError;
use slab_ggml_sys;
use std::ffi::CStr;
use std::fmt::{self, Result};
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct GGML {
    pub(crate) lib: Arc<slab_ggml_sys::GGmlLib>,
}

impl GGML {
    /// Load the `ggml` shared library from `path`.
    ///
    /// # Errors
    /// Returns a [`GGMLError`] when the library cannot be opened or a
    /// required symbol is missing.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, GGMLError> {
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
            let ggml_lib = unsafe { slab_ggml_sys::GGmlLib::from_library(lib)? };
            Ok(Self { lib: Arc::new(ggml_lib) })
        }

        #[cfg(not(windows))]
        {
            let lib = unsafe { slab_ggml_sys::GGmlLib::new(path.as_ref())? };
            Ok(Self { lib: Arc::new(lib) })
        }
    }

    pub fn version(&self) -> Result<&'static str, GGMLError> {
        let ptr = unsafe { self.lib.ggml_version() };
        if ptr.is_null() {
            return Err(GGMLError::NullPointer());
        }
        Ok(unsafe { CStr::from_ptr(ptr) }.to_str()?)
    }

    pub fn load_all_backend(&self) {
        unsafe { self.lib.ggml_backend_load_all() };
    }

    pub fn load_all_backend_from_path(&self, path: &str) -> Result<(), GGMLError> {
        let c_path = std::ffi::CString::new(path)?;
        unsafe { self.lib.ggml_backend_load_all_from_path(c_path.as_ptr()) };
        Ok(())
    }

    pub fn ggml_backend_load(&self, path: &str) -> Result<GGMLBackendReg, GGMLError> {
        let c_backend = std::ffi::CString::new(path)?;
        let reg = unsafe { self.lib.ggml_backend_load(c_backend.as_ptr()) };

        if reg.is_null() {
            return Err(GGMLError::NullPointer);
        }

        Ok(GGMLBackendReg { reg: Arc::new(reg) })
    }
}

impl fmt::Debug for GGML {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GGML").finish()
    }
}
