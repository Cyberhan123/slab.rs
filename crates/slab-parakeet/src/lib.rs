use std::fmt;
use std::path::Path;
use std::sync::Arc;

mod common_logging;
mod context;
mod context_params;
mod error;
mod full_params;
mod logging_hook;
mod standalone;
mod state;

pub use common_logging::GGMLLogLevel;
pub use context::ParakeetContext;
pub use context_params::ContextParams;
pub use error::ParakeetError;
pub use full_params::FullParams;
pub use state::{ParakeetSegment, ParakeetSegmentIterator, ParakeetState};

pub type ParakeetSysContext = slab_parakeet_sys::parakeet_context;
pub type ParakeetSysState = slab_parakeet_sys::parakeet_state;

pub type ParakeetTokenData = slab_parakeet_sys::parakeet_token_data;
pub type ParakeetTokenId = slab_parakeet_sys::parakeet_token;
pub type ParakeetNewSegmentCallback = slab_parakeet_sys::parakeet_new_segment_callback;
pub type ParakeetNewTokenCallback = slab_parakeet_sys::parakeet_new_token_callback;
pub type ParakeetProgressCallback = slab_parakeet_sys::parakeet_progress_callback;
pub type ParakeetEncoderBeginCallback = slab_parakeet_sys::parakeet_encoder_begin_callback;
pub type ParakeetAbortCallback = slab_parakeet_sys::ggml_abort_callback;
pub type ParakeetLogCallback = slab_parakeet_sys::ggml_log_callback;

use slab_ggml::GGML;
use slab_ggml::load_runtime_with_ggml_sidecar;

/// Safe handle to the loaded parakeet shared library.
///
/// Cloneable and cheap (an `Arc` to the loaded symbol table plus the ggml sidecar
/// that backs its compute backends). Create contexts via [`Parakeet::new_context`].
#[derive(Clone)]
pub struct Parakeet {
    lib: Arc<slab_parakeet_sys::ParakeetLib>,
    // Keep ggml.dll loaded when backend symbols are resolved from it.
    _ggml_lib: Option<Arc<GGML>>,
}

impl Parakeet {
    pub fn new<P: AsRef<Path>>(lib_dir: P) -> Result<Self, ParakeetError> {
        let (parakeet_lib, ggml_lib) =
            load_runtime_with_ggml_sidecar(lib_dir, "parakeet", load_parakeet_lib)?;

        let parakeet = Self { lib: Arc::new(parakeet_lib), _ggml_lib: ggml_lib };
        parakeet.install_logging_hooks();
        Ok(parakeet)
    }

    /// Redirect parakeet (and its ggml) logs to Rust logging hooks.
    ///
    /// Safe to call multiple times. Only has an effect the first time.
    pub fn install_logging_hooks(&self) {
        self.install_parakeet_logging_hook();
    }
}

impl fmt::Debug for Parakeet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Parakeet").finish()
    }
}

fn load_parakeet_lib(
    _lib_dir: &Path,
    path: &Path,
) -> Result<slab_parakeet_sys::ParakeetLib, libloading::Error> {
    unsafe {
        slab_parakeet_sys::ParakeetLib::from_library(slab_utils::loader::open_native_library(path)?)
    }
}
