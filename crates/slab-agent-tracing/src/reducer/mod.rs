//! L3 conversation reducer — OFFLINE reconstruction of what the model saw.
//!
//! This module is purely a diagnostic: it is NEVER wired into the agent hot
//! path. The reducer reads a finished (or in-progress) trace bundle and folds
//! its many inferences into the linear conversation the model was shown — the
//! L3 semantic layer that complements the L1 rollout (which records *what
//! happened*) by reconstructing *what the model actually received*.
//!
//! See [`conversation`] for the folding semantics (AppendOnly /
//! FullSnapshot / post-compaction).

pub mod conversation;

pub use conversation::{
    ConversationMessage, ReduceError, reduce_conversation, reduce_conversation_cached,
};
