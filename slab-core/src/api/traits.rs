//! Public capability traits for slab-core engine backends.
//!
//! These traits define the contract that every engine adapter must satisfy.
//! They are the **only** public surface for backend capability; all concrete
//! engine types remain `pub(crate)` inside `internal`.
//!
//! # Trait hierarchy
//!
//! ```text
//! ModelLoader  ←  base: load / unload / is_loaded
//!     └── CausalLM  ←  adds: generate / generate_stream
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use slab_core::api::traits::{CausalLM, ModelLoader};
//! ```

use crate::base::error::CoreError;
use crate::inference::{JsonOptions, TextGenerationChunk, TextGenerationRequest, TextGenerationResponse};

// ── Config marker trait ───────────────────────────────────────────────────────

/// Marker trait for load-time engine configuration types.
///
/// Implement this on any struct that you pass to [`ModelLoader::load`].
/// It carries no required methods; its sole purpose is to make the
/// [`ModelLoader::LoadConfig`] associated-type bound self-documenting and
/// prevent accidental use of unrelated types as load configs.
pub trait ModelLoadConfig: Send + Sync {}

// ── ModelLoader ───────────────────────────────────────────────────────────────

/// Base capability trait for engine backends that can load and unload a model.
///
/// `ModelLoader` is the foundation for all higher-level capability traits.
/// Future capability traits (e.g., audio transcription, image generation) will
/// also extend `ModelLoader` so that callers can always rely on a uniform
/// model-lifecycle API.
///
/// # Contract
///
/// - Calling [`load`] while a model is already loaded **replaces** it.
/// - [`unload`] is safe to call when no model is loaded (it is a no-op).
/// - [`is_loaded`] reflects whether the engine currently holds a live model
///   in memory.
///
/// [`load`]: ModelLoader::load
/// [`unload`]: ModelLoader::unload
/// [`is_loaded`]: ModelLoader::is_loaded
pub trait ModelLoader: Send + Sync {
    /// Load-time configuration accepted by this backend.
    ///
    /// The type is kept `pub(crate)` for internal backends; only the trait
    /// itself is part of the public API.
    type LoadConfig: ModelLoadConfig;

    /// Load a model using the provided configuration.
    ///
    /// If a model is already loaded it is replaced. Implementations must
    /// ensure that any previously allocated resources (workers, KV cache,
    /// etc.) are released before the new model is activated.
    fn load(&self, config: Self::LoadConfig) -> Result<(), CoreError>;

    /// Unload the current model and release all backend resources.
    fn unload(&self) -> Result<(), CoreError>;

    /// Returns `true` if a model is currently loaded and ready for inference.
    fn is_loaded(&self) -> bool;
}

// ── CausalLM ──────────────────────────────────────────────────────────────────

/// Capability trait for causal language model backends.
///
/// Extends [`ModelLoader`] with text-generation capability (both unary and
/// streaming).  A model **must** be loaded via [`ModelLoader::load`] before
/// any inference method is called; calling inference on an unloaded engine
/// returns [`CoreError::ModelNotLoaded`].
///
/// # Streaming
///
/// [`generate_stream`] returns a [`tokio::sync::mpsc::Receiver`] that yields
/// [`TextGenerationChunk`]s.  The stream ends with a terminal chunk where
/// [`TextGenerationChunk::done`] is `true`.  If generation fails after the
/// stream has started, the terminal chunk additionally contains an `"error"`
/// key in its `metadata` map.
///
/// [`generate_stream`]: CausalLM::generate_stream
#[async_trait::async_trait]
pub trait CausalLM: ModelLoader + Send + Sync {
    /// Run non-streaming text generation.
    ///
    /// Awaits until the full response is assembled, then returns it.
    async fn generate(
        &self,
        request: &TextGenerationRequest,
    ) -> Result<TextGenerationResponse, CoreError>;

    /// Run streaming text generation.
    ///
    /// Returns a channel receiver that yields [`TextGenerationChunk`]s as
    /// tokens are produced.  The final chunk has `done == true`.
    /// Errors during generation are reported as a terminal chunk with
    /// `done == true` and an `"error"` entry in `metadata`.
    async fn generate_stream(
        &self,
        request: &TextGenerationRequest,
    ) -> Result<tokio::sync::mpsc::Receiver<TextGenerationChunk>, CoreError>;
}

// ── Internal helpers (not part of the public API) ─────────────────────────────

/// Convert an engine-level stream error into a terminal [`TextGenerationChunk`].
pub(crate) fn error_chunk(message: impl Into<String>) -> TextGenerationChunk {
    let mut meta = JsonOptions::default();
    meta.insert("error".to_owned(), serde_json::Value::String(message.into()));
    TextGenerationChunk {
        delta: String::new(),
        done: true,
        metadata: meta,
    }
}

/// Build the final "done" [`TextGenerationChunk`].
pub(crate) fn done_chunk() -> TextGenerationChunk {
    TextGenerationChunk {
        delta: String::new(),
        done: true,
        metadata: JsonOptions::default(),
    }
}
