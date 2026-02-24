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
use whisper_ctx::WhisperInnerContext;
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

#[derive(Clone)]
pub struct Whisper {
    // 确保这里的类型名和你 bindgen 生成的一致
    lib: Arc<slab_whisper_sys::WhisperLib>,
}

impl Whisper {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, WhisperError> {
        // 1. 在 unsafe 块中尝试加载库，并使用 ? 提取出 lib
        let lib = unsafe { slab_whisper_sys::WhisperLib::new(path.as_ref())? };

        // 2. 将加载成功的 lib 包装进 Arc
        Ok(Self { lib: Arc::new(lib) })
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
