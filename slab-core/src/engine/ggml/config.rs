//! Shared deserialization types for GGML backend worker configuration.
//!
//! These structs are used by all three GGML backends (llama, whisper, diffusion)
//! for their `lib.load`, `lib.reload`, and `model.load` JSON input payloads.

use serde::Deserialize;

/// Input configuration for `lib.load` / `lib.reload` operations.
#[derive(Deserialize)]
pub(crate) struct LibLoadConfig {
    pub lib_path: String,
}

/// Input configuration for `model.load` operations (all backends).
#[derive(Deserialize)]
pub(crate) struct ModelLoadConfig {
    pub model_path: String,
}
