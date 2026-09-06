use std::collections::{BTreeMap, VecDeque};

use crate::port::ParsedToolCall;

/// Same-signature occurrences within the sliding window that trigger a strike.
const REPETITION_THRESHOLD: usize = 3;
/// How many recent side-effectful signatures the guard remembers. Oscillating
/// loops (A B A B …) never stack consecutively, so a window — not a
/// consecutive-run counter — is what detects them; ~10 keeps the memory
/// bounded while letting a healthy agent revisit a call after intervening
/// work without tripping.
const REPETITION_WINDOW: usize = 10;
/// The strike count that terminates the thread. Strike 1 only injects a
/// developer warning and lets the run continue (the model gets one chance to
/// change strategy); the NEXT detection ends the run.
pub(crate) const REPETITION_TERMINATION_STRIKE: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ToolCallSignature {
    tool_name: String,
    arguments: String,
}

impl ToolCallSignature {
    pub(crate) fn new(tool_call: &ParsedToolCall) -> Self {
        let arguments = canonicalize_arguments(&tool_call.arguments);
        Self { tool_name: tool_call.name.clone(), arguments }
    }

    pub(crate) fn tool_name(&self) -> &str {
        &self.tool_name
    }

    /// The canonical (key-sorted) JSON arguments string.
    pub(crate) fn arguments_json(&self) -> &str {
        &self.arguments
    }

    pub(crate) fn as_trace_key(&self) -> String {
        format!("{}:{}", self.tool_name, self.arguments)
    }

    pub(crate) fn signature_hash(&self) -> String {
        format!("{:016x}", stable_hash64(&self.as_trace_key()))
    }

    fn is_side_effectful(&self) -> bool {
        !matches!(
            self.tool_name.as_str(),
            "read_file"
                | "list_dir"
                | "file_glob"
                | "grep"
                | "web_search"
                | "mcp_list_tools"
                | "git_status"
                | "git_diff"
                | "fs_watch"
                | "task_status"
                | "task_output"
                | "task_stop"
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepetitionDetected {
    pub(crate) signature: ToolCallSignature,
    pub(crate) hit_count: usize,
    /// 1-based strike number for this thread run: strike 1 warns, strike
    /// [`REPETITION_TERMINATION_STRIKE`] and beyond terminate.
    pub(crate) strike: u32,
}

#[derive(Debug, Default)]
pub(crate) struct RepetitionGuard {
    window: VecDeque<ToolCallSignature>,
    strikes: u32,
}

impl RepetitionGuard {
    pub(crate) fn observe(
        &mut self,
        signatures: &[ToolCallSignature],
    ) -> Option<RepetitionDetected> {
        for signature in signatures.iter().filter(|signature| signature.is_side_effectful()) {
            self.window.push_back(signature.clone());
            while self.window.len() > REPETITION_WINDOW {
                self.window.pop_front();
            }
            // Count within the sliding window (not consecutively): an
            // oscillation A B A B A reaches the threshold on its third A.
            let hit_count = self.window.iter().filter(|entry| *entry == signature).count();
            if hit_count >= REPETITION_THRESHOLD {
                self.strikes = self.strikes.saturating_add(1);
                return Some(RepetitionDetected {
                    signature: signature.clone(),
                    hit_count,
                    strike: self.strikes,
                });
            }
        }
        None
    }

    /// Drop the remembered window after a non-terminating strike: the model
    /// starts from a clean slate, so the termination strike requires a fresh
    /// full threshold of repetitions rather than inheriting the old counts.
    pub(crate) fn clear_window(&mut self) {
        self.window.clear();
    }
}

fn canonicalize_arguments(arguments: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(arguments) else {
        return arguments.to_owned();
    };
    serde_json::to_string(&canonicalize_value(value)).unwrap_or_else(|_| arguments.to_owned())
}

fn stable_hash64(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn canonicalize_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(canonicalize_value).collect())
        }
        serde_json::Value::Object(values) => serde_json::Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, canonicalize_value(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        value => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signature(name: &str, arguments: &str) -> ToolCallSignature {
        ToolCallSignature::new(&ParsedToolCall {
            id: "call".to_owned(),
            name: name.to_owned(),
            arguments: arguments.to_owned(),
        })
    }

    #[test]
    fn canonicalizes_json_arguments_recursively() {
        assert_eq!(
            signature("write_file", r#"{"z":1,"a":{"b":2,"a":1}}"#).as_trace_key(),
            r#"write_file:{"a":{"a":1,"b":2},"z":1}"#
        );
    }

    #[test]
    fn detects_third_repeated_side_effect_signature() {
        let mut guard = RepetitionGuard::default();
        let calls = vec![signature("write_file", r#"{"path":"a","content":"x"}"#)];

        assert_eq!(guard.observe(&calls).map(|d| d.strike), None);
        assert_eq!(guard.observe(&calls).map(|d| d.strike), None);
        let detected = guard.observe(&calls).expect("third call should be detected");

        assert_eq!(detected.hit_count, 3);
        assert_eq!(detected.strike, 1);
        assert_eq!(detected.signature, calls[0]);
    }

    #[test]
    fn ignores_read_only_repetition() {
        let mut guard = RepetitionGuard::default();

        for name in ["read_file", "task_status", "task_output", "task_stop", "fs_watch"] {
            let calls = vec![signature(name, r#"{"id":"t1"}"#)];
            assert_eq!(guard.observe(&calls), None);
            assert_eq!(guard.observe(&calls), None);
            assert_eq!(guard.observe(&calls), None, "{name} polling is exempt");
        }
    }

    /// The oscillation the consecutive-only tracker could never see: A B A B A
    /// reaches the threshold inside the sliding window.
    #[test]
    fn detects_oscillating_signatures_within_the_window() {
        let mut guard = RepetitionGuard::default();
        let a = vec![signature("write_file", r#"{"path":"a"}"#)];
        let b = vec![signature("write_file", r#"{"path":"b"}"#)];

        assert_eq!(guard.observe(&a), None);
        assert_eq!(guard.observe(&b), None);
        assert_eq!(guard.observe(&a), None);
        assert_eq!(guard.observe(&b), None);
        let detected = guard.observe(&a).expect("third A inside the window triggers");
        assert_eq!(detected.hit_count, 3);
        assert_eq!(detected.signature, a[0]);
    }

    /// Strike semantics: the first trigger reports strike 1; after the window
    /// is cleared the same repetition must rebuild a full threshold before
    /// strike 2 (which terminates — see REPETITION_TERMINATION_STRIKE).
    #[test]
    fn second_strike_requires_a_fresh_window() {
        let mut guard = RepetitionGuard::default();
        let calls = vec![signature("shell", r#"{"command":"ls"}"#)];

        let first = guard.observe(&calls).or(None);
        assert!(first.is_none());
        let first = guard.observe(&calls).or(None);
        assert!(first.is_none());
        let first = guard.observe(&calls).expect("threshold reached");
        assert_eq!(first.strike, 1);

        guard.clear_window();
        // One/two post-warning repeats alone do not re-trigger.
        assert_eq!(guard.observe(&calls).map(|d| d.strike), None);
        assert_eq!(guard.observe(&calls).map(|d| d.strike), None);
        let second = guard.observe(&calls).expect("fresh threshold re-triggers");
        assert_eq!(second.strike, 2);
    }

    /// The window is bounded: a signature that aged out no longer counts, so
    /// long healthy runs that revisit an old call do not trip the guard.
    #[test]
    fn window_is_bounded_and_old_signatures_age_out() {
        let mut guard = RepetitionGuard::default();
        let a = vec![signature("shell", r#"{"command":"ls -la"}"#)];
        assert_eq!(guard.observe(&a), None);
        assert_eq!(guard.observe(&a), None);

        // Flood the window with distinct calls so both A occurrences age out.
        for index in 0..12 {
            let filler = vec![signature("shell", &format!(r#"{{"command":"echo {index}"}}"#))];
            assert_eq!(guard.observe(&filler), None);
        }

        // A re-visit starts from zero inside the fresh window.
        assert_eq!(guard.observe(&a), None);
    }
}
