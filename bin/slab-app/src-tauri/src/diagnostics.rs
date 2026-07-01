//! Host-only diagnostics export (INFRA-08 / ADR-014).
//!
//! `export_diagnostics` is a Tauri command (not a `/v1/*` endpoint) that gathers
//! host-available fields into a whitelisted, secret-redacted snapshot via
//! `slab_utils::diagnostics`. The collection types enforce the whitelist at the
//! type system — forbidden data (db rows, session messages, secret values, trace
//! args) is not representable in [`DiagnosticsInput`].
//!
//! Active plugin/model ids are fetched from the running server (`/v1/plugins`,
//! `/v1/models`), as are recent agent thread stats + failed tool calls
//! (`/v1/system/diagnostics/agent-stats`); if the server is unreachable the
//! snapshot still succeeds with those fields empty.

use std::time::Duration;

use serde_json::Value;

use slab_utils::app_home::server_log_file;
use slab_utils::diagnostics::{
    DiagnosticsInput, FailedToolCall, ThreadStat, build_diagnostics_snapshot,
};

use crate::setup::ApiEndpointConfig;

/// Maximum bytes of the server log tail to include (the snapshot redacts secrets
/// before inclusion).
const LOG_TAIL_BYTES: usize = 64 * 1024;

/// Per-request timeout for server fetches so diagnostics can never hang waiting
/// on an unresponsive server.
const SERVER_FETCH_TIMEOUT_SECS: u64 = 5;

/// Export a diagnostics snapshot for bug reports / support.
#[tauri::command]
pub fn export_diagnostics(
    api_endpoint: tauri::State<'_, ApiEndpointConfig>,
) -> Result<Value, String> {
    let log_tail = read_log_tail(LOG_TAIL_BYTES).unwrap_or_default();
    let plugins_response = fetch_json(&api_endpoint, "v1/plugins");
    let models_response = fetch_json(&api_endpoint, "v1/models");
    let agent_stats_response = fetch_json(&api_endpoint, "v1/system/diagnostics/agent-stats");

    let active_plugins =
        plugins_response.as_ref().map(extract_active_plugin_ids).unwrap_or_default();
    let active_models = models_response.as_ref().map(extract_active_model_ids).unwrap_or_default();
    let threads = agent_stats_response.as_ref().map(extract_thread_stats).unwrap_or_default();
    let failed_tool_calls =
        agent_stats_response.as_ref().map(extract_failed_tool_calls).unwrap_or_default();

    let empty_args: [String; 0] = [];

    let input = DiagnosticsInput {
        app_version: env!("CARGO_PKG_VERSION"),
        git_sha: option_env!("SLAB_GIT_SHA"),
        os: std::env::consts::OS,
        sidecar_args: &empty_args,
        log_tail: &log_tail,
        threads: &threads,
        failed_tool_calls: &failed_tool_calls,
        resource_snapshot: None,
        active_plugins: &active_plugins,
        active_models: &active_models,
    };

    Ok(build_diagnostics_snapshot(&input))
}

/// Fetch JSON from the server. Returns `None` if the server is unreachable or the
/// response is not valid JSON — diagnostics must never fail just because the
/// server is down.
fn fetch_json(api_endpoint: &ApiEndpointConfig, path: &str) -> Option<Value> {
    let url = format!("{}{path}", api_endpoint.api_base_url());
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(SERVER_FETCH_TIMEOUT_SECS))
        .build()
        .ok()?;
    let response = client.get(&url).send().ok()?;
    response.json::<Value>().ok()
}

/// Read the trailing `max_bytes` of the server log, if present.
fn read_log_tail(max_bytes: usize) -> Option<String> {
    let path = server_log_file();
    let bytes = std::fs::read(&path).ok()?;
    let start = bytes.len().saturating_sub(max_bytes);
    Some(String::from_utf8_lossy(&bytes[start..]).into_owned())
}

/// Extract ids of active (non-disabled) plugins from a `/v1/plugins` response.
/// Plugins with no `enabled` field are treated as active; only explicitly
/// `enabled: false` plugins are dropped.
fn extract_active_plugin_ids(response: &Value) -> Vec<String> {
    let Some(items) = response.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?.to_owned();
            let disabled = item.get("enabled").and_then(Value::as_bool) == Some(false);
            if disabled { None } else { Some(id) }
        })
        .collect()
}

/// Extract ids of ready (downloaded + usable) models from a `/v1/models`
/// response. Models that are `not_downloaded`, `downloading`, or `error` are
/// excluded.
fn extract_active_model_ids(response: &Value) -> Vec<String> {
    let Some(items) = response.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?.to_owned();
            let status = item.get("status").and_then(Value::as_str).unwrap_or("");
            // "ready" == downloaded + usable; "loaded" is the runtime-loaded form.
            if matches!(status, "ready" | "loaded") { Some(id) } else { None }
        })
        .collect()
}

/// Extract recent agent thread stats from a `/v1/system/diagnostics/agent-stats`
/// response. Items missing `thread_id` are skipped; other fields default safely.
fn extract_thread_stats(response: &Value) -> Vec<ThreadStat> {
    let Some(items) = response.get("threads").and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let thread_id = item.get("thread_id")?.as_str()?.to_owned();
            let status = item.get("status").and_then(Value::as_str).unwrap_or("").to_owned();
            let turn_index = item.get("turn_index").and_then(Value::as_u64).unwrap_or(0) as u32;
            let depth = item.get("depth").and_then(Value::as_u64).unwrap_or(0) as u32;
            let reason = item
                .get("reason")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned);
            Some(ThreadStat { thread_id, status, turn_index, depth, reason })
        })
        .collect()
}

/// Extract recent failed tool calls from a `/v1/system/diagnostics/agent-stats`
/// response. Items missing `tool_name` are skipped; `error` defaults to empty.
fn extract_failed_tool_calls(response: &Value) -> Vec<FailedToolCall> {
    let Some(items) = response.get("failed_tool_calls").and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let tool_name = item.get("tool_name")?.as_str()?.to_owned();
            let error = item.get("error").and_then(Value::as_str).unwrap_or("").to_owned();
            Some(FailedToolCall { tool_name, error })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_active_plugin_ids_drops_explicitly_disabled() {
        let response = json!([
            { "id": "p1", "enabled": true },
            { "id": "p2", "enabled": false },
            { "id": "p3" }
        ]);
        let ids = extract_active_plugin_ids(&response);

        // p1 enabled; p2 disabled (dropped); p3 no field (treated as active).
        assert_eq!(ids, vec!["p1".to_owned(), "p3".to_owned()]);
    }

    #[test]
    fn extract_active_plugin_ids_handles_non_array_and_missing_ids() {
        assert!(extract_active_plugin_ids(&json!({})).is_empty());
        // Items missing `id` are skipped, not panicked.
        let ids = extract_active_plugin_ids(&json!([{ "enabled": true }, { "id": "ok" }]));
        assert_eq!(ids, vec!["ok".to_owned()]);
    }

    #[test]
    fn extract_active_model_ids_keeps_ready_or_loaded_only() {
        let response = json!([
            { "id": "m1", "status": "ready" },
            { "id": "m2", "status": "not_downloaded" },
            { "id": "m3", "status": "downloading" },
            { "id": "m4", "status": "error" },
            { "id": "m5", "status": "loaded" }
        ]);
        let ids = extract_active_model_ids(&response);

        assert_eq!(ids, vec!["m1".to_owned(), "m5".to_owned()]);
    }

    #[test]
    fn extract_active_model_ids_handles_non_array() {
        assert!(extract_active_model_ids(&json!("not-an-array")).is_empty());
    }

    #[test]
    fn extract_thread_stats_maps_whitelist_fields_and_skips_invalid() {
        let response = json!({
            "threads": [
                { "thread_id": "t1", "status": "interrupted", "turn_index": 4, "depth": 1, "reason": "max_turns_reached" },
                { "thread_id": "t2", "status": "completed", "turn_index": 2, "depth": 0 },
                { "status": "running" } // missing thread_id ⇒ skipped
            ]
        });
        let stats = extract_thread_stats(&response);

        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].thread_id, "t1");
        assert_eq!(stats[0].turn_index, 4);
        assert_eq!(stats[0].depth, 1);
        assert_eq!(stats[0].reason.as_deref(), Some("max_turns_reached"));
        assert_eq!(stats[1].thread_id, "t2");
        assert!(stats[1].reason.is_none());
    }

    #[test]
    fn extract_thread_stats_handles_missing_threads_field() {
        assert!(extract_thread_stats(&json!({})).is_empty());
        assert!(extract_thread_stats(&json!({ "threads": "oops" })).is_empty());
    }

    #[test]
    fn extract_failed_tool_calls_maps_tool_name_and_error() {
        let response = json!({
            "failed_tool_calls": [
                { "tool_name": "shell", "error": "exit 1" },
                { "error": "no name" } // missing tool_name ⇒ skipped
            ]
        });
        let failed = extract_failed_tool_calls(&response);

        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].tool_name, "shell");
        assert_eq!(failed[0].error, "exit 1");
    }
}
