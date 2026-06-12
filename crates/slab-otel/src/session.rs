use serde::Serialize;
use serde_json::Value;

/// Emits session-scoped business telemetry through the active tracing subscriber.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionTelemetry {
    session_id: String,
    thread_id: Option<String>,
    turn_index: Option<u32>,
    capture_content: bool,
}

impl SessionTelemetry {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            thread_id: None,
            turn_index: None,
            capture_content: false,
        }
    }

    pub fn with_thread(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }

    pub fn with_turn(mut self, turn_index: u32) -> Self {
        self.turn_index = Some(turn_index);
        self
    }

    pub fn with_capture_content(mut self, capture_content: bool) -> Self {
        self.capture_content = capture_content;
        self
    }

    pub fn capture_content(&self) -> bool {
        self.capture_content
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn thread_id(&self) -> Option<&str> {
        self.thread_id.as_deref()
    }

    pub fn turn_index(&self) -> Option<u32> {
        self.turn_index
    }

    pub fn emit_event(
        &self,
        source: impl AsRef<str>,
        event: impl AsRef<str>,
        payload: impl Serialize,
    ) {
        let payload_json = serde_json::to_value(payload).unwrap_or(Value::Null);
        tracing::info!(
            target: "slab_otel::session",
            session_id = %self.session_id,
            thread_id = ?self.thread_id,
            turn_index = ?self.turn_index,
            source = source.as_ref(),
            event = event.as_ref(),
            payload = %payload_json,
            "session telemetry event"
        );
    }

    pub fn emit_gen_ai_event(&self, event: impl AsRef<str>, payload: impl Serialize) {
        self.emit_event("gen_ai", event, payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_scope_builder_sets_optional_fields() {
        let telemetry = SessionTelemetry::new("session")
            .with_thread("thread")
            .with_turn(2)
            .with_capture_content(true);

        assert_eq!(telemetry.session_id(), "session");
        assert_eq!(telemetry.thread_id(), Some("thread"));
        assert_eq!(telemetry.turn_index(), Some(2));
        assert!(telemetry.capture_content());
    }
}
