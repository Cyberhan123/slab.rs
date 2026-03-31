macro_rules! generic_error {
    ($($expr:tt)*) => {
        #[cfg(feature = "log_backend")]
        log::error!($($expr)*);
        #[cfg(feature = "tracing_backend")]
        tracing::error!($($expr)*);
    };
}

macro_rules! generic_warn {
    ($($expr:tt)*) => {
        #[cfg(feature = "log_backend")]
        log::warn!($($expr)*);
        #[cfg(feature = "tracing_backend")]
        tracing::warn!($($expr)*);
    }
}

macro_rules! generic_info {
    ($($expr:tt)*) => {
        #[cfg(feature = "log_backend")]
        log::info!($($expr)*);
        #[cfg(feature = "tracing_backend")]
        tracing::info!($($expr)*);
    }
}

macro_rules! generic_debug {
    ($($expr:tt)*) => {
        #[cfg(feature = "log_backend")]
        log::debug!($($expr)*);
        #[cfg(feature = "tracing_backend")]
        tracing::debug!($($expr)*);
    }
}

macro_rules! generic_trace {
    ($($expr:tt)*) => {
        #[cfg(feature = "log_backend")]
        log::trace!($($expr)*);
        #[cfg(feature = "tracing_backend")]
        tracing::trace!($($expr)*);
    }
}

use slab_whisper_sys::ggml_log_level;
pub(crate) use {generic_debug, generic_error, generic_info, generic_trace, generic_warn};

// Unsigned integer type on most platforms is 32 bit, niche platforms that whisper.cpp
// likely doesn't even support would use 16 bit and would still fit
#[cfg_attr(any(not(windows), target_env = "gnu"), repr(u32))]
// Of course Windows thinks it's a special little shit and
// picks a signed integer for an unsigned type
#[cfg_attr(all(windows, not(target_env = "gnu")), repr(i32))]
pub enum GGMLLogLevel {
    None = slab_whisper_sys::ggml_log_level_GGML_LOG_LEVEL_NONE,
    Info = slab_whisper_sys::ggml_log_level_GGML_LOG_LEVEL_INFO,
    Warn = slab_whisper_sys::ggml_log_level_GGML_LOG_LEVEL_WARN,
    Error = slab_whisper_sys::ggml_log_level_GGML_LOG_LEVEL_ERROR,
    Debug = slab_whisper_sys::ggml_log_level_GGML_LOG_LEVEL_DEBUG,
    Cont = slab_whisper_sys::ggml_log_level_GGML_LOG_LEVEL_CONT,
    Unknown(ggml_log_level),
}
impl From<ggml_log_level> for GGMLLogLevel {
    fn from(level: ggml_log_level) -> Self {
        match level {
            slab_whisper_sys::ggml_log_level_GGML_LOG_LEVEL_NONE => GGMLLogLevel::None,
            slab_whisper_sys::ggml_log_level_GGML_LOG_LEVEL_INFO => GGMLLogLevel::Info,
            slab_whisper_sys::ggml_log_level_GGML_LOG_LEVEL_WARN => GGMLLogLevel::Warn,
            slab_whisper_sys::ggml_log_level_GGML_LOG_LEVEL_ERROR => GGMLLogLevel::Error,
            slab_whisper_sys::ggml_log_level_GGML_LOG_LEVEL_DEBUG => GGMLLogLevel::Debug,
            slab_whisper_sys::ggml_log_level_GGML_LOG_LEVEL_CONT => GGMLLogLevel::Cont,
            other => GGMLLogLevel::Unknown(other),
        }
    }
}
