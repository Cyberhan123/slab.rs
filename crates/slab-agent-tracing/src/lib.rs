//! Session-scoped trace logging for agent and local-runtime diagnostics.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use chrono::{Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Context that lets independent layers append trace events to one agent session file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentTraceContext {
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    /// Span id of the parent thread (the delegating parent's thread id), so
    /// subagent trace events can be correlated back to the parent that spawned
    /// them (INFRA-09). `None` for root threads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_index: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_dir: Option<PathBuf>,
}

impl AgentTraceContext {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            thread_id: None,
            parent_span_id: None,
            turn_index: None,
            trace_dir: None,
        }
    }

    pub fn with_thread(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }

    /// Attach the parent thread's span id (INFRA-09 subagent linkage).
    pub fn with_parent_span_id(mut self, parent_span_id: impl Into<String>) -> Self {
        let parent_span_id = parent_span_id.into();
        self.parent_span_id = if parent_span_id.is_empty() { None } else { Some(parent_span_id) };
        self
    }

    pub fn with_turn(mut self, turn_index: u32) -> Self {
        self.turn_index = Some(turn_index);
        self
    }

    pub fn with_trace_dir(mut self, trace_dir: impl Into<PathBuf>) -> Self {
        self.trace_dir = Some(trace_dir.into());
        self
    }
}

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
        let log_dir = context.trace_dir.as_ref()?.clone();
        let sinks = CONTEXT_SINKS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut guard = sinks.lock().expect("agent trace context sink lock poisoned");
        Some(guard.entry(log_dir.clone()).or_insert_with(|| Arc::new(Self::new(log_dir))).clone())
    }

    fn record_payload(&self, context: &AgentTraceContext, event: &AgentTraceEvent) -> Value {
        let record = AgentTraceRecord {
            timestamp: Utc::now().to_rfc3339(),
            session_id: &context.session_id,
            thread_id: context.thread_id.as_deref(),
            parent_span_id: context.parent_span_id.as_deref(),
            turn_index: context.turn_index,
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
}

impl AgentTraceSink for FileAgentTraceSink {
    fn record(&self, context: &AgentTraceContext, event: AgentTraceEvent) {
        let mut telemetry = slab_otel::SessionTelemetry::new(context.session_id.clone());
        if let Some(thread_id) = context.thread_id.as_deref() {
            telemetry = telemetry.with_thread(thread_id);
        }
        if let Some(turn_index) = context.turn_index {
            telemetry = telemetry.with_turn(turn_index);
        }
        telemetry.emit_event(&event.source, &event.event, self.record_payload(context, &event));
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
