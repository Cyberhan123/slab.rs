use core::ffi::{c_char, c_void};
use std::ffi::CStr;
use std::sync::Once;

use crate::Llama;

static LLAMA_LOG_HOOK_INSTALL: Once = Once::new();
static GGML_LOG_HOOK_INSTALL: Once = Once::new();

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum GgmlLogLevel {
    None,
    Debug,
    Info,
    Warn,
    Error,
    Cont,
    Unknown(slab_llama_sys::ggml_log_level),
}

impl From<slab_llama_sys::ggml_log_level> for GgmlLogLevel {
    fn from(level: slab_llama_sys::ggml_log_level) -> Self {
        match level {
            slab_llama_sys::ggml_log_level_GGML_LOG_LEVEL_NONE => Self::None,
            slab_llama_sys::ggml_log_level_GGML_LOG_LEVEL_DEBUG => Self::Debug,
            slab_llama_sys::ggml_log_level_GGML_LOG_LEVEL_INFO => Self::Info,
            slab_llama_sys::ggml_log_level_GGML_LOG_LEVEL_WARN => Self::Warn,
            slab_llama_sys::ggml_log_level_GGML_LOG_LEVEL_ERROR => Self::Error,
            slab_llama_sys::ggml_log_level_GGML_LOG_LEVEL_CONT => Self::Cont,
            other => Self::Unknown(other),
        }
    }
}

impl Llama {
    /// Redirect native llama.cpp and GGML logs into `tracing`.
    pub fn install_logging_hooks(&self) {
        self.install_llama_logging_hook();
        self.install_ggml_logging_hook();
    }

    fn install_llama_logging_hook(&self) {
        LLAMA_LOG_HOOK_INSTALL.call_once(|| match self.lib.llama_log_set.as_ref() {
            Ok(llama_log_set) => unsafe {
                llama_log_set(Some(llama_logging_trampoline), std::ptr::null_mut());
            },
            Err(error) => {
                tracing::debug!(
                    target: "slab_llama::ffi",
                    error = %error,
                    "llama log callback symbol is unavailable"
                );
            }
        });
    }

    fn install_ggml_logging_hook(&self) {
        GGML_LOG_HOOK_INSTALL.call_once(|| match self.lib.ggml_log_set.as_ref() {
            Ok(ggml_log_set) => unsafe {
                ggml_log_set(Some(ggml_logging_trampoline), std::ptr::null_mut());
            },
            Err(error) => {
                tracing::debug!(
                    target: "slab_llama::ffi",
                    error = %error,
                    "llama-local ggml log callback symbol is unavailable"
                );
            }
        });
    }
}

unsafe extern "C" fn llama_logging_trampoline(
    level: slab_llama_sys::ggml_log_level,
    text: *const c_char,
    _: *mut c_void,
) {
    emit_native_log(NativeLogSource::Llama, level, text);
}

unsafe extern "C" fn ggml_logging_trampoline(
    level: slab_llama_sys::ggml_log_level,
    text: *const c_char,
    _: *mut c_void,
) {
    emit_native_log(NativeLogSource::Ggml, level, text);
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum NativeLogSource {
    Llama,
    Ggml,
}

macro_rules! emit_ggml_log {
    ($target:literal, $level:expr, $text:expr) => {{
        let text = $text.trim_end();
        match $level {
            GgmlLogLevel::None | GgmlLogLevel::Cont => {
                tracing::trace!(target: $target, "{}", text);
            }
            GgmlLogLevel::Debug => {
                tracing::debug!(target: $target, "{}", text);
            }
            GgmlLogLevel::Info => {
                tracing::info!(target: $target, "{}", text);
            }
            GgmlLogLevel::Warn => {
                tracing::warn!(target: $target, "{}", text);
            }
            GgmlLogLevel::Error => {
                tracing::error!(target: $target, "{}", text);
            }
            GgmlLogLevel::Unknown(native_level) => {
                tracing::warn!(
                    target: $target,
                    native_level,
                    message = %text,
                    "native log callback received unknown log level"
                );
            }
        }
    }};
}

fn emit_native_log(
    source: NativeLogSource,
    level: slab_llama_sys::ggml_log_level,
    text: *const c_char,
) {
    if text.is_null() {
        match source {
            NativeLogSource::Llama => {
                tracing::error!(
                    target: "slab_llama::ffi::llama",
                    "native log callback received null text"
                );
            }
            NativeLogSource::Ggml => {
                tracing::error!(
                    target: "slab_llama::ffi::ggml",
                    "native log callback received null text"
                );
            }
        }
        return;
    }

    let level = GgmlLogLevel::from(level);
    let text = unsafe { CStr::from_ptr(text) }.to_string_lossy();
    match source {
        NativeLogSource::Llama => emit_ggml_log!("slab_llama::ffi::llama", level, text),
        NativeLogSource::Ggml => emit_ggml_log!("slab_llama::ffi::ggml", level, text),
    }
}
