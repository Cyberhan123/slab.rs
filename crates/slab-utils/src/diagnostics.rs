//! Diagnostics snapshot assembly for the host-only `export_diagnostics`
//! command (INFRA-08 / ADR-014).
//!
//! Safety design: the input types ([`ThreadStat`], [`FailedToolCall`],
//! [`DiagnosticsInput`]) deliberately cannot carry message content, tool-call
//! arguments, database rows, session transcripts, or secret values. The
//! whitelist is therefore enforced by the type system, not by runtime filtering
//! — a producer that tries to leak forbidden data will not compile.

use regex_lite::Regex;
use serde_json::{Value, json};

/// Redact common secret shapes from free-form text (log tails, error strings)
/// before including them in a diagnostics snapshot. Mirrors the patterns used
/// by the slab-server log redaction filter.
pub fn redact_secret_patterns(text: &str) -> String {
    let bearer = Regex::new(r"(?i)\b(Bearer\s+)([A-Za-z0-9._~+/-]{8,})").expect("bearer regex");
    let openai = Regex::new(r"\bsk-[A-Za-z0-9][A-Za-z0-9_-]{8,}").expect("openai regex");
    let secret_uri = Regex::new(r"secret://[A-Za-z0-9._~+/-]+").expect("secret-uri regex");
    let kv = Regex::new(
        r#"(?i)("?)(token|api[_-]?key|secret|password)("?)(\s*[=:]\s*)(["']?)([A-Za-z0-9][^"'\s,;}]{5,})(["']?)"#,
    )
    .expect("kv secret regex");

    let s = bearer.replace_all(text, "${1}<redacted>");
    let s = openai.replace_all(&s, "sk-<redacted>");
    let s = secret_uri.replace_all(&s, "secret://<redacted>");
    // Keep the key + separator + surrounding quotes; drop the value (group 6).
    let s = kv.replace_all(&s, "${1}${2}${3}${4}${5}<redacted>${7}");
    s.into_owned()
}

/// Agent thread statistics included in diagnostics (no message content).
#[derive(Debug, Clone)]
pub struct ThreadStat {
    pub thread_id: String,
    pub status: String,
    pub turn_index: u32,
    pub depth: u32,
    pub reason: Option<String>,
}

/// Failed tool call summary (tool name + error; no arguments).
#[derive(Debug, Clone)]
pub struct FailedToolCall {
    pub tool_name: String,
    pub error: String,
}

/// Whitelisted inputs for a diagnostics snapshot. Producers may populate only
/// these fields; forbidden data (db rows, session messages, secret values,
/// trace args) is not representable here.
pub struct DiagnosticsInput<'a> {
    pub app_version: &'a str,
    pub git_sha: Option<&'a str>,
    pub os: &'a str,
    /// Sidecar launch args (paths only — the host must not include secrets).
    pub sidecar_args: &'a [String],
    /// Tail of the server log (will be secret-redacted).
    pub log_tail: &'a str,
    pub threads: &'a [ThreadStat],
    pub failed_tool_calls: &'a [FailedToolCall],
    /// Resource snapshot (RSS / concurrency / token totals) — pre-redacted.
    pub resource_snapshot: Option<&'a Value>,
    /// Active plugin ids (id only — no config or secret handles).
    pub active_plugins: &'a [String],
    /// Active model ids (id only).
    pub active_models: &'a [String],
}

/// Build a whitelisted, secret-redacted diagnostics snapshot (INFRA-08).
///
/// The returned object intentionally has no field for `slab.db` contents,
/// session transcripts, raw tool-call arguments, or secret values.
pub fn build_diagnostics_snapshot(input: &DiagnosticsInput<'_>) -> Value {
    json!({
        "app_version": input.app_version,
        "git_sha": input.git_sha,
        "os": input.os,
        "sidecar_args": input.sidecar_args,
        "log_tail": redact_secret_patterns(input.log_tail),
        "agent_threads": input.threads.iter().map(|thread| json!({
            "thread_id": thread.thread_id,
            "status": thread.status,
            "turn_index": thread.turn_index,
            "depth": thread.depth,
            "reason": thread.reason,
        })).collect::<Vec<_>>(),
        "failed_tool_calls": input.failed_tool_calls.iter().map(|failed| json!({
            "tool_name": failed.tool_name,
            "error": failed.error,
        })).collect::<Vec<_>>(),
        "resource_snapshot": input.resource_snapshot,
        "active_plugins": input.active_plugins,
        "active_models": input.active_models,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_bearer_openai_and_secret_uri_patterns() {
        let text = "Authorization: Bearer abcdef1234567890 ref=sk-abcdefghijklmnopqrstuvwxyz store=secret://provider/openai";
        let redacted = redact_secret_patterns(text);

        assert!(redacted.contains("Bearer <redacted>"));
        assert!(redacted.contains("sk-<redacted>"));
        assert!(redacted.contains("secret://<redacted>"));
        assert!(!redacted.contains("abcdef1234567890"));
        assert!(!redacted.contains("secret://provider/openai"));
    }

    #[test]
    fn redacts_key_value_secret_shapes() {
        let redacted =
            redact_secret_patterns("api_key = abcdef123456 password: \"ghp_1234567890\"");

        assert!(redacted.contains("api_key = <redacted>"));
        assert!(redacted.contains("password: \"<redacted>\""));
        assert!(!redacted.contains("ghp_1234567890"));
    }

    #[test]
    fn snapshot_includes_whitelist_and_redacts_log_tail() {
        let threads = vec![ThreadStat {
            thread_id: "t1".into(),
            status: "completed".into(),
            turn_index: 3,
            depth: 0,
            reason: Some("completed".into()),
        }];
        let failed =
            vec![FailedToolCall { tool_name: "shell".into(), error: "command failed".into() }];
        let resources = json!({ "rss_mb": 123, "concurrency": 2, "tokens": 500 });

        let input = DiagnosticsInput {
            app_version: "0.1.0",
            git_sha: Some("abc123"),
            os: "windows",
            sidecar_args: &["--database-url".to_owned(), ":memory:".to_owned()],
            log_tail: " Authorization: Bearer secret-xyz1234567890 ",
            threads: &threads,
            failed_tool_calls: &failed,
            resource_snapshot: Some(&resources),
            active_plugins: &["video-subtitle-translator".to_owned()],
            active_models: &["slab-llama".to_owned()],
        };

        let snap = build_diagnostics_snapshot(&input);

        assert_eq!(snap["app_version"], "0.1.0");
        assert_eq!(snap["git_sha"], "abc123");
        assert_eq!(snap["os"], "windows");
        assert_eq!(snap["agent_threads"][0]["thread_id"], "t1");
        assert_eq!(snap["agent_threads"][0]["status"], "completed");
        // ThreadStat has no message field -> not present in the snapshot.
        assert!(snap["agent_threads"][0].get("messages").is_none());
        assert!(snap["agent_threads"][0].get("content").is_none());
        assert_eq!(snap["failed_tool_calls"][0]["tool_name"], "shell");
        // FailedToolCall has no arguments field -> not present.
        assert!(snap["failed_tool_calls"][0].get("arguments").is_none());
        assert!(snap["failed_tool_calls"][0].get("args").is_none());
        assert_eq!(snap["active_plugins"][0], "video-subtitle-translator");
        assert_eq!(snap["resource_snapshot"]["rss_mb"], 123);

        let log_tail = snap["log_tail"].as_str().expect("log tail");
        assert!(log_tail.contains("Bearer <redacted>"));
        assert!(!log_tail.contains("secret-xyz1234567890"));
    }

    #[test]
    fn snapshot_has_no_db_session_or_secret_fields() {
        let input = DiagnosticsInput {
            app_version: "0",
            git_sha: None,
            os: "linux",
            sidecar_args: &[],
            log_tail: "",
            threads: &[],
            failed_tool_calls: &[],
            resource_snapshot: None,
            active_plugins: &[],
            active_models: &[],
        };

        let snap = build_diagnostics_snapshot(&input);
        let obj = snap.as_object().expect("snapshot object");

        assert!(!obj.contains_key("slab_db"));
        assert!(!obj.contains_key("sessions"));
        assert!(!obj.contains_key("admin_api_token"));
        assert!(!obj.contains_key("provider_key"));
        assert!(!obj.contains_key("trace_args"));
    }
}
