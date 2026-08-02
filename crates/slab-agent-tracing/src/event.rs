//! Trace event taxonomy.
//!
//! Two layers coexist:
//! 1. The legacy free-form [`super::sink::AgentTraceEvent`] (`source`/`event`/
//!    `payload` strings) used by the ~50 existing `record_json` call sites.
//!    These keep working unchanged.
//! 2. The typed [`RawTraceEvent`] / [`RawTraceEventPayload`] taxonomy below,
//!    used by the Slice 9 trace-bundle writer. The bare-string events are
//!    carried via the [`RawTraceEventPayload::Other`] catch-all variant so the
//!    50 call sites are NOT migrated in this slice — future slices progressively
//!    type the high-frequency events.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Kind of raw payload stored under `payloads/*.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RawPayloadKind {
    /// LLM inference request envelope.
    Request,
    /// LLM inference response envelope.
    Response,
    /// A tool-call invocation payload.
    ToolCall,
    /// A tool-call result payload.
    ToolResult,
    /// Reconstructed turn context (input messages / tool specs).
    TurnContext,
    /// Anything else (legacy string events that carry an opaque JSON payload).
    Other,
}

/// A durable reference to a payload file written under `payloads/`.
///
/// `path` is always relative to the bundle directory so bundles remain
/// relocatable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawPayloadRef {
    pub raw_payload_id: String,
    pub kind: RawPayloadKind,
    pub path: PathBuf,
}

/// Typed payload for a [`RawTraceEvent`]. Covers the 9 reducer dimensions; the
/// `Other` variant is the catch-all for the legacy bare-string events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RawTraceEventPayload {
    /// lifecycle/llm — an inference request was dispatched.
    InferenceStarted {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_payload: Option<RawPayloadRef>,
    },
    /// lifecycle/llm — an inference response was received.
    InferenceCompleted {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        response_payload: Option<RawPayloadRef>,
    },
    /// tool/dispatch — a tool call began.
    ToolCallStarted {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        payload: Option<RawPayloadRef>,
    },
    /// tool/dispatch — a tool call finished.
    ToolCallCompleted {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        payload: Option<RawPayloadRef>,
    },
    /// lifecycle/turn — a turn began.
    TurnStarted,
    /// lifecycle/turn — a turn finished.
    TurnCompleted,
    /// compaction — context was compacted.
    ContextCompacted,
    /// Catch-all for legacy bare-string events (`record_json` source/event). No
    /// call site is migrated in Slice 9; the writer wraps such events here.
    Other {
        source: String,
        event: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        payload: Option<RawPayloadRef>,
    },
}

impl RawTraceEventPayload {
    /// Every payload reference attached to this event (used to verify the
    /// payload-before-event invariant when reading a bundle back).
    pub fn payload_refs(&self) -> Vec<&RawPayloadRef> {
        match self {
            RawTraceEventPayload::InferenceStarted { request_payload } => {
                request_payload.iter().collect()
            }
            RawTraceEventPayload::InferenceCompleted { response_payload } => {
                response_payload.iter().collect()
            }
            RawTraceEventPayload::ToolCallStarted { payload }
            | RawTraceEventPayload::ToolCallCompleted { payload }
            | RawTraceEventPayload::Other { payload, .. } => payload.iter().collect(),
            RawTraceEventPayload::TurnStarted
            | RawTraceEventPayload::TurnCompleted
            | RawTraceEventPayload::ContextCompacted => Vec::new(),
        }
    }

    /// Defensively null out any payload ref whose target does not exist.
    ///
    /// `exists` returns `true` when the referenced payload file is present.
    /// Refs for which it returns `false` are set to `None` so a later
    /// serialization (e.g. by [`crate::writer::TraceWriter::append`]) cannot
    /// carry a dangling reference into `trace.jsonl`. This turns the
    /// payload-before-event invariant from caller-discipline into a runtime
    /// guarantee: a fabricated or out-of-order [`RawPayloadRef`] is dropped
    /// rather than persisted.
    ///
    /// The event itself is always preserved — only the missing refs are nulled.
    pub fn retain_existing_payload_refs(&mut self, mut exists: impl FnMut(&RawPayloadRef) -> bool) {
        let opt: &mut Option<RawPayloadRef> = match self {
            RawTraceEventPayload::InferenceStarted { request_payload } => request_payload,
            RawTraceEventPayload::InferenceCompleted { response_payload } => response_payload,
            RawTraceEventPayload::ToolCallStarted { payload }
            | RawTraceEventPayload::ToolCallCompleted { payload }
            | RawTraceEventPayload::Other { payload, .. } => payload,
            RawTraceEventPayload::TurnStarted
            | RawTraceEventPayload::TurnCompleted
            | RawTraceEventPayload::ContextCompacted => return,
        };
        if let Some(reff) = opt
            && !exists(reff)
        {
            *opt = None;
        }
    }
}

/// One durable event line in `trace.jsonl`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawTraceEvent {
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_index: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,
    pub event: RawTraceEventPayload,
}

impl RawTraceEvent {
    /// Convenience constructor.
    pub fn new(
        timestamp: impl Into<String>,
        thread_id: Option<String>,
        turn_index: Option<u32>,
        parent_span_id: Option<String>,
        event: RawTraceEventPayload,
    ) -> Self {
        Self { timestamp: timestamp.into(), thread_id, turn_index, parent_span_id, event }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_refs_collects_attached_refs() {
        let req = RawPayloadRef {
            raw_payload_id: "p1".into(),
            kind: RawPayloadKind::Request,
            path: PathBuf::from("payloads/p1.json"),
        };
        let event = RawTraceEventPayload::InferenceStarted { request_payload: Some(req.clone()) };
        assert_eq!(event.payload_refs(), vec![&req]);

        assert!(RawTraceEventPayload::TurnStarted.payload_refs().is_empty());

        let other = RawTraceEventPayload::Other {
            source: "slab-agent".into(),
            event: "something".into(),
            payload: None,
        };
        assert!(other.payload_refs().is_empty());
    }

    #[test]
    fn tagged_serialization_round_trips() {
        let event = RawTraceEvent::new(
            "2026-08-02T00:00:00Z",
            Some("thread-1".into()),
            Some(3),
            None,
            RawTraceEventPayload::ToolCallCompleted { payload: None },
        );
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(json.contains("\"kind\":\"tool_call_completed\""), "{json}");

        let restored: RawTraceEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, event);
    }

    #[test]
    fn retain_existing_payload_refs_nulls_missing() {
        let keep = RawPayloadRef {
            raw_payload_id: "p-keep".into(),
            kind: RawPayloadKind::Request,
            path: PathBuf::from("payloads/p-keep.json"),
        };
        let drop = RawPayloadRef {
            raw_payload_id: "p-drop".into(),
            kind: RawPayloadKind::Request,
            path: PathBuf::from("payloads/p-drop.json"),
        };

        // InferenceStarted: keep when file exists.
        let mut event =
            RawTraceEventPayload::InferenceStarted { request_payload: Some(keep.clone()) };
        event.retain_existing_payload_refs(|r| r.raw_payload_id == "p-keep");
        assert_eq!(event.payload_refs(), vec![&keep]);

        // Drops when the exists-closure says missing.
        let mut event =
            RawTraceEventPayload::InferenceStarted { request_payload: Some(drop.clone()) };
        event.retain_existing_payload_refs(|_| false);
        assert!(event.payload_refs().is_empty(), "dangling ref was nulled");

        // No-ref variants are a no-op.
        let mut event = RawTraceEventPayload::TurnStarted;
        event.retain_existing_payload_refs(|_| false);
        assert!(matches!(event, RawTraceEventPayload::TurnStarted));
    }
}
