//! Live bundle-aware trace sink (hot-path wiring).
//!
//! [`BundleAgentTraceSink`] is the production sink assembled by `slab-app-core`
//! bootstrap when `agent.debug` is on. It is a thin adapter over the existing
//! [`crate::FileAgentTraceSink`] (legacy per-session JSONL + the
//! `slab_otel::session` telemetry wire) that ADDITIONALLY records every event
//! into a per-root-thread trace bundle (`manifest.json` / `trace.jsonl` /
//! `payloads/*.json`).
//!
//! ## Why it composes a legacy sink instead of replacing it
//!
//! Two reasons:
//!
//! 1. **Telemetry wire must stay byte-identical.** The decouple made
//!    [`crate::FileAgentTraceSink`] emit the `slab_otel::session` event directly
//!    via `tracing::info!`. Delegating to that same sink preserves the wire
//!    format exactly (a review checkpoint).
//! 2. **`slab-runtime` (cross-process) keeps writing the legacy JSONL this
//!    slice.** It calls [`crate::record_json_from_context`], which resolves a
//!    shared [`crate::FileAgentTraceSink`] keyed by `trace_dir`. Keeping the
//!    legacy file alive means the cross-process path is untouched (plan risk #6).
//!
//! The bundle is therefore ADDITIVE here: main-process `slab-agent`
//! events land in BOTH the legacy JSONL and the bundle; `slab-runtime` and the
//! `slab-app-core` adapter `record_json_from_context` events keep landing in the
//! legacy JSONL only.
//!
//! ## Failure mode
//!
//! All bundle write failures are diagnostic-only: a bundle/append error is
//! logged at `warn!` and the event is simply not recorded to the bundle. Agent
//! execution always continues (the trait contract).
//!
//! ## Per-record writer + per-bundle append lock
//!
//! A fresh [`crate::writer::TraceWriter`] is opened per record so each event is
//! stamped with its OWN `thread_id` / `turn_index` / `parent_span_id` (a root
//! thread and a child thread sharing one bundle must carry distinct stamps).
//! Reopening `trace.jsonl` in append mode is cheap, and payload ids are
//! uuid-based so there is no collision across the per-record writers (see the
//! `writer` module docs). The bundle DIRECTORY (and its once-written manifest)
//! is cached per root thread id so the manifest is written exactly once.
//!
//! Because a fresh `BufWriter` is opened per record, two concurrent `record`
//! calls into the SAME bundle could otherwise interleave a `trace.jsonl` line's
//! JSON body with its trailing `"\n"` (a buffered writer may split one logical
//! line across OS `write()` calls). A PER-BUNDLE append lock serializes the
//! payload-write + append section for one bundle, so concurrent root + subagent
//! records into the same bundle produce clean, parseable lines. The lock is
//! per-bundle (not global), so distinct root bundles do not block each other.
//!
//! ## Bundle cache growth (known bound)
//!
//! The per-root-thread bundle cache (`bundles`) grows by one entry per DISTINCT
//! root thread id seen during the process lifetime and is never evicted. Each
//! entry is tiny (a directory path + a `Mutex`), and the number of distinct
//! roots equals the number of root agent threads the process has run — bounded
//! by session activity, not by event volume. If long-running hosts ever make
//! this a concern, an LRU / soft cap is the follow-up (deferred: no current
//! host runs enough root threads to matter).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::bundle::{AGENT_TRACE_DIR_NAME, BundleStart, ThreadTraceContext, TraceBundle};
use crate::event::{RawPayloadKind, RawPayloadRef, RawTraceEventPayload};
use crate::sink::{AgentTraceEvent, AgentTraceSink, FileAgentTraceSink};
use crate::writer::TraceWriter;

/// Bundle-aware production trace sink.
///
/// See the module docs for the composition rationale and failure mode.
pub struct BundleAgentTraceSink {
    /// Configured agent-trace base directory (the legacy log dir). Bundles live
    /// under `<trace_dir>/agent_trace/`.
    trace_dir: PathBuf,
    /// Legacy sink: per-session JSONL + `slab_otel::session` telemetry wire.
    legacy: Arc<FileAgentTraceSink>,
    /// Cached per-root-thread bundle entries: the bundle directory (created +
    /// manifest-once on first sight) AND a per-bundle append lock. The append
    /// lock serializes concurrent `record` calls into the SAME bundle (root +
    /// subagent threads in one process) so the fresh-writer-per-record pattern
    /// cannot interleave partial `trace.jsonl` lines.
    bundles: Mutex<HashMap<String, BundleEntry>>,
    /// Optional rollout JSONL path stamped into the bundle manifest. None until
    /// a caller wires it (the manifest field is optional).
    rollout_path: Option<String>,
}

/// Cached bundle bookkeeping (see [`BundleAgentTraceSink::bundles`]).
struct BundleEntry {
    dir: PathBuf,
    append_lock: Arc<Mutex<()>>,
}

impl BundleAgentTraceSink {
    /// Create a bundle sink rooted at `trace_dir`. The internal legacy
    /// [`FileAgentTraceSink`] is resolved from the process-global registry
    /// ([`FileAgentTraceSink::shared_for_log_dir`]) so it shares ONE sequence
    /// counter with the `record_json_from_context` path that writes into the
    /// SAME per-session JSONL — preventing the `[0,0,1,1]` interleaved-sequence
    /// collision that two independent sink instances would produce.
    pub fn new(trace_dir: impl Into<PathBuf>) -> Self {
        let trace_dir = trace_dir.into();
        Self {
            legacy: FileAgentTraceSink::shared_for_log_dir(&trace_dir),
            trace_dir,
            bundles: Mutex::new(HashMap::new()),
            rollout_path: None,
        }
    }

    /// Stamp a rollout JSONL path into the bundle manifest (optional diagnostic
    /// pointer). Consumed on first bundle creation for each root thread.
    pub fn with_rollout_path(mut self, rollout_path: Option<String>) -> Self {
        self.rollout_path = rollout_path;
        self
    }

    /// Convenience constructor returning a trait object, mirroring
    /// [`FileAgentTraceSink::shared`].
    pub fn shared(trace_dir: impl Into<PathBuf>) -> Arc<dyn AgentTraceSink> {
        Arc::new(Self::new(trace_dir))
    }

    /// The configured trace base directory.
    pub fn trace_dir(&self) -> &Path {
        &self.trace_dir
    }

    /// Resolve (creating + caching) the bundle entry for the context's root
    /// thread: the bundle directory (manifest written once) and a per-bundle
    /// append lock. Returns the cloneable entry so the caller can drop the
    /// registry guard before taking the append lock (no deadlock, other
    /// bundles are not blocked). Returns `None` when the bundle cannot be
    /// created (diagnostic-only; caller skips the bundle).
    fn bundle_entry_for(&self, root_thread_id: &str) -> Option<BundleEntry> {
        let mut guard = self.bundles.lock().expect("bundle sink dirs lock poisoned");
        if let Some(entry) = guard.get(root_thread_id) {
            return Some(BundleEntry {
                dir: entry.dir.clone(),
                append_lock: Arc::clone(&entry.append_lock),
            });
        }
        let bundle_root = self.trace_dir.join(AGENT_TRACE_DIR_NAME);
        let start = BundleStart::new(
            root_thread_id.to_owned(),
            root_thread_id.to_owned(),
            self.rollout_path.clone(),
        );
        match TraceBundle::create_at(bundle_root, start) {
            Ok(bundle) => {
                let entry = BundleEntry {
                    dir: bundle.dir().to_path_buf(),
                    append_lock: Arc::new(Mutex::new(())),
                };
                let cloned = BundleEntry {
                    dir: entry.dir.clone(),
                    append_lock: Arc::clone(&entry.append_lock),
                };
                guard.insert(root_thread_id.to_owned(), entry);
                Some(cloned)
            }
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    root_thread_id = %root_thread_id,
                    "failed to create agent trace bundle; event not recorded to bundle",
                );
                None
            }
        }
    }

    /// Resolve the root thread id for a context. Prefers the explicit
    /// `root_thread_id` field; falls back to the thread id when this is a root
    /// thread (no parent). Returns `None` for a child context without an
    /// explicit root thread id (cannot group — event stays legacy-only).
    fn root_thread_id_of(context: &crate::AgentTraceContext) -> Option<String> {
        if let Some(root) = context.root_thread_id.as_deref().filter(|s| !s.is_empty()) {
            return Some(root.to_owned());
        }
        if context.is_root_thread() {
            return context.thread_id.clone();
        }
        None
    }
}

impl AgentTraceSink for BundleAgentTraceSink {
    fn record(&self, context: &crate::AgentTraceContext, event: AgentTraceEvent) {
        // (1) Legacy path: per-session JSONL + slab_otel::session telemetry wire.
        // Delegating preserves the byte-identical telemetry wire and keeps the
        // cross-process slab-runtime path (which writes the same legacy file
        // via record_json_from_context) untouched.
        self.legacy.record(context, event.clone());

        // (2) Bundle path. Resolve the root thread id; without it the event
        // stays legacy-only (the bundle groups per root thread).
        let Some(root_thread_id) = Self::root_thread_id_of(context) else {
            return;
        };
        let Some(entry) = self.bundle_entry_for(&root_thread_id) else {
            return;
        };

        // Open a fresh writer per record so the event is stamped with THIS
        // context's thread/turn/parent (root vs child in the same bundle).
        let thread_ctx = ThreadTraceContext::new(
            entry.dir,
            context.thread_id.clone().unwrap_or_else(|| root_thread_id.clone()),
            context.turn_index,
            context.parent_span_id.clone(),
        );
        let writer = match TraceWriter::for_thread(&thread_ctx) {
            Ok(writer) => writer,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "failed to open trace writer; event not recorded to bundle",
                );
                return;
            }
        };

        // Serialize the append (and payload write) per bundle: the fresh-writer-
        // per-record pattern opens a NEW BufWriter each call, so without this
        // lock two concurrent records into the SAME bundle could interleave a
        // line's JSON body and its trailing "\n" (BufWriter may split a line
        // across OS write() calls). The per-bundle granularity means different
        // root bundles are NOT blocked by each other.
        let _append_guard = entry.append_lock.lock().expect("bundle append lock poisoned");

        // Bridge the free-form event into the typed taxonomy, writing the
        // payload file FIRST (payload-before-event invariant). Unmatched events
        // fall through to the Other catch-all so nothing is dropped.
        let typed = build_typed_event(&writer, &event);
        if let Err(error) = writer.append(typed) {
            tracing::warn!(error = %error, "trace bundle append failed");
        }
    }
}

/// Write the event payload to a payload file (when non-null) and return the
/// reference, or `None` for a null payload.
fn write_optional_payload(
    writer: &TraceWriter,
    kind: RawPayloadKind,
    payload: &Value,
) -> Option<RawPayloadRef> {
    if payload.is_null() {
        return None;
    }
    match writer.write_json_payload(kind, payload) {
        Ok(reff) => Some(reff),
        Err(error) => {
            tracing::warn!(
                error = %error,
                "trace bundle payload write failed; recording event without payload ref",
            );
            None
        }
    }
}

/// Classify a free-form event name into a typed [`RawTraceEventPayload`], writing
/// the payload file first when the variant carries one.
///
/// Two matching strategies coexist:
///
/// - **Exact whitelist for inference** (`agent_llm_request`,
///   `llm_response_normalized`, `chat_response_normalized`). Substring matching
///   on `"request"`/`"response"`/`"normalized"` was RETIRED because slab-agent
///   emits a real `structured_output_requested` event every turn when
///   structured output is configured — a substring match would misclassify it
///   as `InferenceStarted` and pollute the bundle's inference dimension with a
///   phantom request (the reducer would see a ghost inference turn). The
///   same hazard applies to free-form names like `runtime_request`,
///   `mcp_approval_request`, and `invalid_request_error` (all real names in the
///   app stack). Exact matching is safe: the inference request/response event
///   names are fixed `record_json` literals in `slab-agent`/`slab-app-core`.
/// - **Substring matching for tool / turn / compaction** — those lifecycles have
///   no real-name collisions in the app stack (verified against every
///   `record_json` call site), and the "completed" qualifier on compaction
///   excludes the "skipped" outcome.
///
/// Any event that does not match falls through to [`RawTraceEventPayload::Other`]
/// carrying its source/event/payload verbatim, so an event is NEVER dropped —
/// only its taxonomy slot varies.
///
/// Marker variants ([`RawTraceEventPayload::TurnStarted`] / `TurnCompleted` /
/// `ContextCompacted`) have no payload slot; their (small, redundant) payloads
/// are intentionally not carried — the `thread_id`/`turn_index` are already on
/// the event envelope, and the rollout true source records turn/compaction
/// details authoritatively.
fn build_typed_event(writer: &TraceWriter, event: &AgentTraceEvent) -> RawTraceEventPayload {
    let name = event.event.to_ascii_lowercase();

    // Tool lifecycle.
    if name.contains("tool_call_started") {
        return RawTraceEventPayload::ToolCallStarted {
            payload: write_optional_payload(writer, RawPayloadKind::ToolCall, &event.payload),
        };
    }
    if name.contains("tool_call_output")
        || name == "tool_calls_completed"
        || name.contains("tool_call_completed")
    {
        return RawTraceEventPayload::ToolCallCompleted {
            payload: write_optional_payload(writer, RawPayloadKind::ToolResult, &event.payload),
        };
    }
    // Turn lifecycle (exact-ish; "thread_started"/"thread_completed" must NOT match).
    if name.contains("turn_started") {
        return RawTraceEventPayload::TurnStarted;
    }
    if name.contains("turn_completed") {
        return RawTraceEventPayload::TurnCompleted;
    }
    // Compaction (only the completed outcome; "skipped" stays Other).
    if name.contains("compaction") && name.contains("completed") {
        return RawTraceEventPayload::ContextCompacted;
    }
    // Inference request — EXACT match only. A substring `contains("request")`
    // would misclassify `structured_output_requested` (emitted every turn when
    // structured output is configured) and other `*_request*` names as a
    // phantom inference request, polluting the reducer's inference dimension.
    if name == "agent_llm_request" {
        return RawTraceEventPayload::InferenceStarted {
            request_payload: write_optional_payload(
                writer,
                RawPayloadKind::Request,
                &event.payload,
            ),
        };
    }
    // Inference response — EXACT whitelist only (same derived-word hazard).
    if name == "llm_response_normalized" || name == "chat_response_normalized" {
        return RawTraceEventPayload::InferenceCompleted {
            response_payload: write_optional_payload(
                writer,
                RawPayloadKind::Response,
                &event.payload,
            ),
        };
    }
    // Catch-all: never drop an event.
    RawTraceEventPayload::Other {
        source: event.source.clone(),
        event: event.event.clone(),
        payload: write_optional_payload(writer, RawPayloadKind::Other, &event.payload),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::{
        AGENT_TRACE_DIR_NAME, MANIFEST_FILE, PAYLOADS_DIR, TRACE_FILE, bundle_dir_for_root_thread,
    };
    use crate::context::AgentTraceContext;
    use serde_json::json;

    /// Run `f` under a default `tracing_subscriber::registry()` dispatcher.
    ///
    /// Every `BundleAgentTraceSink::record` delegates to `FileAgentTraceSink::record`,
    /// which emits `tracing::info!(target: "slab_otel::session", ...)`. If a test
    /// evaluates that callsite FIRST under a no-op dispatcher, tracing caches the
    /// interest as "never" and starves `sink::tests::file_sink_emits_session_telemetry`
    /// (a known cross-test callsite-caching flake in this crate; the
    /// `file_sink_writes_jsonl_to_session_file` test exists for the same reason).
    /// Running the record() body under a registry keeps callsite caching
    /// deterministic regardless of test execution order.
    fn under_registry<F: FnOnce()>(f: F) {
        tracing::subscriber::with_default(tracing_subscriber::registry(), f);
    }

    /// Read+parse every line of a bundle's trace.jsonl.
    fn read_trace_events(dir: &Path) -> Vec<crate::event::RawTraceEvent> {
        let contents = std::fs::read_to_string(dir.join(TRACE_FILE)).expect("read trace.jsonl");
        contents
            .trim()
            .lines()
            .map(|line| serde_json::from_str(line).expect("parse trace line"))
            .collect()
    }

    /// Read+parse a payload referenced by a ref.
    fn read_payload(dir: &Path, reff: &RawPayloadRef) -> Value {
        let body = std::fs::read_to_string(dir.join(&reff.path)).expect("read payload");
        serde_json::from_str(body.trim()).expect("parse payload")
    }

    #[test]
    fn classifies_high_frequency_event_names() {
        // No writer needed for marker variants; for payload variants we only
        // check the discriminant (payload writing is exercised in the disk
        // tests below). Use a throwaway bundle for the payload-writing cases.
        let temp = tempfile::tempdir().expect("temp dir");
        let bundle =
            TraceBundle::create_at(temp.path().to_path_buf(), BundleStart::new("rt", "rt", None))
                .expect("create bundle");
        let ctx = ThreadTraceContext::new(bundle.dir(), "rt", None, None);
        let writer = TraceWriter::for_thread(&ctx).expect("open writer");

        let mk = |event: &str, payload: Value| {
            build_typed_event(&writer, &AgentTraceEvent::new("slab-agent", event, payload))
        };

        assert!(matches!(mk("turn_started", json!({})), RawTraceEventPayload::TurnStarted));
        assert!(matches!(mk("turn_completed", json!({})), RawTraceEventPayload::TurnCompleted));
        assert!(matches!(
            mk("context_compaction_completed", json!({})),
            RawTraceEventPayload::ContextCompacted
        ));
        assert!(matches!(
            mk("agent_llm_request", json!({ "prompt": 1 })),
            RawTraceEventPayload::InferenceStarted { .. }
        ));
        assert!(matches!(
            mk("llm_response_normalized", json!({ "text": "x" })),
            RawTraceEventPayload::InferenceCompleted { .. }
        ));
        assert!(matches!(
            mk("tool_call_started", json!({ "tool": "x" })),
            RawTraceEventPayload::ToolCallStarted { .. }
        ));
        assert!(matches!(
            mk("tool_call_output", json!({ "out": 1 })),
            RawTraceEventPayload::ToolCallCompleted { .. }
        ));
        assert!(matches!(
            mk("tool_calls_completed", json!({})),
            RawTraceEventPayload::ToolCallCompleted { .. }
        ));
        // Catch-all preserves source+event.
        match mk("thread_started", json!({ "depth": 0 })) {
            RawTraceEventPayload::Other { source, event, .. } => {
                assert_eq!(source, "slab-agent");
                assert_eq!(event, "thread_started");
            }
            other => panic!("expected Other, got {other:?}"),
        }
        // Skipped compaction is NOT a marker.
        assert!(matches!(
            mk("context_compaction_skipped", json!({})),
            RawTraceEventPayload::Other { .. }
        ));
        // Case-insensitive.
        assert!(matches!(mk("TURN_STARTED", json!({})), RawTraceEventPayload::TurnStarted));

        // F1 negative cases: real event names whose substrings ("request" /
        // "response" / "normalized") must NOT be misclassified as inference.
        // `structured_output_requested` is emitted every turn when structured
        // output is configured (slab-agent turn.rs); misclassifying it would
        // invent a phantom inference request in the reducer. The rest are real
        // app-stack names (`runtime_request` from slab-app-core local.rs,
        // `mcp_approval_request` / `invalid_request_error` from the response
        // event surface). All must fall through to Other.
        for phantom in [
            "structured_output_requested",
            "runtime_request",
            "mcp_approval_request",
            "invalid_request_error",
            "tool_call_approval_required",
        ] {
            match mk(phantom, json!({ "x": 1 })) {
                RawTraceEventPayload::Other { event, .. } => {
                    assert_eq!(event, phantom, "{phantom} kept its name in Other");
                }
                other => panic!("{phantom} must classify as Other (not inference), got {other:?}"),
            }
        }
    }

    /// F1 disk-level guard: driving the real [`BundleAgentTraceSink::record`]
    /// with the actual `structured_output_requested` event name must persist a
    /// trace.jsonl line tagged `kind: "other"`, NEVER `inference_started`. This
    /// pins the fix against a substring-regression (a future relaxation of the
    /// inference arm would surface here, not in a phantom reducer turn).
    #[test]
    fn structured_output_requested_records_as_other_not_inference() {
        let temp = tempfile::tempdir().expect("temp dir");
        let trace_dir = temp.path().to_path_buf();
        let sink = BundleAgentTraceSink::new(trace_dir.clone());
        let ctx = AgentTraceContext::new("session")
            .with_thread("rt")
            .with_root_thread_id("rt")
            .with_trace_dir(trace_dir.clone());

        // The exact payload slab-agent emits (turn.rs).
        under_registry(|| {
            sink.record(
                &ctx,
                AgentTraceEvent::new(
                    "slab-agent",
                    "structured_output_requested",
                    json!({ "structured_output": { "schema": "x" } }),
                ),
            );
        });

        let dir = bundle_dir_for_root_thread(&trace_dir, "rt");
        let events = read_trace_events(&dir);
        assert_eq!(events.len(), 1, "one event recorded");
        match &events[0].event {
            RawTraceEventPayload::Other { source, event, payload } => {
                assert_eq!(source, "slab-agent");
                assert_eq!(event, "structured_output_requested");
                assert!(payload.is_some(), "Other payload carried");
            }
            other => panic!(
                "structured_output_requested must be Other, got {other:?} (phantom inference)"
            ),
        }
        // No inference_started line leaked into the bundle.
        let trace_text = std::fs::read_to_string(dir.join(TRACE_FILE)).expect("read trace");
        assert!(
            !trace_text.contains("\"kind\":\"inference_started\""),
            "phantom inference leaked: {trace_text}",
        );
        assert!(trace_text.contains("\"kind\":\"other\""), "Other line present: {trace_text}");
    }

    #[test]
    fn writes_live_bundle_with_manifest_trace_and_payloads() {
        let temp = tempfile::tempdir().expect("temp dir");
        let trace_dir = temp.path().to_path_buf();
        let sink = BundleAgentTraceSink::new(trace_dir.clone());

        let root_ctx = AgentTraceContext::new("session")
            .with_thread("root-1")
            .with_turn(0)
            .with_trace_dir(trace_dir.clone())
            .with_root_thread_id("root-1");

        // Mix of typed + Other + marker + null-payload events.
        sink.record(
            &root_ctx,
            AgentTraceEvent::new("slab-agent", "turn_started", json!({ "depth": 0 })),
        );
        sink.record(
            &root_ctx,
            AgentTraceEvent::new(
                "slab-agent",
                "agent_llm_request",
                json!({ "model": "x", "messages": [{ "role": "user", "content": "hi" }] }),
            ),
        );
        sink.record(
            &root_ctx,
            AgentTraceEvent::new(
                "slab-agent",
                "llm_response_normalized",
                json!({ "content": "yo" }),
            ),
        );
        sink.record(
            &root_ctx,
            AgentTraceEvent::new("slab-agent", "tool_call_started", json!({ "name": "shell" })),
        );
        sink.record(
            &root_ctx,
            AgentTraceEvent::new("slab-agent", "thread_started", json!({ "k": "v" })),
        );
        // Null payload should not produce a payload file.
        sink.record(&root_ctx, AgentTraceEvent::new("slab-agent", "some_event", Value::Null));

        let bundle_dir = bundle_dir_for_root_thread(&trace_dir, "root-1");
        assert!(bundle_dir.is_dir(), "bundle dir created: {}", bundle_dir.display());

        // manifest.json. The schema defines 5 fields (trace_id, root_thread_id,
        // created_at, rollout_path, format_version); rollout_path is optional
        // (skip_serializing_if None) so the 4 core fields are always present and
        // rollout_path appears only when wired.
        let manifest: serde_json::Value = serde_json::from_str(
            std::fs::read_to_string(bundle_dir.join(MANIFEST_FILE)).expect("read manifest").trim(),
        )
        .expect("manifest json");
        let obj = manifest.as_object().expect("manifest object");
        assert!(obj.contains_key("trace_id"), "manifest has trace_id: {obj:?}");
        assert!(obj.contains_key("root_thread_id"), "manifest has root_thread_id");
        assert!(obj.contains_key("created_at"), "manifest has created_at");
        assert!(obj.contains_key("format_version"), "manifest has format_version");
        assert_eq!(manifest["trace_id"], "root-1");
        assert_eq!(manifest["root_thread_id"], "root-1");
        assert_eq!(manifest["format_version"], crate::bundle::BUNDLE_FORMAT_VERSION);
        assert!(manifest["created_at"].is_string());

        // payloads/ created and populated.
        let payloads_dir = bundle_dir.join(PAYLOADS_DIR);
        assert!(payloads_dir.is_dir());
        let payload_files: Vec<_> = std::fs::read_dir(&payloads_dir)
            .expect("read payloads")
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().and_then(|e| e.to_str()) == Some("json"))
            .collect();
        // request + response + tool_call + thread_started(Other) = 4 payload
        // files (turn_started marker has no payload; null-payload event none).
        assert_eq!(payload_files.len(), 4, "payload files: {payload_files:?}");

        // trace.jsonl: one line per record (6 events).
        let events = read_trace_events(&bundle_dir);
        assert_eq!(events.len(), 6, "one trace line per record");

        // Payload-before-event: every referenced payload file exists + matches.
        for event in &events {
            for reff in event.event.payload_refs() {
                let abs = bundle_dir.join(&reff.path);
                assert!(abs.is_file(), "referenced payload missing: {}", abs.display());
                let _body = read_payload(&bundle_dir, reff);
            }
        }

        // Every stamped event carries this context's thread id + turn.
        for event in &events {
            assert_eq!(event.thread_id.as_deref(), Some("root-1"));
            assert_eq!(event.turn_index, Some(0));
        }

        // Legacy file also written (telemetry wire + JSONL).
        let legacy_glob = std::fs::read_dir(&trace_dir).expect("read trace dir");
        let has_legacy = legacy_glob
            .filter_map(Result::ok)
            .any(|e| e.file_name().to_string_lossy().starts_with("slab-agent-session-"));
        assert!(has_legacy, "legacy per-session JSONL still written alongside bundle");

        // No dangling refs: every payload id in trace.jsonl has a file.
        let trace_text = std::fs::read_to_string(bundle_dir.join(TRACE_FILE)).expect("read trace");
        for event in &events {
            for reff in event.event.payload_refs() {
                assert!(trace_text.contains(&reff.raw_payload_id));
            }
        }
    }

    #[test]
    fn per_root_thread_grouping_and_consistency_with_helper() {
        let temp = tempfile::tempdir().expect("temp dir");
        let trace_dir = temp.path().to_path_buf();
        let sink = BundleAgentTraceSink::new(trace_dir.clone());

        // Root thread event.
        let root_ctx = AgentTraceContext::new("session")
            .with_thread("root-A")
            .with_root_thread_id("root-A")
            .with_trace_dir(trace_dir.clone());
        sink.record(&root_ctx, AgentTraceEvent::new("slab-agent", "turn_started", json!({})));

        // Child of root-A shares the SAME bundle (via root_thread_id).
        let child_ctx = AgentTraceContext::new("session")
            .with_thread("child-A1")
            .with_parent_span_id("root-A")
            .with_root_thread_id("root-A")
            .with_trace_dir(trace_dir.clone());
        sink.record(
            &child_ctx,
            AgentTraceEvent::new("slab-agent", "agent_llm_request", json!({ "i": 1 })),
        );

        // A different root gets a different bundle.
        let root_b_ctx = AgentTraceContext::new("session")
            .with_thread("root-B")
            .with_root_thread_id("root-B")
            .with_trace_dir(trace_dir.clone());
        sink.record(&root_b_ctx, AgentTraceEvent::new("slab-agent", "turn_started", json!({})));

        let dir_a = bundle_dir_for_root_thread(&trace_dir, "root-A");
        let dir_b = bundle_dir_for_root_thread(&trace_dir, "root-B");
        assert_ne!(dir_a, dir_b, "different roots → different bundle dirs");

        // root-A bundle has both the root event and the child event.
        let events_a = read_trace_events(&dir_a);
        assert_eq!(events_a.len(), 2, "root + child in one bundle");
        // Child event stamped with its OWN thread id, not the root's.
        assert!(
            events_a.iter().any(|e| e.thread_id.as_deref() == Some("child-A1")),
            "child event carries its own thread id",
        );
        assert!(
            events_a.iter().any(|e| e.thread_id.as_deref() == Some("root-A")),
            "root event carries its own thread id",
        );

        // root-B bundle has its single event.
        let events_b = read_trace_events(&dir_b);
        assert_eq!(events_b.len(), 1);
    }

    #[test]
    fn payload_ids_unique_across_records() {
        let temp = tempfile::tempdir().expect("temp dir");
        let trace_dir = temp.path().to_path_buf();
        let sink = BundleAgentTraceSink::new(trace_dir.clone());

        let ctx = AgentTraceContext::new("session")
            .with_thread("root")
            .with_root_thread_id("root")
            .with_trace_dir(trace_dir.clone());

        // Record many request events; each must get a distinct payload id and
        // no earlier payload is overwritten.
        for i in 0..5 {
            sink.record(
                &ctx,
                AgentTraceEvent::new(
                    "slab-agent",
                    "agent_llm_request",
                    json!({ "turn": i, "msg": vec![format!("m{i}")] }),
                ),
            );
        }

        let dir = bundle_dir_for_root_thread(&trace_dir, "root");
        let events = read_trace_events(&dir);
        assert_eq!(events.len(), 5);

        // F4: strong order-preserving assertion. For the idx-th record the
        // payload content must match idx exactly — turn == idx and msg == ["m{idx}"].
        // This proves NO earlier payload was overwritten or silently swapped by
        // a later record (a collision-regression would show turn != idx).
        let mut ids = Vec::new();
        for (idx, event) in events.iter().enumerate() {
            let refs: Vec<_> = event.event.payload_refs();
            assert_eq!(refs.len(), 1, "one payload per record: idx {idx}");
            let reff = refs[0];
            ids.push(reff.raw_payload_id.clone());
            let body = read_payload(&dir, reff);
            assert_eq!(
                body["turn"].as_i64(),
                Some(idx as i64),
                "record {idx} payload carries turn {idx}: {}",
                body,
            );
            assert_eq!(
                body["msg"],
                json!([format!("m{idx}")]),
                "record {idx} payload carries msg m{idx}: {}",
                body,
            );
        }
        ids.sort();
        let mut deduped = ids.clone();
        deduped.dedup();
        assert_eq!(ids.len(), deduped.len(), "payload ids must be unique: {ids:?}");

        // 5 distinct payload files.
        let count = std::fs::read_dir(dir.join(PAYLOADS_DIR))
            .expect("read payloads")
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|e| e.to_str()) == Some("json"))
            .count();
        assert_eq!(count, 5);
    }

    #[test]
    fn child_without_root_thread_id_stays_legacy_only() {
        // A child context that lacks an explicit root_thread_id cannot be
        // grouped; it must NOT create a bundle, only the legacy file.
        let temp = tempfile::tempdir().expect("temp dir");
        let trace_dir = temp.path().to_path_buf();
        let sink = BundleAgentTraceSink::new(trace_dir.clone());

        let child_ctx = AgentTraceContext::new("session")
            .with_thread("child")
            .with_parent_span_id("parent") // not a root, no root_thread_id
            .with_trace_dir(trace_dir.clone());
        sink.record(&child_ctx, AgentTraceEvent::new("slab-agent", "turn_started", json!({})));

        // No agent_trace subdir created (no bundle).
        let agent_trace = trace_dir.join(AGENT_TRACE_DIR_NAME);
        assert!(
            !agent_trace.exists(),
            "no bundle created for ungroupable child: {}",
            agent_trace.display(),
        );
    }

    #[test]
    fn bundle_dir_resides_under_agent_trace_subdir() {
        let temp = tempfile::tempdir().expect("temp dir");
        let trace_dir = temp.path().to_path_buf();
        let sink = BundleAgentTraceSink::new(trace_dir.clone());
        let ctx = AgentTraceContext::new("session")
            .with_thread("rt")
            .with_root_thread_id("rt")
            .with_trace_dir(trace_dir.clone());
        sink.record(&ctx, AgentTraceEvent::new("slab-agent", "turn_started", json!({})));

        let dir = bundle_dir_for_root_thread(&trace_dir, "rt");
        assert!(dir.starts_with(trace_dir.join(AGENT_TRACE_DIR_NAME)));
        assert!(dir.join(MANIFEST_FILE).is_file());
        assert!(dir.join(TRACE_FILE).is_file());
    }

    /// F7: the bundle sink's internal legacy sink MUST share the
    /// process-global sequence counter with `record_json_from_context` (the
    /// cross-process / adapter path) when both target the same `trace_dir`.
    /// Pre-fix the bundle sink held its OWN `FileAgentTraceSink` (counter from
    /// 0), so interleaving the two paths produced `[0,0,1,1]` collisions in the
    /// shared per-session JSONL. This test drives BOTH paths into the same file
    /// and asserts every `sequence` is distinct.
    #[test]
    fn bundle_sink_shares_sequence_counter_with_record_json_from_context() {
        use crate::sink::{record_json_from_context, session_log_path};

        let temp = tempfile::tempdir().expect("temp dir");
        let trace_dir = temp.path().to_path_buf();
        let sink = BundleAgentTraceSink::new(trace_dir.clone());

        let ctx = AgentTraceContext::new("seq-session")
            .with_thread("rt")
            .with_root_thread_id("rt")
            .with_trace_dir(trace_dir.clone());

        // Two events via the bundle sink's legacy delegation.
        under_registry(|| {
            sink.record(&ctx, AgentTraceEvent::new("slab-agent", "turn_started", json!({})));
            sink.record(&ctx, AgentTraceEvent::new("slab-agent", "thread_started", json!({})));
            // Two events via the parallel `record_json_from_context` path (the
            // slab-runtime / slab-app-core adapter route).
            record_json_from_context(&ctx, "slab-runtime", "runtime_request", json!({ "i": 1 }));
            record_json_from_context(&ctx, "slab-runtime", "runtime_chunk", json!({ "i": 2 }));
        });

        // Read the shared per-session JSONL both paths wrote into.
        let path = session_log_path(&trace_dir, "seq-session", chrono::Utc::now().date_naive());
        let contents = std::fs::read_to_string(&path).expect("read session log");
        let lines: Vec<&str> = contents.trim().lines().collect();
        assert_eq!(lines.len(), 4, "all 4 records in one shared file: {contents}");

        let mut seqs: Vec<i64> = lines
            .iter()
            .map(|line| {
                let v: serde_json::Value = serde_json::from_str(line).expect("parse line");
                v["sequence"].as_i64().expect("sequence field present")
            })
            .collect();
        let mut deduped = seqs.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(
            seqs.len(),
            deduped.len(),
            "every sequence distinct across the two write paths: {seqs:?}",
        );
        // And they form a contiguous 0..N run (no gaps from a second counter).
        seqs.sort();
        assert_eq!(seqs, (0..4).collect::<Vec<_>>(), "sequences are a contiguous run");
    }

    /// F6 concurrency regression: many threads recording into the SAME root
    /// bundle (root + subagent threads in one process) must produce a
    /// `trace.jsonl` where EVERY line is independently parseable — no two
    /// concurrent fresh-`BufWriter` appends interleave a line's JSON body with
    /// another line's trailing newline. The per-bundle append lock serializes
    /// the append section so this holds deterministically.
    #[test]
    fn concurrent_records_into_one_bundle_produce_parseable_lines() {
        use std::sync::Barrier;
        use std::thread;

        let temp = tempfile::tempdir().expect("temp dir");
        let trace_dir = temp.path().to_path_buf();
        let sink = Arc::new(BundleAgentTraceSink::new(trace_dir.clone()));

        // Many distinct CHILD thread ids all rooting back to the SAME root, so
        // every record lands in the same bundle (exercising the append lock).
        let n_threads = 16usize;
        let records_per_thread = 8usize;
        let barrier = Arc::new(Barrier::new(n_threads));
        let mut handles = Vec::new();
        for t in 0..n_threads {
            let sink = Arc::clone(&sink);
            let barrier = Arc::clone(&barrier);
            let thread_trace_dir = trace_dir.clone();
            handles.push(thread::spawn(move || {
                let ctx = AgentTraceContext::new("session")
                    .with_thread(format!("child-{t}"))
                    .with_parent_span_id("root")
                    .with_root_thread_id("root")
                    .with_trace_dir(thread_trace_dir);
                // Barrier so all threads start appends as simultaneously as
                // possible (max contention on the lock + the file).
                barrier.wait();
                // under_registry is thread-local, so set it INSIDE the worker.
                under_registry(|| {
                    for i in 0..records_per_thread {
                        sink.record(
                            &ctx,
                            AgentTraceEvent::new(
                                "slab-agent",
                                "agent_llm_request",
                                json!({ "thread": t, "i": i, "msg": format!("m{t}-{i}") }),
                            ),
                        );
                    }
                });
            }));
        }
        for h in handles {
            h.join().expect("worker thread panicked");
        }

        let dir = bundle_dir_for_root_thread(&trace_dir, "root");
        let contents = std::fs::read_to_string(dir.join(TRACE_FILE)).expect("read trace.jsonl");
        let lines: Vec<&str> = contents.trim().lines().collect();
        assert_eq!(lines.len(), n_threads * records_per_thread, "one parseable line per record",);

        // Every line must parse as a RawTraceEvent (an interleaved/corrupted
        // line would fail to deserialize here).
        for line in &lines {
            let _: crate::event::RawTraceEvent =
                serde_json::from_str(line).unwrap_or_else(|e| panic!("corrupt line: {e}: {line}"));
        }

        // All payload refs resolve to existing files (payload-before-event).
        for line in &lines {
            let event: crate::event::RawTraceEvent =
                serde_json::from_str(line).expect("parse line");
            for reff in event.event.payload_refs() {
                assert!(dir.join(&reff.path).is_file(), "payload missing: {}", reff.path.display());
            }
        }
    }
}
