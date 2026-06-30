//! Host-only diagnostics export (INFRA-08 / ADR-014).
//!
//! `export_diagnostics` is a Tauri command (not a `/v1/*` endpoint) that gathers
//! host-available fields into a whitelisted, secret-redacted snapshot via
//! `slab_utils::diagnostics`. The collection types enforce the whitelist at the
//! type system — forbidden data (db rows, session messages, secret values, trace
//! args) is not representable in [`DiagnosticsInput`].
//!
//! Active plugin/model ids are fetched from the running server (`/v1/plugins`,
//! `/v1/models`); if the server is unreachable the snapshot still succeeds with
//! those fields empty. Thread statistics / failed-tool-call aggregation still
//! require dedicated server endpoints and remain a follow-up.

use std::time::Duration;

use serde_json::Value;

use slab_utils::app_home::server_log_file;
use slab_utils::diagnostics::{DiagnosticsInput, build_diagnostics_snapshot};

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

    let active_plugins =
        plugins_response.as_ref().map(extract_active_plugin_ids).unwrap_or_default();
    let active_models = models_response.as_ref().map(extract_active_model_ids).unwrap_or_default();

    let empty_args: [String; 0] = [];
    let empty_threads: [slab_utils::diagnostics::ThreadStat; 0] = [];
    let empty_failed: [slab_utils::diagnostics::FailedToolCall; 0] = [];

    let input = DiagnosticsInput {
        app_version: env!("CARGO_PKG_VERSION"),
        git_sha: option_env!("SLAB_GIT_SHA"),
        os: std::env::consts::OS,
        sidecar_args: &empty_args,
        log_tail: &log_tail,
        threads: &empty_threads,
        failed_tool_calls: &empty_failed,
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
}
