//! Trace sinks: the legacy free-form [`AgentTraceEvent`] + the
//! [`AgentTraceSink`] trait with no-op and file-backed implementations.
//!
//! This crate is decoupled from `slab-otel`: the file sink now emits the
//! session-telemetry event directly via `tracing::info!` with the SAME target
//! (`slab_otel::session`) and field names/order as
//! `slab_otel::SessionTelemetry::emit_event`, so downstream OTel/subscriber
//! filters see a byte-identical wire format.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use chrono::{Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::context::AgentTraceContext;

/// A single event payload written by an [`AgentTraceSink`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentTraceEvent {
    pub source: String,
    pub event: String,
    pub payload: Value,
}

impl AgentTraceEvent {
    pub fn new(source: impl Into<String>, event: impl Into<String>, payload: Value) -> Self {
        Self { source: source.into(), event: event.into(), payload }
    }
}

#[derive(Debug, Serialize)]
struct AgentTraceRecord<'a> {
    timestamp: String,
    session_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_span_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    turn_index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    root_thread_id: Option<&'a str>,
    sequence: u64,
    source: &'a str,
    event: &'a str,
    payload: &'a Value,
}

/// Destination for full-fidelity agent trace events.
///
/// Implementations should treat write failures as diagnostic failures only:
/// agent execution must continue even when tracing cannot be written.
pub trait AgentTraceSink: Send + Sync {
    fn record(&self, context: &AgentTraceContext, event: AgentTraceEvent);
}

/// Trace sink used when agent debugging is disabled.
#[derive(Debug, Default)]
pub struct NoopAgentTraceSink;

impl AgentTraceSink for NoopAgentTraceSink {
    fn record(&self, _context: &AgentTraceContext, _event: AgentTraceEvent) {}
}

/// File-backed JSONL trace sink, grouped by session id.
#[derive(Debug)]
pub struct FileAgentTraceSink {
    log_dir: PathBuf,
    sequence: AtomicU64,
}

static CONTEXT_SINKS: OnceLock<Mutex<HashMap<PathBuf, Arc<FileAgentTraceSink>>>> = OnceLock::new();

impl FileAgentTraceSink {
    pub fn new(log_dir: impl Into<PathBuf>) -> Self {
        Self { log_dir: log_dir.into(), sequence: AtomicU64::new(0) }
    }

    pub fn from_context(context: &AgentTraceContext) -> Option<Self> {
        context.trace_dir.as_ref().map(Self::new)
    }

    pub fn shared(log_dir: impl Into<PathBuf>) -> Arc<dyn AgentTraceSink> {
        Arc::new(Self::new(log_dir))
    }

    pub fn shared_for_context(context: &AgentTraceContext) -> Option<Arc<FileAgentTraceSink>> {
        let log_dir = context.trace_dir.as_ref()?;
        Some(Self::shared_for_log_dir(log_dir))
    }

    /// Resolve the process-global shared [`FileAgentTraceSink`] for a log
    /// directory, creating + caching it on first sight. The registry is keyed
    /// by the canonical log directory, so EVERY caller writing into the same
    /// `log_dir` shares ONE instance — and therefore ONE monotonically
    /// increasing `sequence` counter.
    ///
    /// This matters because two write paths land in the SAME per-session JSONL:
    /// the [`crate::BundleAgentTraceSink`] (main-process `slab-agent` events,
    /// which compose a legacy sink internally) and
    /// [`record_json_from_context`] (`slab-runtime` / `slab-app-core` adapter,
    /// resolved via the registry). If each held its OWN sink instance, both
    /// would start their sequence counter at 0 and interleave `[0,0,1,1,...]`
    /// in the shared file. Routing both through this registry unifies the
    /// counter so sequences are collision-free across the two paths.
    pub fn shared_for_log_dir(log_dir: impl AsRef<Path>) -> Arc<FileAgentTraceSink> {
        let log_dir = log_dir.as_ref().to_path_buf();
        let sinks = CONTEXT_SINKS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut guard = sinks.lock().expect("agent trace context sink lock poisoned");
        guard.entry(log_dir.clone()).or_insert_with(|| Arc::new(Self::new(log_dir))).clone()
    }

    fn record_payload(&self, context: &AgentTraceContext, event: &AgentTraceEvent) -> Value {
        let record = AgentTraceRecord {
            timestamp: Utc::now().to_rfc3339(),
            session_id: &context.session_id,
            thread_id: context.thread_id.as_deref(),
            parent_span_id: context.parent_span_id.as_deref(),
            turn_index: context.turn_index,
            root_thread_id: context.root_thread_id.as_deref(),
            sequence: self.sequence.fetch_add(1, Ordering::SeqCst),
            source: &event.source,
            event: &event.event,
            payload: &event.payload,
        };
        let mut value = serde_json::to_value(record).unwrap_or_else(|error| {
            serde_json::json!({
                "session_id": context.session_id,
                "source": event.source,
                "event": event.event,
                "serialization_error": error.to_string()
            })
        });
        if let Value::Object(object) = &mut value {
            object
                .insert("trace_dir".to_owned(), Value::String(self.log_dir.display().to_string()));
        }
        value
    }

    /// Append one trace record as a JSONL line to the per-session file under
    /// `log_dir`. Creates the directory and file as needed. Errors are
    /// diagnostic only — callers must not abort agent execution on failure.
    fn append_to_session_file(
        &self,
        context: &AgentTraceContext,
        payload: &Value,
    ) -> std::io::Result<()> {
        use std::io::Write;
        let date = Utc::now().date_naive();
        let path = session_log_path(&self.log_dir, &context.session_id, date);
        std::fs::create_dir_all(&self.log_dir)?;
        let file = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
        let mut writer = std::io::BufWriter::new(file);
        serde_json::to_writer(&mut writer, payload)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        Ok(())
    }
}

impl AgentTraceSink for FileAgentTraceSink {
    fn record(&self, context: &AgentTraceContext, event: AgentTraceEvent) {
        let payload = self.record_payload(context, &event);

        // Persist the trace record to the per-session JSONL file. Write failures
        // are diagnostic only — agent execution must continue (see trait docs).
        if let Err(error) = self.append_to_session_file(context, &payload) {
            tracing::warn!(error = %error, "agent trace file append failed");
        }

        // Decouple: emit the session-telemetry event directly. This is
        // WIRE-IDENTICAL to `slab_otel::SessionTelemetry::emit_event` (see
        // crates/slab-otel/src/session.rs:54-71): same target, same field names
        // and order, same `%`/`?` recording conventions. Downstream OTel /
        // subscriber filters that match on target "slab_otel::session" are
        // unchanged.
        tracing::info!(
            target: "slab_otel::session",
            session_id = %context.session_id,
            thread_id = ?context.thread_id,
            turn_index = ?context.turn_index,
            source = %event.source,
            event = %event.event,
            payload = %payload,
            "session telemetry event"
        );
    }
}

pub fn record_json(
    sink: &dyn AgentTraceSink,
    context: &AgentTraceContext,
    source: impl Into<String>,
    event: impl Into<String>,
    payload: Value,
) {
    sink.record(context, AgentTraceEvent::new(source, event, payload));
}

pub fn record_json_from_context(
    context: &AgentTraceContext,
    source: impl Into<String>,
    event: impl Into<String>,
    payload: Value,
) {
    if let Some(sink) = FileAgentTraceSink::shared_for_context(context) {
        sink.record(context, AgentTraceEvent::new(source, event, payload));
    }
}

pub fn sanitize_session_id(session_id: &str) -> String {
    let mut safe = String::with_capacity(session_id.len());
    for ch in session_id.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            safe.push(ch);
        } else {
            safe.push('_');
        }
    }
    let safe = safe.trim_matches('_');
    if safe.is_empty() { "unknown".to_owned() } else { safe.to_owned() }
}

pub fn session_log_file_name(session_id: &str, date: NaiveDate) -> String {
    format!(
        "slab-agent-session-{}-{}-{}-{}.log",
        sanitize_session_id(session_id),
        date.year(),
        date.month(),
        date.day()
    )
}

pub fn session_log_path(log_dir: &Path, session_id: &str, date: NaiveDate) -> PathBuf {
    log_dir.join(session_log_file_name(session_id, date))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tracing::{Event, Subscriber};
    use tracing_subscriber::Layer;
    use tracing_subscriber::layer::Context;
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn sanitizes_session_id_for_file_names() {
        assert_eq!(sanitize_session_id("abc-123_DEF"), "abc-123_DEF");
        assert_eq!(sanitize_session_id("abc/../你好"), "abc");
        assert_eq!(sanitize_session_id("///"), "unknown");
    }

    #[test]
    fn builds_session_file_name_with_unpadded_date() {
        let date = NaiveDate::from_ymd_opt(2026, 6, 5).expect("date should be valid");
        assert_eq!(
            session_log_file_name("session:one", date),
            "slab-agent-session-session_one-2026-6-5.log"
        );
    }

    #[test]
    fn noop_sink_does_not_write() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        let context = AgentTraceContext::new("session").with_trace_dir(temp.path());
        let sink = NoopAgentTraceSink;
        sink.record(
            &context,
            AgentTraceEvent::new("test", "noop", serde_json::json!({ "value": 1 })),
        );

        assert!(
            std::fs::read_dir(temp.path()).expect("temp dir should be readable").next().is_none()
        );
    }

    #[test]
    fn file_sink_builds_records_and_increments_sequence() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        let context = AgentTraceContext::new("session")
            .with_thread("thread")
            .with_turn(3)
            .with_trace_dir(temp.path());
        let sink = FileAgentTraceSink::new(temp.path());

        let first = sink.record_payload(
            &context,
            &AgentTraceEvent::new("test", "first", serde_json::json!({ "value": 1 })),
        );
        let second = sink.record_payload(
            &context,
            &AgentTraceEvent::new("test", "second", serde_json::json!({ "value": 2 })),
        );

        assert_eq!(first["sequence"], 0);
        assert_eq!(second["sequence"], 1);
        assert_eq!(first["session_id"], "session");
        assert_eq!(first["thread_id"], "thread");
        assert_eq!(first["turn_index"], 3);
    }

    #[test]
    fn root_thread_id_is_propagated_to_records_when_set() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        let context = AgentTraceContext::new("session")
            .with_thread("child")
            .with_root_thread_id("root-1")
            .with_trace_dir(temp.path());
        let sink = FileAgentTraceSink::new(temp.path());

        let record = sink.record_payload(
            &context,
            &AgentTraceEvent::new("test", "child_event", serde_json::json!({})),
        );

        assert_eq!(record["root_thread_id"], "root-1");
    }

    #[test]
    fn parent_span_id_is_propagated_to_records() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        let context = AgentTraceContext::new("session")
            .with_thread("child-thread")
            .with_parent_span_id("parent-thread")
            .with_trace_dir(temp.path());
        let sink = FileAgentTraceSink::new(temp.path());

        let record = sink.record_payload(
            &context,
            &AgentTraceEvent::new("test", "child_event", serde_json::json!({})),
        );

        assert_eq!(record["thread_id"], "child-thread");
        assert_eq!(record["parent_span_id"], "parent-thread");
    }

    #[test]
    fn root_thread_omits_parent_span_id() {
        let context = AgentTraceContext::new("session").with_thread("root");
        assert!(context.parent_span_id.is_none());
        // Empty parent span id is normalized away (treated as a root thread).
        assert!(context.with_parent_span_id("").parent_span_id.is_none());
    }

    #[test]
    fn file_sink_emits_session_telemetry() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        let context = AgentTraceContext::new("session").with_trace_dir(temp.path());
        let sink = FileAgentTraceSink::new(temp.path());
        let events = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::registry().with(CaptureTargets(Arc::clone(&events)));

        tracing::subscriber::with_default(subscriber, || {
            sink.record(
                &context,
                AgentTraceEvent::new("test", "first", serde_json::json!({ "value": 1 })),
            );
        });

        assert!(events.lock().expect("events").iter().any(|target| target == "slab_otel::session"));
    }

    #[test]
    fn file_sink_writes_jsonl_to_session_file() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        let context = AgentTraceContext::new("session").with_trace_dir(temp.path());
        let sink = FileAgentTraceSink::new(temp.path());

        // Drive `record` under an enabled registry subscriber so the telemetry
        // bridge evaluates its tracing callsites against a real dispatcher. This
        // keeps cross-test callsite interest caching deterministic — otherwise a
        // no-op dispatcher can cache "never" and starve later subscriber tests.
        tracing::subscriber::with_default(tracing_subscriber::registry(), || {
            sink.record(
                &context,
                AgentTraceEvent::new("test", "first", serde_json::json!({ "value": 1 })),
            );
            sink.record(
                &context,
                AgentTraceEvent::new("test", "second", serde_json::json!({ "value": 2 })),
            );
        });

        let date = chrono::Utc::now().date_naive();
        let path = session_log_path(temp.path(), "session", date);
        let contents = std::fs::read_to_string(&path).expect("session log file should exist");
        let lines: Vec<&str> = contents.trim().lines().collect();
        assert_eq!(lines.len(), 2, "one JSONL line per record");

        let first: serde_json::Value =
            serde_json::from_str(lines[0]).expect("first line should be valid JSON");
        assert_eq!(first["session_id"], "session");
        assert_eq!(first["source"], "test");
        assert_eq!(first["event"], "first");
        assert_eq!(first["sequence"], 0);

        let second: serde_json::Value =
            serde_json::from_str(lines[1]).expect("second line should be valid JSON");
        assert_eq!(second["event"], "second");
        assert_eq!(second["sequence"], 1);
    }

    struct CaptureTargets(Arc<Mutex<Vec<String>>>);

    impl<S> Layer<S> for CaptureTargets
    where
        S: Subscriber,
    {
        fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
            self.0.lock().expect("events").push(event.metadata().target().to_owned());
        }
    }
}
