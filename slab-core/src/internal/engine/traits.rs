//! Internal capability traits for slab-core engine backends.
//!
//! These traits define the contract that every engine adapter must satisfy.
//! They live in `internal` and are **only** visible within this crate; external
//! callers use the higher-level public API in `slab_core::api`.
//!
//! # Trait hierarchy
//!
//! ```text
//! ModelLoader  ← base: load / unload / is_loaded
//!     └── CausalLM  ← adds: forward (raw model forward pass → logits)
//! ```
//!
//! # Design notes
//!
//! * `ModelLoader` manages the model lifecycle (weights in memory).
//! * `CausalLM::forward` is a **stateless** single-step computation:
//!   given a sequence of token IDs it returns the logit distribution for the
//!   next token.  KV-cache management, sampling, and session state all live in
//!   the engine adapter layer on top of this trait.
//! * The input to `forward` must be a [`Tensor`] created with
//!   [`Tensor::from_token_ids`]; the output is a logit [`Tensor`] created
//!   with [`Tensor::from_logits`].

use crate::base::error::CoreError;
use crate::internal::engine::tensor::Tensor;

// ── ModelLoadConfig ───────────────────────────────────────────────────────────

/// Marker trait for types that carry load-time engine configuration.
///
/// Implement on any struct passed to [`ModelLoader::load`].  The trait has no
/// required methods; it exists as a documentation and type-safety aid.
pub(crate) trait ModelLoadConfig: Send + Sync {}

// ── ModelLoader ───────────────────────────────────────────────────────────────

/// Trait for engine backends that manage a model lifecycle.
///
/// # Contract
///
/// * Calling [`load`] while a model is already loaded **replaces** it.  Any
///   previously allocated resources (worker threads, KV-cache, etc.) are
///   released before the new model is activated.
/// * [`unload`] is a no-op when no model is loaded.
/// * [`is_loaded`] reflects whether the engine currently holds live model
///   weights ready for inference.
///
/// [`load`]: ModelLoader::load
/// [`unload`]: ModelLoader::unload
/// [`is_loaded`]: ModelLoader::is_loaded
pub(crate) trait ModelLoader: Send + Sync {
    /// The load-time configuration type accepted by this backend.
    type LoadConfig: ModelLoadConfig;

    /// Load a model using `config`.  Replaces any currently-loaded model.
    fn load(&self, config: Self::LoadConfig) -> Result<(), CoreError>;

    /// Unload the current model and release all backend resources.
    fn unload(&self) -> Result<(), CoreError>;

    /// Returns `true` when a model is loaded and ready for inference.
    fn is_loaded(&self) -> bool;
}

// ── CausalLM ──────────────────────────────────────────────────────────────────

/// Trait for causal language model backends that expose a raw forward pass.
///
/// Extends [`ModelLoader`]: the model must be loaded (via
/// [`ModelLoader::load`]) before calling [`forward`].  Calling [`forward`]
/// when no model is loaded returns [`CoreError::ModelNotLoaded`].
///
/// # Forward pass semantics
///
/// `forward` runs a **stateless** single decode step:
///
/// 1. `input_ids` — a token-ID [`Tensor`] of shape `[seq_len]` covering the
///    full context window.
/// 2. The method runs the model over those tokens and returns a logit
///    [`Tensor`] of shape `[vocab_size]` for the **last** input position.
///
/// Neither KV-cache state nor session state is read or mutated by this method.
/// The sampling step (argmax / nucleus / etc.) and KV-cache management are the
/// caller's responsibility.
///
/// # Input preconditions
///
/// * `input_ids` must have been constructed with [`Tensor::from_token_ids`].
/// * `input_ids` must not be empty.
///
/// [`forward`]: CausalLM::forward
pub(crate) trait CausalLM: ModelLoader + Send + Sync {
    /// Run a single stateless forward pass and return per-vocabulary logits.
    ///
    /// `input_ids` is a 1-D token-ID tensor (`U32` data variant).
    /// Returns a 1-D logit tensor (`F32` data variant, length = vocab size).
    fn forward(&self, input_ids: &Tensor) -> Result<Tensor, CoreError>;
}
