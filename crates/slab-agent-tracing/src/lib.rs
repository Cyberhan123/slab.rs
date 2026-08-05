//! Session-scoped trace logging for agent and local-runtime diagnostics.
//!
//! This crate is the single owner of agent trace recording. It has two layers:
//!
//! 1. **Free-form sink API** (legacy, unchanged): [`AgentTraceContext`] +
//!    [`AgentTraceSink`] + [`record_json`] / [`record_json_from_context`]. The
//!    ~50 call sites across `slab-agent`, `slab-app-core`, and `slab-runtime`
//!    plus the `AgentTraceContext` propagation chain depend only on these
//!    public items and are NOT changed by this refactor.
//! 2. **Typed trace bundle**: the [`bundle`] and [`writer`] modules
//!    lay out an L3 bundle directory (`manifest.json` / `trace.jsonl` /
//!    `payloads/*.json`) with a payload-before-event invariant, plus a typed
//!    [`RawTraceEvent`] taxonomy in [`event`].
//! 3. **Conversation reducer**: the [`reducer`] module folds a
//!    bundle's many inferences into the linear conversation the model was shown
//!    (AppendOnly / FullSnapshot / post-compaction). This is an OFFLINE L3
//!    diagnostic — it is never wired into the agent hot path. The L1 rollout
//!    records *what happened*; the reducer reconstructs *what the model saw*.
//!
//! This crate is decoupled from `slab-otel`: the file sink now emits its
//! session-telemetry event directly via `tracing::info!` with the SAME target
//! (`slab_otel::session`) and fields as `slab_otel::SessionTelemetry::emit_event`,
//! so downstream filters see a byte-identical wire format. `slab-otel` remains
//! a dependency of `slab-agent` / `slab-app-core` for gen_ai metrics.

pub mod bundle;
pub mod bundle_sink;
pub mod event;
pub mod reducer;
pub mod writer;

mod context;
mod sink;

pub use context::AgentTraceContext;
pub use event::{RawPayloadKind, RawPayloadRef, RawTraceEvent, RawTraceEventPayload};
pub use sink::{
    AgentTraceEvent, AgentTraceSink, FileAgentTraceSink, NoopAgentTraceSink, record_json,
    record_json_from_context, sanitize_session_id, session_log_file_name, session_log_path,
};

// Bundle re-exports at the crate root for ergonomic access by callers.
pub use bundle::{
    AGENT_TRACE_DIR_NAME, BUNDLE_FORMAT_VERSION, BundleManifest, BundleStart, MANIFEST_FILE,
    PAYLOADS_DIR, STATE_FILE, TRACE_FILE, ThreadTraceContext, TraceBundle, agent_trace_root,
    bundle_dir_for_root_thread, bundle_dir_name, start_root_or_disabled,
};
pub use bundle_sink::BundleAgentTraceSink;

/// Compile-time snapshot of this crate's `Cargo.toml`. Used by the
/// `crate_has_no_slab_otel_dependency` test to assert the decouple is
/// not accidentally reverted.
#[cfg(test)]
const CARGO_TOML: &str = include_str!("../Cargo.toml");

#[cfg(test)]
mod decouple_tests {
    use super::*;

    #[test]
    fn crate_has_no_slab_otel_dependency() {
        // Decouple: slab-otel must NOT appear as a dependency line.
        // (Comments mentioning it are fine; the dependency line is not.)
        for line in CARGO_TOML.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            assert!(
                !trimmed.starts_with("slab-otel"),
                "slab-otel dependency must stay removed: found `{trimmed}`"
            );
        }
    }

    #[test]
    fn public_api_surface_is_re_exported() {
        // Smoke-check that the public items documented as the unchanged API are
        // reachable from the crate root with their original signatures. The 50+
        // call sites import these paths.
        let ctx = AgentTraceContext::new("session");
        let noop: Box<dyn AgentTraceSink> = Box::new(NoopAgentTraceSink);
        record_json(
            noop.as_ref(),
            &ctx,
            "smoke-source",
            "smoke-event",
            serde_json::json!({ "ok": true }),
        );
        record_json_from_context(&ctx, "smoke-source", "smoke-event", serde_json::json!({}));
        assert_eq!(sanitize_session_id("a/b"), "a_b");
        assert!(
            session_log_file_name("s", chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())
                .starts_with("slab-agent-session-")
        );
        let _ = session_log_path(
            std::path::Path::new("/tmp"),
            "s",
            chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
        );
        // Exercise the event constructor path so the re-export resolves.
        let _event = AgentTraceEvent::new("s", "e", serde_json::json!(null));
        let _sink = FileAgentTraceSink::new("/tmp");
    }
}
