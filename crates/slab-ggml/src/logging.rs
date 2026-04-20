use core::ffi::{c_char, c_void};
use std::borrow::Cow;
use std::ffi::CStr;
use std::sync::Once;

use crate::GGML;

static GGML_LOG_HOOK_INSTALL: Once = Once::new();

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum GgmlLogLevel {
    None,
    Debug,
    Info,
    Warn,
    Error,
    Cont,
    Unknown(slab_ggml_sys::ggml_log_level),
}

impl From<slab_ggml_sys::ggml_log_level> for GgmlLogLevel {
    fn from(level: slab_ggml_sys::ggml_log_level) -> Self {
        match level {
            slab_ggml_sys::ggml_log_level_GGML_LOG_LEVEL_NONE => Self::None,
            slab_ggml_sys::ggml_log_level_GGML_LOG_LEVEL_DEBUG => Self::Debug,
            slab_ggml_sys::ggml_log_level_GGML_LOG_LEVEL_INFO => Self::Info,
            slab_ggml_sys::ggml_log_level_GGML_LOG_LEVEL_WARN => Self::Warn,
            slab_ggml_sys::ggml_log_level_GGML_LOG_LEVEL_ERROR => Self::Error,
            slab_ggml_sys::ggml_log_level_GGML_LOG_LEVEL_CONT => Self::Cont,
            other => Self::Unknown(other),
        }
    }
}

impl GGML {
    pub fn install_logging_hook(&self) {
        GGML_LOG_HOOK_INSTALL.call_once(|| match self.lib.base.ggml_log_set.as_ref() {
            Ok(ggml_log_set) => unsafe {
                ggml_log_set(Some(ggml_logging_trampoline), std::ptr::null_mut());
            },
            Err(error) => {
                tracing::debug!(
                    target: "slab_ggml::ffi",
                    error = %error,
                    "ggml log callback symbol is unavailable"
                );
            }
        });
    }
}

unsafe extern "C" fn ggml_logging_trampoline(
    level: slab_ggml_sys::ggml_log_level,
    text: *const c_char,
    _: *mut c_void,
) {
    if text.is_null() {
        tracing::error!(target: "slab_ggml::ffi", "ggml log callback received null text");
        return;
    }

    let level = GgmlLogLevel::from(level);
    let text = unsafe { CStr::from_ptr(text) }.to_string_lossy();
    emit_ggml_log(level, text);
}

fn emit_ggml_log(level: GgmlLogLevel, text: Cow<'_, str>) {
    let text = text.trim_end();
    match level {
        GgmlLogLevel::None | GgmlLogLevel::Cont => {
            tracing::trace!(target: "slab_ggml::ffi", "{}", text);
        }
        GgmlLogLevel::Debug => {
            tracing::debug!(target: "slab_ggml::ffi", "{}", text);
        }
        GgmlLogLevel::Info => {
            tracing::info!(target: "slab_ggml::ffi", "{}", text);
        }
        GgmlLogLevel::Warn => {
            tracing::warn!(target: "slab_ggml::ffi", "{}", text);
        }
        GgmlLogLevel::Error => {
            tracing::error!(target: "slab_ggml::ffi", "{}", text);
        }
        GgmlLogLevel::Unknown(native_level) => {
            tracing::warn!(
                target: "slab_ggml::ffi",
                native_level,
                message = %text,
                "ggml log callback received unknown log level"
            );
        }
    }
}
