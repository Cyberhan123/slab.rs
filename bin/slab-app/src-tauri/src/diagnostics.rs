//! Host-only diagnostics export (INFRA-08 / ADR-014).
//!
//! `export_diagnostics` is a Tauri command (not a `/v1/*` endpoint) that gathers
//! host-available fields into a whitelisted, secret-redacted snapshot via
//! `slab_utils::diagnostics`. The collection types enforce the whitelist at the
//! type system — forbidden data (db rows, session messages, secret values, trace
//! args) is not representable in [`DiagnosticsInput`].

use slab_utils::app_home::server_log_file;
use slab_utils::diagnostics::{DiagnosticsInput, build_diagnostics_snapshot};

/// Maximum bytes of the server log tail to include (the snapshot redacts secrets
/// before inclusion).
const LOG_TAIL_BYTES: usize = 64 * 1024;

/// Export a diagnostics snapshot for bug reports / support.
///
/// Currently gathers the host-side fields (version, OS, redacted server-log
/// tail). Thread statistics, resource snapshot, and active plugin/model lists
/// require a server API call and are intentionally left empty here — they will
/// be populated by a follow-up that fetches them from `/v1/agents` / `/v1/tasks`.
#[tauri::command]
pub fn export_diagnostics() -> Result<serde_json::Value, String> {
    let log_tail = read_log_tail(LOG_TAIL_BYTES).unwrap_or_default();
    let empty_args: [String; 0] = [];
    let empty_threads: [slab_utils::diagnostics::ThreadStat; 0] = [];
    let empty_failed: [slab_utils::diagnostics::FailedToolCall; 0] = [];
    let empty_ids: [String; 0] = [];

    let input = DiagnosticsInput {
        app_version: env!("CARGO_PKG_VERSION"),
        git_sha: option_env!("SLAB_GIT_SHA"),
        os: std::env::consts::OS,
        sidecar_args: &empty_args,
        log_tail: &log_tail,
        threads: &empty_threads,
        failed_tool_calls: &empty_failed,
        resource_snapshot: None,
        active_plugins: &empty_ids,
        active_models: &empty_ids,
    };

    Ok(build_diagnostics_snapshot(&input))
}

/// Read the trailing `max_bytes` of the server log, if present.
fn read_log_tail(max_bytes: usize) -> Option<String> {
    let path = server_log_file();
    let bytes = std::fs::read(&path).ok()?;
    let start = bytes.len().saturating_sub(max_bytes);
    Some(String::from_utf8_lossy(&bytes[start..]).into_owned())
}
