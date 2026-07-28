use crate::Parakeet;
use crate::common_logging::{
    GGMLLogLevel, generic_debug, generic_error, generic_info, generic_trace, generic_warn,
};
use core::ffi::{c_char, c_void};
use slab_parakeet_sys::ggml_log_level;
use std::borrow::Cow;
use std::ffi::CStr;
use std::sync::Once;

static PARAKEET_LOG_TRAMPOLINE_INSTALL: Once = Once::new();

impl Parakeet {
    pub fn install_parakeet_logging_hook(&self) {
        PARAKEET_LOG_TRAMPOLINE_INSTALL.call_once(|| match self.lib.parakeet_log_set.as_ref() {
            Ok(parakeet_log_set) => unsafe {
                parakeet_log_set(Some(parakeet_logging_trampoline), std::ptr::null_mut());
            },
            Err(_error) => {
                generic_debug!("parakeet log callback symbol is unavailable: {}", _error);
            }
        });
    }
}

unsafe extern "C" fn parakeet_logging_trampoline(
    level: ggml_log_level,
    text: *const c_char,
    _: *mut c_void, // user_data
) {
    if text.is_null() {
        generic_error!("parakeet_logging_trampoline: text is nullptr");
        return;
    }
    let level = GGMLLogLevel::from(level);

    // SAFETY: we must trust parakeet that it will not pass us a string that does
    // not satisfy from_ptr's requirements.
    let log_str = unsafe { CStr::from_ptr(text) }.to_string_lossy();

    parakeet_logging_trampoline_safe(level, log_str)
}

// this code essentially compiles down to a noop if neither feature is enabled
#[cfg_attr(not(any(feature = "log_backend", feature = "tracing_backend")), allow(unused_variables))]
fn parakeet_logging_trampoline_safe(level: GGMLLogLevel, text: Cow<str>) {
    match level {
        GGMLLogLevel::None => {
            generic_trace!("{}", text.trim());
        }
        GGMLLogLevel::Info => {
            generic_info!("{}", text.trim());
        }
        GGMLLogLevel::Warn => {
            generic_warn!("{}", text.trim());
        }
        GGMLLogLevel::Error => {
            generic_error!("{}", text.trim());
        }
        GGMLLogLevel::Debug => {
            generic_debug!("{}", text.trim());
        }
        GGMLLogLevel::Cont => {
            // parakeet splits long lines and doesn't change the kind, so treat as trace
            generic_trace!("{}", text.trim());
        }
        GGMLLogLevel::Unknown(level) => {
            generic_warn!(
                "parakeet_logging_trampoline: unknown log level {}: message: {}",
                level,
                text.trim()
            );
        }
    }
}
