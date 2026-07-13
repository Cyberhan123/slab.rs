//! OpenAI-Responses-compatible adapter (re-export shim).
//!
//! The non-streaming `Response` assembler (`build_response` + `AdapterInput` +
//! the `parse_*` helpers) and the streaming state machine (`StreamCtx` +
//! `envelope_to_events`) now live in
//! `slab_app_core::domain::services::agent::response` (slice 1d/1e — the
//! `ResponseService` owns the OpenAI Responses wire projection). This module
//! re-exports them so the HTTP layer ([`super::handler`]) and [`tests`] keep
//! their historical import paths unchanged.
//!
//! Boundary rules (unchanged):
//! - The projection never calls the agent services and never touches
//!   `tokio`/`axum`/sqlx.
//! - HTTP / SSE / WebSocket framing stays in [`super::handler`].
//! - [`tests::redact_dynamic_fields`] is test-only normalization used to compare
//!   adapter output against the golden fixtures in
//!   `testdata/fixtures/openai-compatible/responses/`.

// Transitional: kept while the remaining local `build_error_response` is wired
// into the HTTP error paths. Removed once every error branch calls it.
// `unused_imports` covers `AdapterInput`/`build_response`, which are re-exported
// for the fixture tests (consumed only under `#[cfg(test)]`).
#![allow(dead_code, unused_imports)]

use serde_json::Value;

// Re-export the wire projection from the app-core `ResponseService` module so
// historical call sites (`handler.rs`, `tests.rs`) keep compiling unchanged.
// The `parse_*` helpers stay `pub` in the response module but are no longer
// re-exported here — only the streaming state machine (now in app-core)
// consumed them, and it imports them directly from `super::projection`.
pub use slab_app_core::domain::services::agent::response::single_shot::StreamFrame;
pub use slab_app_core::domain::services::agent::response::{
    AdapterInput, StreamCtx, build_response, build_terminal_event, envelope_to_events,
};

/// Build an OpenAI-style top-level error envelope `{"error": {...}}` for the
/// non-streaming error path. Mirrors `slab_app_core::schemas::chat::OpenAiError`
/// but is emitted as a `serde_json::Value` so the handler can return it from
/// any 4xx/5xx branch without pulling the chat schema into this pure adapter.
pub fn build_error_response(
    message: &str,
    error_type: &str,
    code: &str,
    param: Option<String>,
) -> serde_json::Value {
    let mut error = serde_json::Map::new();
    error.insert("message".to_owned(), Value::String(message.to_owned()));
    error.insert("type".to_owned(), Value::String(error_type.to_owned()));
    error.insert("code".to_owned(), Value::String(code.to_owned()));
    if let Some(p) = param {
        error.insert("param".to_owned(), Value::String(p));
    }
    serde_json::json!({ "error": Value::Object(error) })
}

#[cfg(test)]
mod tests;
