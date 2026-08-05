//! Trace context that travels with an agent turn/thread so independent layers
//! can append events to one session sink without re-threading configuration.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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
    /// Root thread id owning the trace bundle. Set when a trace
    /// bundle is started so child/subagent threads can be correlated back to
    /// the same bundle. `None` for legacy contexts that predate trace bundles
    /// (backward compatible — older payloads still deserialize).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_thread_id: Option<String>,
}

impl AgentTraceContext {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            thread_id: None,
            parent_span_id: None,
            turn_index: None,
            trace_dir: None,
            root_thread_id: None,
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

    /// Attach the root thread id owning the trace bundle.
    pub fn with_root_thread_id(mut self, root_thread_id: impl Into<String>) -> Self {
        let root_thread_id = root_thread_id.into();
        self.root_thread_id = if root_thread_id.is_empty() { None } else { Some(root_thread_id) };
        self
    }

    /// A root thread is one with no parent span id.
    pub fn is_root_thread(&self) -> bool {
        self.parent_span_id.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_thread_id_is_optional_and_backward_compatible() {
        // Legacy context without root_thread_id still round-trips.
        let ctx = AgentTraceContext::new("session").with_thread("root");
        let json = serde_json::to_string(&ctx).expect("serialize");
        assert!(!json.contains("root_thread_id"), "omitted when None: {json}");

        let restored: AgentTraceContext =
            serde_json::from_str(&json).expect("deserialize legacy payload");
        assert_eq!(restored, ctx);
        assert!(restored.root_thread_id.is_none());
    }

    #[test]
    fn root_thread_id_builder_normalizes_empty() {
        let ctx = AgentTraceContext::new("session").with_root_thread_id("").with_thread("root");
        assert!(ctx.root_thread_id.is_none());

        let ctx = AgentTraceContext::new("session").with_root_thread_id("root-1");
        assert_eq!(ctx.root_thread_id.as_deref(), Some("root-1"));
    }

    #[test]
    fn is_root_thread_tracks_parent_span_id() {
        let root = AgentTraceContext::new("session").with_thread("root");
        assert!(root.is_root_thread());
        assert!(root.with_parent_span_id("").is_root_thread());

        let child =
            AgentTraceContext::new("session").with_thread("child").with_parent_span_id("parent");
        assert!(!child.is_root_thread());
    }
}
