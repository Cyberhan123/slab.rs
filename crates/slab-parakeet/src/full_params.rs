use crate::error::ParakeetError;
use serde::{Deserialize, Serialize};
use std::ffi::c_int;

/// Stable Rust-native full inference parameters shared across the runtime chain.
///
/// Parakeet is **greedy-only** (the C enum has a single `PARAKEET_SAMPLING_GREEDY`
/// strategy), so there is no `SamplingStrategy` here. Only the subset of
/// `parakeet_full_params` the runtime actually tunes is surfaced; the callback
/// slots are left at their C defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FullParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n_threads: Option<c_int>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_ms: Option<c_int>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<c_int>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_context: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_ctx: Option<c_int>,
}

/// Inner FFI-mirroring full params. Parakeet's `parakeet_full_params` holds no
/// owned string/token buffers, so (unlike whisper) there is no `sync_backing` step.
#[derive(Debug)]
pub(crate) struct InnerFullParams {
    pub(crate) fp: slab_parakeet_sys::parakeet_full_params,
}

impl InnerFullParams {
    pub(crate) fn from_canonical(
        lib: &slab_parakeet_sys::ParakeetLib,
        value: &FullParams,
    ) -> Result<Self, ParakeetError> {
        let mut inner = Self {
            fp: unsafe {
                lib.parakeet_full_default_params(
                    slab_parakeet_sys::parakeet_sampling_strategy_PARAKEET_SAMPLING_GREEDY,
                )
            },
        };

        if let Some(n_threads) = value.n_threads {
            inner.fp.n_threads = n_threads;
        }
        if let Some(offset_ms) = value.offset_ms {
            inner.fp.offset_ms = offset_ms;
        }
        if let Some(duration_ms) = value.duration_ms {
            inner.fp.duration_ms = duration_ms;
        }
        if let Some(no_context) = value.no_context {
            inner.fp.no_context = no_context;
        }
        if let Some(audio_ctx) = value.audio_ctx {
            inner.fp.audio_ctx = audio_ctx;
        }

        Ok(inner)
    }
}

// concurrent usage is prevented by &mut self on methods that use the struct
unsafe impl Send for InnerFullParams {}
unsafe impl Sync for InnerFullParams {}
