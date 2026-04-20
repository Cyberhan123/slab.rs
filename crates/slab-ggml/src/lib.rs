mod backend;
mod error;
mod logging;

pub use backend::GGMLBackendDevice;
pub use backend::GGMLBackendReg;
pub use error::GGMLError;
pub use logging::GgmlLogLevel;
use slab_ggml_sys::GGmlLib;
use slab_utils::loader::{RuntimeLibrary, load_runtime_library_from_dir};
use std::ffi::CStr;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct GGML {
    pub(crate) lib: Arc<GGmlLib>,
}

/// Safety: `GGML` is thread-safe because it only contains an `Arc` to the underlying library,
/// which is immutable and can be safely shared across threads.
unsafe impl Send for GGML {}
unsafe impl Sync for GGML {}

impl GGML {
    /// Load the `ggml` shared library from `path`.
    ///
    /// # Errors
    /// Returns a [`GGMLError`] when the library cannot be opened or a
    /// required symbol is missing.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, GGMLError> {
        let path = path.as_ref();
        let lib_dir = path.parent().ok_or(GGMLError::NotParentDir)?;
        let lib = unsafe { <GGmlLib as RuntimeLibrary>::load_from_dir(lib_dir, path)? };
        Ok(Self { lib: Arc::new(lib) })
    }

    /// Load the `ggml` shared library from a unified runtime library directory.
    pub fn from_dir<P: AsRef<Path>>(lib_dir: P) -> Result<Self, GGMLError> {
        let ggml =
            load_runtime_library_from_dir::<_, Self>(lib_dir, "ggml").map_err(GGMLError::from)?;
        ggml.install_logging_hook();
        Ok(ggml)
    }

    /// Load the `ggml` shared library and eagerly register backend plugins from
    /// the same runtime directory.
    pub fn from_dir_and_load_backends<P: AsRef<Path>>(lib_dir: P) -> Result<Self, GGMLError> {
        let lib_dir = lib_dir.as_ref();
        let ggml = Self::from_dir(lib_dir)?;
        let lib_dir = lib_dir.to_string_lossy();
        ggml.load_all_backend_from_path(lib_dir.as_ref())?;
        Ok(ggml)
    }

    /// Load ggml after verifying the library directory exists.
    pub fn new_with<P: AsRef<Path>>(path: P) -> Result<Self, GGMLError> {
        let path = path.as_ref();
        let Some(lib_dir) = path.parent() else {
            return Err(GGMLError::NullPointer);
        };

        if !lib_dir.is_dir() {
            return Err(GGMLError::NullPointer);
        }

        Self::new(path)
    }

    /// Backward-compatible typo alias for `new_with`.
    pub fn new_wih<P: AsRef<Path>>(path: P) -> Result<Self, GGMLError> {
        Self::new_with(path)
    }

    pub fn version(&self) -> Result<&'static str, GGMLError> {
        let ptr = unsafe { self.lib.base.ggml_version() };
        if ptr.is_null() {
            return Err(GGMLError::NullPointer);
        }
        Ok(unsafe { CStr::from_ptr(ptr) }.to_str()?)
    }

    pub fn load_all_backend(&self) {
        unsafe { self.lib.loader.ggml_backend_load_all() };
    }

    pub fn load_all_backend_from_path(&self, path: &str) -> Result<(), GGMLError> {
        let c_path = std::ffi::CString::new(path)?;
        unsafe { self.lib.loader.ggml_backend_load_all_from_path(c_path.as_ptr()) };
        Ok(())
    }

    pub fn ggml_backend_load(&self, path: &str) -> Result<GGMLBackendReg, GGMLError> {
        let c_backend = std::ffi::CString::new(path)?;
        let reg = unsafe { self.lib.loader.ggml_backend_load(c_backend.as_ptr()) };

        if reg.is_null() {
            return Err(GGMLError::NullPointer);
        }

        Ok(GGMLBackendReg { reg })
    }

    pub fn ggml_backend_unload(&self, reg: GGMLBackendReg) {
        unsafe { self.lib.loader.ggml_backend_unload(reg.reg) };
    }

    pub fn ggml_backend_dev_count(&self) -> usize {
        unsafe { self.lib.loader.ggml_backend_dev_count() }
    }

    pub fn ggml_backend_dev_get(&self, index: usize) -> Result<GGMLBackendDevice, GGMLError> {
        let device = unsafe { self.lib.loader.ggml_backend_dev_get(index) };
        if device.is_null() {
            return Err(GGMLError::NullPointer);
        }
        Ok(GGMLBackendDevice { device })
    }

    pub fn ggml_backend_dev_name(&self, device: GGMLBackendDevice) -> Result<&str, GGMLError> {
        let name_ptr = unsafe { self.lib.base.ggml_backend_dev_name(device.device) };
        if name_ptr.is_null() {
            return Err(GGMLError::NullPointer);
        }
        Ok(unsafe { CStr::from_ptr(name_ptr) }.to_str()?)
    }
}

impl fmt::Debug for GGML {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GGML").finish()
    }
}

impl RuntimeLibrary for GGML {
    unsafe fn load_from_dir(lib_dir: &Path, path: &Path) -> Result<Self, libloading::Error> {
        let lib = unsafe { <GGmlLib as RuntimeLibrary>::load_from_dir(lib_dir, path)? };
        Ok(Self { lib: Arc::new(lib) })
    }
}

pub fn load_runtime_with_ggml_sidecar<P, Main>(
    lib_dir: P,
    main_name: &str,
) -> Result<(Main, Option<Arc<GGML>>), libloading::Error>
where
    P: AsRef<Path>,
    Main: RuntimeLibrary,
{
    let lib_dir = lib_dir.as_ref();
    let main = load_runtime_library_from_dir::<_, Main>(lib_dir, main_name)?;
    let ggml = GGML::from_dir_and_load_backends(lib_dir).ok().map(Arc::new);
    Ok((main, ggml))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_with_rejects_missing_parent_directory() {
        let missing = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("missing-runtime-dir")
            .join("ggml.dll");

        let error = GGML::new_with(&missing).unwrap_err();
        assert!(matches!(error, GGMLError::NullPointer));
    }

    #[test]
    fn new_wih_matches_new_with_for_missing_parent_directory() {
        let missing = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("missing-runtime-dir")
            .join("ggml.dll");

        let error = GGML::new_wih(&missing).unwrap_err();
        assert!(matches!(error, GGMLError::NullPointer));
    }
}
