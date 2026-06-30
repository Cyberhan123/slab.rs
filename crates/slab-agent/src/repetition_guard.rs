use std::collections::BTreeMap;

use crate::port::ParsedToolCall;

const REPETITION_THRESHOLD: usize = 3;

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
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepetitionDetected {
    pub(crate) signature: ToolCallSignature,
    pub(crate) hit_count: usize,
}

#[derive(Debug, Default)]
pub(crate) struct RepetitionGuard {
    current_signature: Option<ToolCallSignature>,
    current_hit_count: usize,
}

impl RepetitionGuard {
    pub(crate) fn observe(
        &mut self,
        signatures: &[ToolCallSignature],
    ) -> Option<RepetitionDetected> {
        for signature in signatures.iter().filter(|signature| signature.is_side_effectful()) {
            if self.current_signature.as_ref() == Some(signature) {
                self.current_hit_count = self.current_hit_count.saturating_add(1);
            } else {
                self.current_signature = Some(signature.clone());
                self.current_hit_count = 1;
            }

            if self.current_hit_count >= REPETITION_THRESHOLD {
                return Some(RepetitionDetected {
                    signature: signature.clone(),
                    hit_count: self.current_hit_count,
                });
            }
        }
        None
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

        assert_eq!(guard.observe(&calls), None);
        assert_eq!(guard.observe(&calls), None);
        let detected = guard.observe(&calls).expect("third call should be detected");

        assert_eq!(detected.hit_count, 3);
        assert_eq!(detected.signature, calls[0]);
    }

    #[test]
    fn ignores_read_only_repetition() {
        let mut guard = RepetitionGuard::default();
        let calls = vec![signature("read_file", r#"{"path":"a"}"#)];

        assert_eq!(guard.observe(&calls), None);
        assert_eq!(guard.observe(&calls), None);
        assert_eq!(guard.observe(&calls), None);
    }
}
