use std::fmt;
use std::path::Path;
use std::sync::Arc;

mod common_logging;
mod error;
mod ggml_logging_hook;
mod standalone;
mod utilities;
mod whisper_ctx;
mod whisper_ctx_wrapper;
mod whisper_grammar;
mod whisper_logging_hook;
mod whisper_params;
mod whisper_state;
mod whisper_vad;

pub use common_logging::GGMLLogLevel;
pub use error::WhisperError;
pub use utilities::*;
pub use whisper_ctx::DtwMode;
pub use whisper_ctx::DtwModelPreset;
pub use whisper_ctx::DtwParameters;
pub use whisper_ctx::WhisperContextParameters;

pub use whisper_ctx_wrapper::WhisperContext;
pub use whisper_grammar::{WhisperGrammarElement, WhisperGrammarElementType};
pub use whisper_params::{FullParams, SamplingStrategy, SegmentCallbackData};

pub use whisper_state::{WhisperSegment, WhisperState, WhisperStateSegmentIterator, WhisperToken};
pub use whisper_vad::*;

pub type WhisperSysContext = slab_whisper_sys::whisper_context;
pub type WhisperSysState = slab_whisper_sys::whisper_state;

pub type WhisperTokenData = slab_whisper_sys::whisper_token_data;
pub type WhisperTokenId = slab_whisper_sys::whisper_token;
pub type WhisperNewSegmentCallback = slab_whisper_sys::whisper_new_segment_callback;
pub type WhisperStartEncoderCallback = slab_whisper_sys::whisper_encoder_begin_callback;
pub type WhisperProgressCallback = slab_whisper_sys::whisper_progress_callback;
pub type WhisperLogitsFilterCallback = slab_whisper_sys::whisper_logits_filter_callback;
pub type WhisperAbortCallback = slab_whisper_sys::ggml_abort_callback;
pub type WhisperLogCallback = slab_whisper_sys::ggml_log_callback;
pub type DtwAhead = slab_whisper_sys::whisper_ahead;

use slab_ggml::GGML;
use slab_ggml::load_runtime_with_ggml_sidecar;
use whisper_ctx::WhisperInnerContext;

#[derive(Clone)]
pub struct Whisper {
    lib: Arc<slab_whisper_sys::WhisperLib>,
    // Keep ggml.dll loaded when backend symbols are resolved from it.
    _ggml_lib: Option<Arc<GGML>>,
}

impl Whisper {
    pub fn new<P: AsRef<Path>>(lib_dir: P) -> Result<Self, WhisperError> {
        let (whisper_lib, ggml_lib) =
            load_runtime_with_ggml_sidecar::<_, slab_whisper_sys::WhisperLib>(lib_dir, "whisper")?;

        Ok(Self { lib: Arc::new(whisper_lib), _ggml_lib: ggml_lib })
    }

    /// Redirect all whisper.cpp and GGML logs to logging hooks installed by whisper-rs.
    ///
    /// This will stop most logs from being output to stdout/stderr and will bring them into
    /// `log` or `tracing`, if the `log_backend` or `tracing_backend` features, respectively,
    /// are enabled. If neither is enabled, this will essentially disable logging, as they won't
    /// be output anywhere.
    ///
    /// Note whisper.cpp and GGML do not reliably follow Rust logging conventions.
    /// Use your logging crate's configuration to control how these logs will be output.
    /// whisper-rs does not currently output any logs, but this may change in the future.
    /// You should configure by module path and use `whisper_rs::ggml_logging_hook`,
    /// and/or `whisper_rs::whisper_logging_hook`, to avoid possibly ignoring useful
    /// `whisper-rs` logs in the future.
    ///
    /// Safe to call multiple times. Only has an effect the first time.
    /// (note this means installing your own logging handlers with unsafe functions after this call
    /// is permanent and cannot be undone)
    pub fn install_logging_hooks(&self) {
        self.install_whisper_logging_hook();
        self.install_ggml_logging_hook();
    }
}

impl fmt::Debug for Whisper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Whisper").finish()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::env;

    #[test]
    fn test_load_library() {
        match env::current_exe() {
            Ok(exe_path) => {
                let dir = exe_path.parent().unwrap().parent().unwrap().join("resources\\libs");
                println!("The executable file directory is: {:?}", dir);
                println!(
                    "Whisper DLL path: {:?}",
                    slab_utils::loader::library_path(&dir, "whisper")
                );
                match Whisper::new(dir) {
                    Ok(_) => println!("Successfully loaded Whisper library!"),
                    Err(e) => println!("Failed to load Whisper library: {}", e),
                }
            }
            Err(e) => println!("Failed to get current executable path: {}", e),
        };
    }
}
