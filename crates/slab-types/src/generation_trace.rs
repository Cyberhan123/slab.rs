//! Shared field set of the generation agent-trace payloads.
//!
//! Two layers emit a trace event describing the same generation request —
//! app-core's `runtime_request` (pre-dispatch view) and the runtime engine's
//! `llama_request` (dispatch view). The sampling fields they describe are one
//! contract; this module holds that field list once so the two payloads
//! cannot drift apart (they did before: each side hand-maintained its own
//! JSON literal). Layer-specific fields (model routing on the app side,
//! sampler internals on the engine side) stay with their layer and merge in
//! as extras.

use serde::Serialize;

/// The generation-sampling fields shared by both trace events.
///
/// `max_tokens` is `Option`: the app layer traces before resolution (cap may
/// be unset), the engine always carries the resolved cap.
#[derive(Debug, Serialize)]
pub struct GenerationSamplingTracePayload<'a> {
    pub prompt: &'a str,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub min_p: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub repetition_penalty: Option<f32>,
    pub session_key: Option<&'a str>,
    pub gbnf: Option<&'a str>,
    pub stop_sequences: &'a [String],
    pub thinking_budget: Option<u32>,
}

impl GenerationSamplingTracePayload<'_> {
    /// Serialize the shared fields and merge the layer-specific `extras`
    /// on top (extras win on key collision — they are the layer's own view).
    pub fn into_trace_value(
        self,
        extras: serde_json::Map<String, serde_json::Value>,
    ) -> serde_json::Value {
        // Plain strings/numbers/arrays only — serialization cannot fail
        // except for allocation, which would already have failed elsewhere.
        let mut value = serde_json::to_value(self).expect("trace payload serializes");
        let Some(map) = value.as_object_mut() else {
            return value;
        };
        for (key, extra) in extras {
            map.insert(key, extra);
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn shared_field_set_is_complete_and_extras_merge() {
        let payload = GenerationSamplingTracePayload {
            prompt: "prompt",
            max_tokens: Some(1024),
            temperature: Some(0.6),
            top_p: Some(0.95),
            top_k: Some(20),
            min_p: Some(0.0),
            presence_penalty: Some(0.0),
            repetition_penalty: Some(1.0),
            session_key: Some("session"),
            gbnf: None,
            stop_sequences: &["</think>".to_owned()],
            thinking_budget: Some(4096),
        };
        let mut extras = serde_json::Map::new();
        extras.insert("model".to_owned(), json!("Qwen3.5-9B"));

        let value = payload.into_trace_value(extras);

        // The full shared key set, exactly once, plus the layer extra.
        // (f32 fields widen to f64 on serialization — expected values built
        // the same way.)
        let expected = json!({
            "prompt": "prompt",
            "max_tokens": 1024,
            "temperature": f64::from(0.6f32),
            "top_p": f64::from(0.95f32),
            "top_k": 20,
            "min_p": 0.0,
            "presence_penalty": 0.0,
            "repetition_penalty": 1.0,
            "session_key": "session",
            "gbnf": null,
            "stop_sequences": ["</think>"],
            "thinking_budget": 4096,
            "model": "Qwen3.5-9B",
        });
        assert_eq!(value, expected);
    }
}
