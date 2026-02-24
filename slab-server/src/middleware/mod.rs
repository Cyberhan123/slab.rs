//! HTTP middleware stack.
//!
//! Re-exports the trace module and the [`TraceLayer`] type.

pub mod auth;
pub mod trace;

pub use trace::TraceLayer;
