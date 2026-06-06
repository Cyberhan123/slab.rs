//! Session-scoped trace logging for agent and local-runtime diagnostics.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use chrono::{Datelike, Local, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::warn;

/// Context that lets independent layers append trace events to one agent session file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentTraceContext {
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_index: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_dir: Option<PathBuf>,
}

impl AgentTraceContext {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self { session_id: session_id.into(), thread_id: None, turn_index: None, trace_dir: None }
    }

    pub fn with_thread(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = Some(thread_id.into());
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
    writers: Mutex<HashMap<String, BufWriter<File>>>,
}

static CONTEXT_SINKS: OnceLock<Mutex<HashMap<PathBuf, Arc<FileAgentTraceSink>>>> = OnceLock::new();

impl FileAgentTraceSink {
    pub fn new(log_dir: impl Into<PathBuf>) -> Self {
        Self {
            log_dir: log_dir.into(),
            sequence: AtomicU64::new(0),
            writers: Mutex::new(HashMap::new()),
        }
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

    fn record_inner(
        &self,
        context: &AgentTraceContext,
        event: &AgentTraceEvent,
    ) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.log_dir)?;
        let date = Local::now().date_naive();
        let file_name = session_log_file_name(&context.session_id, date);
        let path = self.log_dir.join(file_name);
        let key = path.to_string_lossy().into_owned();

        let mut writers = self.writers.lock().expect("agent trace writer lock poisoned");
        if !writers.contains_key(&key) {
            let file = OpenOptions::new().create(true).append(true).open(&path)?;
            writers.insert(key.clone(), BufWriter::new(file));
        }

        let record = AgentTraceRecord {
            timestamp: Utc::now().to_rfc3339(),
            session_id: &context.session_id,
            thread_id: context.thread_id.as_deref(),
            turn_index: context.turn_index,
            sequence: self.sequence.fetch_add(1, Ordering::SeqCst),
            source: &event.source,
            event: &event.event,
            payload: &event.payload,
        };
        let writer = writers.get_mut(&key).expect("writer should exist after insertion");
        serde_json::to_writer(&mut *writer, &record)?;
        writer.write_all(b"\n")?;
        writer.flush()
    }
}

impl AgentTraceSink for FileAgentTraceSink {
    fn record(&self, context: &AgentTraceContext, event: AgentTraceEvent) {
        if let Err(error) = self.record_inner(context, &event) {
            warn!(
                session_id = %context.session_id,
                source = %event.source,
                event = %event.event,
                error = %error,
                "failed to write agent trace event"
            );
        }
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
    fn file_sink_writes_jsonl_and_increments_sequence() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        let context = AgentTraceContext::new("session")
            .with_thread("thread")
            .with_turn(3)
            .with_trace_dir(temp.path());
        let sink = FileAgentTraceSink::new(temp.path());

        sink.record(
            &context,
            AgentTraceEvent::new("test", "first", serde_json::json!({ "value": 1 })),
        );
        sink.record(
            &context,
            AgentTraceEvent::new("test", "second", serde_json::json!({ "value": 2 })),
        );

        let path = session_log_path(temp.path(), "session", Local::now().date_naive());
        let raw = std::fs::read_to_string(path).expect("trace file should be readable");
        let lines = raw.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        let first: Value = serde_json::from_str(lines[0]).expect("first line should be JSON");
        let second: Value = serde_json::from_str(lines[1]).expect("second line should be JSON");
        assert_eq!(first["sequence"], 0);
        assert_eq!(second["sequence"], 1);
        assert_eq!(first["session_id"], "session");
        assert_eq!(first["thread_id"], "thread");
        assert_eq!(first["turn_index"], 3);
    }
}
