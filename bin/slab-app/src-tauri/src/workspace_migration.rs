//! Host-side workspace switch with agent migration (B-8 / INFRA-01).
//!
//! `switch_workspace_with_migration` interrupts every active agent thread on the
//! server, persists a project-scoped snapshot (so a future restore only resumes
//! threads that belong to the originating workspace), then switches the active
//! workspace. Returns how many threads were suspended so the UI can surface
//! "N tasks suspended".

use std::time::Duration;

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

use crate::setup::ApiEndpointConfig;

/// Per-request timeout for server calls so a migration can never hang waiting on
/// an unresponsive server.
const SERVER_FETCH_TIMEOUT_SECS: u64 = 10;

/// Result of a workspace migration (mirrors the server's `WorkspaceMigrationResponse`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationResult {
    pub project_id: String,
    pub suspended_count: u32,
}

/// Switch the active workspace, first interrupting active agent threads and
/// snapshotting them (project-scoped) on the server. Returns the migration
/// outcome so the UI can report how many tasks were suspended. Any step failure
/// aborts and surfaces the error (the caller must not assume the switch applied).
#[tauri::command]
pub fn switch_workspace_with_migration(
    api_endpoint: tauri::State<'_, ApiEndpointConfig>,
    new_root: String,
) -> Result<MigrationResult, String> {
    let new_root = new_root.trim();
    if new_root.is_empty() {
        return Err("switch_workspace_with_migration requires a non-empty new_root".to_owned());
    }

    // 1. Interrupt active threads + write a project-scoped snapshot for the
    //    current workspace (best-effort: no active threads ⇒ empty snapshot).
    let migration: MigrationResult = post_json(&api_endpoint, "v1/agents/migrate", Value::Null)?;

    // 2. Switch the server's active workspace to the new root.
    post_json::<Value>(&api_endpoint, "v1/workspace/open", workspace_open_body(new_root))?;

    Ok(migration)
}

/// Build the `/v1/workspace/open` request body for `root` (camelCase `rootPath`).
fn workspace_open_body(root: &str) -> Value {
    json!({ "rootPath": root })
}

fn post_json<T: DeserializeOwned>(
    api_endpoint: &ApiEndpointConfig,
    path: &str,
    body: Value,
) -> Result<T, String> {
    let url = format!("{}{path}", api_endpoint.api_base_url());
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(SERVER_FETCH_TIMEOUT_SECS))
        .build()
        .map_err(|error| format!("http client build failed: {error}"))?;
    let response = client
        .post(&url)
        .json(&body)
        .send()
        .map_err(|error| format!("{path} request failed: {error}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().unwrap_or_default();
        return Err(format!("{path} returned {status}: {text}"));
    }
    response.json::<T>().map_err(|error| format!("{path} decode failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{MigrationResult, workspace_open_body};
    use serde_json::json;

    #[test]
    fn workspace_open_body_uses_camel_case_root_path() {
        assert_eq!(workspace_open_body("/repo/slab"), json!({ "rootPath": "/repo/slab" }));
    }

    #[test]
    fn migration_result_decodes_camel_case_fields() {
        let payload = json!({ "projectId": "proj-1", "suspendedCount": 3 });
        let result: MigrationResult = serde_json::from_value(payload).expect("decode");
        assert_eq!(result.project_id, "proj-1");
        assert_eq!(result.suspended_count, 3);
    }
}
