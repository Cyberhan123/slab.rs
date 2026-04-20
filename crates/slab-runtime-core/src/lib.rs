//! Backend worker/thread runtime primitives for `bin/slab-runtime`.
//!
//! This crate intentionally exposes only the backend-facing execution
//! substrate: worker ingress/control protocols, worker spawning helpers,
//! backend admission, and in-process payload/stream/error primitives.
//! Runtime domain concepts such as task orchestration, model resolution,
//! application errors, and transport mapping belong in higher layers.

extern crate self as slab_runtime_core;

pub mod backend;
mod base;
mod internal;

/// Backend-facing error surface for worker registration, admission, control,
/// and engine adapters. This is not the runtime application's domain error.
pub use base::error::CoreError;
/// In-process backend payload envelope. This is not a public transport DTO.
pub use base::types::Payload;
