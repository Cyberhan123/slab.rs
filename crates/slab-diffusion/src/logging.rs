use core::ffi::{c_char, c_void};
use std::borrow::Cow;
use std::ffi::CStr;
use std::sync::Once;

use crate::Diffusion;

static DIFFUSION_LOG_HOOK_INSTALL: Once = Once::new();

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DiffusionLogLevel {
    Debug,
    Info,
    Warn,
    Error,
    Unknown(slab_diffusion_sys::sd_log_level_t),
}

impl From<slab_diffusion_sys::sd_log_level_t> for DiffusionLogLevel {
    fn from(level: slab_diffusion_sys::sd_log_level_t) -> Self {
        match level {
            slab_diffusion_sys::sd_log_level_t_SD_LOG_DEBUG => Self::Debug,
            slab_diffusion_sys::sd_log_level_t_SD_LOG_INFO => Self::Info,
            slab_diffusion_sys::sd_log_level_t_SD_LOG_WARN => Self::Warn,
            slab_diffusion_sys::sd_log_level_t_SD_LOG_ERROR => Self::Error,
            other => Self::Unknown(other),
        }
    }
}

impl Diffusion {
    /// Redirect native stable-diffusion.cpp logs into `tracing`.
    pub fn install_logging_hook(&self) {
        DIFFUSION_LOG_HOOK_INSTALL.call_once(|| match self.lib.sd_set_log_callback.as_ref() {
            Ok(sd_set_log_callback) => unsafe {
                sd_set_log_callback(Some(diffusion_logging_trampoline), std::ptr::null_mut());
            },
            Err(error) => {
                tracing::debug!(
                    target: "slab_diffusion::ffi",
                    error = %error,
                    "stable-diffusion log callback symbol is unavailable"
                );
            }
        });
    }
}

unsafe extern "C" fn diffusion_logging_trampoline(
    level: slab_diffusion_sys::sd_log_level_t,
    text: *const c_char,
    _: *mut c_void,
) {
    if text.is_null() {
        tracing::error!(target: "slab_diffusion::ffi", "stable-diffusion log callback received null text");
        return;
    }

    let level = DiffusionLogLevel::from(level);
    let text = unsafe { CStr::from_ptr(text) }.to_string_lossy();
    emit_diffusion_log(level, text);
}

fn emit_diffusion_log(level: DiffusionLogLevel, text: Cow<'_, str>) {
    let text = text.trim_end();
    match level {
        DiffusionLogLevel::Debug => {
            tracing::debug!(target: "slab_diffusion::ffi", "{}", text);
        }
        DiffusionLogLevel::Info => {
            tracing::info!(target: "slab_diffusion::ffi", "{}", text);
        }
        DiffusionLogLevel::Warn => {
            tracing::warn!(target: "slab_diffusion::ffi", "{}", text);
        }
        DiffusionLogLevel::Error => {
            tracing::error!(target: "slab_diffusion::ffi", "{}", text);
        }
        DiffusionLogLevel::Unknown(native_level) => {
            tracing::warn!(
                target: "slab_diffusion::ffi",
                native_level,
                message = %text,
                "stable-diffusion log callback received unknown log level"
            );
        }
    }
}
