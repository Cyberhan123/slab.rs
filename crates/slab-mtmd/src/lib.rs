//! Safe wrapper around the llama.cpp multimodal (`mtmd`) shared library.
//!
//! `mtmd` provides vision/audio projectors that interleave media embeddings
//! with text tokens for multimodal chat. This crate loads `mtmd.dll`/`.so` via
//! libloading and exposes a safe Rust API mirroring `llama-cpp-rs`'s mtmd
//! module. It depends on [`slab_llama`] for the text model/context handles,
//! which are passed across the `-sys` boundary as opaque pointers.

mod bitmap;
mod context;
mod error;
mod input;
mod params;

use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

use slab_ggml::GGML;
use slab_ggml::load_runtime_with_ggml_sidecar;

pub use bitmap::MtmdBitmap;
pub use context::MtmdContext;
pub use error::{MtmdError, Result};
pub use input::{MtmdInputChunkType, MtmdInputChunks, MtmdInputText};
pub use params::MtmdContextParams;

pub(crate) struct SharedMtmdLib(pub(crate) slab_mtmd_sys::MtmdLib);

// SAFETY: the loaded mtmd symbol table is treated as immutable and shared.
unsafe impl Send for SharedMtmdLib {}
// SAFETY: same as Send — symbol table is immutable once loaded.
unsafe impl Sync for SharedMtmdLib {}

impl Deref for SharedMtmdLib {
    type Target = slab_mtmd_sys::MtmdLib;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A handle to the dynamically-loaded `mtmd` shared library.
///
/// Cheap to clone; all clones share the same underlying [`Arc`]. Create a
/// [`MtmdContext`] from it with [`MtmdContext::init_from_file`].
#[derive(Clone)]
pub struct Mtmd {
    pub(crate) lib: Arc<SharedMtmdLib>,
    pub(crate) _ggml_lib: Option<Arc<GGML>>,
}

impl Mtmd {
    /// Load the `mtmd` shared library from the given runtime library directory.
    ///
    /// The ggml sidecar is loaded first so transitive symbol resolution works.
    ///
    /// # Errors
    /// Returns a [`libloading::Error`] when the library cannot be opened or a
    /// required symbol is missing.
    pub fn new<P: AsRef<Path>>(lib_dir: P) -> std::result::Result<Self, libloading::Error> {
        let (mtmd_lib, ggml) = load_runtime_with_ggml_sidecar(lib_dir, "mtmd", load_mtmd_lib)?;
        Ok(Self { lib: Arc::new(SharedMtmdLib(mtmd_lib)), _ggml_lib: ggml })
    }

    pub(crate) fn lib_arc(&self) -> Arc<SharedMtmdLib> {
        Arc::clone(&self.lib)
    }
}

impl std::fmt::Debug for Mtmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mtmd").finish()
    }
}

fn load_mtmd_lib(
    _lib_dir: &Path,
    path: &Path,
) -> std::result::Result<slab_mtmd_sys::MtmdLib, libloading::Error> {
    #[cfg(windows)]
    {
        unsafe {
            slab_mtmd_sys::MtmdLib::from_library(slab_utils::loader::open_native_library(path)?)
        }
    }
    #[cfg(not(windows))]
    {
        unsafe { slab_mtmd_sys::MtmdLib::new(path) }
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

    static DLL_DIRS_INIT: OnceLock<std::result::Result<(), String>> = OnceLock::new();

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace root should resolve")
    }

    fn vendored_runtime_dir(artifact: &str) -> PathBuf {
        // mtmd ships inside the llama SDK, so its runtime DLL lives under
        // vendor/llama/bin (alongside llama.dll). The ggml sidecar is under
        // vendor/ggml/bin.
        let sub = if artifact == "ggml" { "ggml" } else { "llama" };
        workspace_root().join("vendor").join(sub).join("bin")
    }

    fn add_dll_directory(path: &Path) -> std::result::Result<(), String> {
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
            add_dll_directory(&vendored_runtime_dir("llama"))?;
            add_dll_directory(&vendored_runtime_dir("ggml"))?;
            Ok(())
        });
        if let Err(error) = init {
            panic!("failed to register vendored runtime directories: {error}");
        }
    }

    fn load_vendored_mtmd() -> Mtmd {
        ensure_vendored_runtime_dirs_registered();
        Mtmd::new(vendored_runtime_dir("llama"))
            .unwrap_or_else(|error| panic!("failed to load vendored mtmd runtime: {error}"))
    }

    #[test]
    fn vendored_ffi_loads() {
        // Loads mtmd.dll and resolves its symbol table; failure here means the
        // allowlist/bindgen emitted a symbol mtmd.dll does not export.
        let _mtmd = load_vendored_mtmd();
    }

    #[test]
    fn bitmap_from_rgb_round_trips() {
        let mtmd = load_vendored_mtmd();
        let (nx, ny) = (4u32, 2u32);
        let rgb: Vec<u8> = (0..(nx * ny * 3)).map(|i| (i % 256) as u8).collect();
        let bitmap = MtmdBitmap::from_rgb(&mtmd, nx, ny, &rgb)
            .expect("mtmd_bitmap_init should succeed for valid RGB");
        assert_eq!(bitmap.nx(), nx);
        assert_eq!(bitmap.ny(), ny);
        assert_eq!(bitmap.n_bytes(), (nx * ny * 3) as usize);
        assert!(!bitmap.is_audio());
    }

    #[test]
    fn bitmap_from_rgb_rejects_short_buffer() {
        let mtmd = load_vendored_mtmd();
        let result = MtmdBitmap::from_rgb(&mtmd, 4, 4, &[0u8; 1]);
        assert!(result.is_err());
    }

    #[test]
    fn input_chunks_allocate_and_count() {
        let mtmd = load_vendored_mtmd();
        let chunks = MtmdInputChunks::new(&mtmd);
        assert!(chunks.is_empty());
        assert_eq!(chunks.len(), 0);
    }
}
