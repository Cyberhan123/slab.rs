use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use slab_utils::hash::{sha256_hex_bytes, verify_sha256_hex_expected};
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{InstallPluginCommand, PluginView};
use crate::error::AppCoreError;
use crate::infra::db::{PluginStateRecord, PluginStateStore};
use crate::infra::endpoint::ensure_http_base_url;
use crate::infra::plugin_runtime::{
    PluginEventBus, PluginSidecarRuntimeClient, PluginSidecarRuntimeKind, PluginSidecarTransport,
};
use slab_config::PluginJsRuntimeTransport;
use slab_plugin::{PluginCallRequest, PluginRegistry, PluginRuntime};
use slab_types::{PluginRuntimeCallRequest, PluginRuntimeFileGrant};

mod package;
mod scan;
mod validation;
mod view;

use self::package::{
    create_staging_dir, ensure_path_within, extract_plugin_pack_archive, load_package_bytes,
    locate_plugin_root, move_directory, run_bun_install, safe_remove_dir,
};
use self::scan::{
    ScannedPlugin, is_builtin_language_server_plugin_id, scan_plugin_dir, scan_plugins,
};
use self::view::{build_missing_plugin_view, build_plugin_view, is_pack_managed_source_kind};

const SOURCE_KIND_DEV: &str = "dev";
const SOURCE_KIND_IMPORT_PACK: &str = "import_pack";
const SOURCE_KIND_PACKAGE_URL: &str = "package_url";
const RUNTIME_STATUS_RUNNING: &str = "running";
const RUNTIME_STATUS_STOPPED: &str = "stopped";
const RUNTIME_STATUS_ERROR: &str = "error";
const DEFAULT_PACKAGE_SOURCE_ID: &str = "direct";

#[derive(Clone)]
pub struct PluginService {
    state: ModelState,
    registry: Arc<Mutex<Option<Arc<PluginRegistry>>>>,
    runtime: Arc<PluginRuntime>,
    js_runtime: PluginSidecarRuntimeClient,
    python_runtime: PluginSidecarRuntimeClient,
    event_bus: PluginEventBus,
}

impl PluginService {
    pub fn new(state: ModelState) -> Self {
        let runtime = ensure_http_base_url(state.config().bind_address.as_str())
            .map(PluginRuntime::with_api_base_url)
            .unwrap_or_else(|_| PluginRuntime::default());
        let event_bus = PluginEventBus::new();
        let api_base_url = ensure_http_base_url(state.config().bind_address.as_str())
            .unwrap_or_else(|_| slab_types::DESKTOP_API_ORIGIN.to_owned());
        let js_transport = match state.config().plugin_js_runtime_transport {
            PluginJsRuntimeTransport::Stdio => PluginSidecarTransport::Stdio,
            PluginJsRuntimeTransport::Uds => PluginSidecarTransport::Uds,
        };
        let js_runtime = PluginSidecarRuntimeClient::for_current_server(
            PluginSidecarRuntimeKind::JavaScript,
            js_transport,
            api_base_url.clone(),
            event_bus.clone(),
        );
        let python_runtime = PluginSidecarRuntimeClient::for_current_server(
            PluginSidecarRuntimeKind::Python,
            PluginSidecarTransport::Stdio,
            api_base_url,
            event_bus.clone(),
        );

        Self {
            state,
            registry: Arc::new(Mutex::new(None)),
            runtime: Arc::new(runtime),
            js_runtime,
            python_runtime,
            event_bus,
        }
    }

    pub fn subscribe_events(
        &self,
    ) -> tokio::sync::broadcast::Receiver<slab_types::PluginEventPayload> {
        self.event_bus.subscribe()
    }

    pub async fn list_plugins(&self) -> Result<Vec<PluginView>, AppCoreError> {
        let scans = self.scan_and_sync().await?;
        let states = self
            .state
            .store()
            .list_plugin_states()
            .await?
            .into_iter()
            .map(|record| (record.plugin_id.clone(), record))
            .collect::<HashMap<_, _>>();

        let mut rows = scans
            .iter()
            .map(|scan| build_plugin_view(scan, states.get(&scan.id)))
            .collect::<Vec<_>>();

        for (plugin_id, state) in &states {
            if is_builtin_language_server_plugin_id(plugin_id) {
                continue;
            }
            if scans.iter().any(|scan| scan.id == *plugin_id) {
                continue;
            }
            rows.push(build_missing_plugin_view(state));
        }

        rows.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(rows)
    }

    pub async fn get_plugin(&self, plugin_id: &str) -> Result<PluginView, AppCoreError> {
        self.list_plugins()
            .await?
            .into_iter()
            .find(|plugin| plugin.id == plugin_id)
            .ok_or_else(|| AppCoreError::NotFound(format!("plugin '{plugin_id}' not found")))
    }

    pub async fn install_plugin(
        &self,
        command: InstallPluginCommand,
    ) -> Result<PluginView, AppCoreError> {
        fs::create_dir_all(&self.state.config().plugins_dir).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to create plugins directory {}: {error}",
                self.state.config().plugins_dir.display()
            ))
        })?;

        let source = self.resolve_install_source(&command).await?;
        let package_bytes = load_package_bytes(&source.package_url).await?;
        if let Some(expected_hash) = source.package_sha256.as_deref() {
            let actual = sha256_hex_bytes(&package_bytes);
            if verify_sha256_hex_expected(&actual, expected_hash).is_err() {
                return Err(AppCoreError::BadRequest(format!(
                    "plugin package sha256 mismatch for '{}': expected {expected_hash}, got {actual}",
                    source.package_url
                )));
            }
        }

        self.install_plugin_pack_bytes(
            &package_bytes,
            Some(command.plugin_id.as_str()),
            SOURCE_KIND_PACKAGE_URL,
            Some(source.source_id.as_str()),
        )
        .await
    }

    pub async fn import_plugin_pack_bytes(
        &self,
        bytes: &[u8],
        source_ref: Option<&str>,
    ) -> Result<PluginView, AppCoreError> {
        self.install_plugin_pack_bytes(bytes, None, SOURCE_KIND_IMPORT_PACK, source_ref).await
    }

    async fn install_plugin_pack_bytes(
        &self,
        package_bytes: &[u8],
        expected_plugin_id: Option<&str>,
        source_kind: &str,
        source_ref: Option<&str>,
    ) -> Result<PluginView, AppCoreError> {
        fs::create_dir_all(&self.state.config().plugins_dir).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to create plugins directory {}: {error}",
                self.state.config().plugins_dir.display()
            ))
        })?;

        let staging_root = create_staging_dir(&self.state.config().plugins_dir)?;
        extract_plugin_pack_archive(package_bytes, &staging_root)?;
        let extracted_root = locate_plugin_root(&staging_root)?;
        let scanned = scan_plugin_dir(&extracted_root, source_kind)?;
        if !scanned.valid {
            return Err(AppCoreError::BadRequest(scanned.error.unwrap_or_else(|| {
                expected_plugin_id.map_or_else(
                    || "plugin pack failed validation after extraction".to_owned(),
                    |plugin_id| format!("plugin '{plugin_id}' failed validation after extraction"),
                )
            })));
        }

        let manifest = scanned
            .manifest
            .as_ref()
            .ok_or_else(|| AppCoreError::BadRequest("plugin pack is missing plugin.json".into()))?;
        if is_builtin_language_server_plugin_id(&manifest.id) {
            return Err(AppCoreError::BadRequest(format!(
                "plugin '{}' is built in and cannot be installed as a plugin",
                manifest.id
            )));
        }
        if let Some(expected_plugin_id) = expected_plugin_id
            && manifest.id != expected_plugin_id
        {
            return Err(AppCoreError::BadRequest(format!(
                "installed plugin id '{}' does not match request '{}'",
                manifest.id, expected_plugin_id
            )));
        }

        let final_dir = self.state.config().plugins_dir.join(&manifest.id);
        if final_dir.exists() {
            let state = self.state.store().get_plugin_state(&manifest.id).await?;
            if !state
                .as_ref()
                .is_some_and(|record| is_pack_managed_source_kind(record.source_kind.as_str()))
            {
                return Err(AppCoreError::BadRequest(format!(
                    "plugin '{}' already exists on disk and is not a managed pack install",
                    manifest.id
                )));
            }
            safe_remove_dir(&final_dir)?;
        }

        if extracted_root == staging_root {
            move_directory(&staging_root, &final_dir)?;
        } else {
            move_directory(&extracted_root, &final_dir)?;
            safe_remove_dir(&staging_root)?;
        }

        if final_dir.join("package.json").exists() {
            run_bun_install(&final_dir).await;
        }

        let now = Utc::now();
        self.state
            .store()
            .upsert_plugin_state(PluginStateRecord {
                plugin_id: manifest.id.clone(),
                source_kind: source_kind.to_owned(),
                source_ref: source_ref.map(str::to_owned),
                install_root: Some(final_dir.to_string_lossy().into_owned()),
                installed_version: Some(manifest.version.clone()),
                manifest_hash: scanned.manifest_hash.clone(),
                enabled: true,
                runtime_status: RUNTIME_STATUS_STOPPED.to_owned(),
                last_error: None,
                installed_at: now,
                updated_at: now,
                last_seen_at: Some(now),
                last_started_at: None,
                last_stopped_at: None,
            })
            .await?;

        self.get_plugin(&manifest.id).await
    }

    pub async fn enable_plugin(&self, plugin_id: &str) -> Result<PluginView, AppCoreError> {
        self.ensure_plugin_state(plugin_id).await?;
        self.state
            .store()
            .update_plugin_enabled(plugin_id, true, RUNTIME_STATUS_STOPPED, Utc::now())
            .await?;
        self.get_plugin(plugin_id).await
    }

    pub async fn disable_plugin(&self, plugin_id: &str) -> Result<PluginView, AppCoreError> {
        self.ensure_plugin_state(plugin_id).await?;
        self.state
            .store()
            .update_plugin_enabled(plugin_id, false, RUNTIME_STATUS_STOPPED, Utc::now())
            .await?;
        self.get_plugin(plugin_id).await
    }

    pub async fn start_plugin(&self, plugin_id: &str) -> Result<PluginView, AppCoreError> {
        self.ensure_plugin_state(plugin_id).await?;
        let now = Utc::now();
        self.state
            .store()
            .update_plugin_runtime_status(
                plugin_id,
                RUNTIME_STATUS_RUNNING,
                None,
                Some(now),
                None,
                now,
            )
            .await?;
        self.get_plugin(plugin_id).await
    }

    pub async fn stop_plugin(
        &self,
        plugin_id: &str,
        last_error: Option<String>,
    ) -> Result<PluginView, AppCoreError> {
        self.ensure_plugin_state(plugin_id).await?;
        let runtime_status =
            if last_error.is_some() { RUNTIME_STATUS_ERROR } else { RUNTIME_STATUS_STOPPED };
        let now = Utc::now();
        self.state
            .store()
            .update_plugin_runtime_status(
                plugin_id,
                runtime_status,
                last_error.as_deref(),
                None,
                Some(now),
                now,
            )
            .await?;
        self.get_plugin(plugin_id).await
    }

    pub async fn remove_plugin(&self, plugin_id: &str) -> Result<(), AppCoreError> {
        let state = self
            .state
            .store()
            .get_plugin_state(plugin_id)
            .await?
            .ok_or_else(|| AppCoreError::NotFound(format!("plugin '{plugin_id}' not found")))?;
        if !is_pack_managed_source_kind(&state.source_kind) {
            return Err(AppCoreError::BadRequest(format!(
                "plugin '{plugin_id}' is not removable because it is not a managed pack install"
            )));
        }

        let root = self.resolve_plugin_dir(plugin_id, state.install_root.as_deref())?;
        if root.exists() {
            safe_remove_dir(&root)?;
        }
        self.state.store().delete_plugin_state(plugin_id).await?;
        Ok(())
    }

    pub async fn dispatch_rpc(
        &self,
        plugin_id: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, AppCoreError> {
        self.ensure_plugin_state(plugin_id).await?;

        let registry = self.plugin_registry()?;
        registry.refresh().map_err(AppCoreError::Internal)?;
        let plugin = registry.get_plugin(plugin_id).map_err(AppCoreError::BadRequest)?;

        if let Some(js_entry) = plugin.manifest.runtime.js.as_ref() {
            let request = PluginRuntimeCallRequest {
                call_id: Uuid::new_v4().to_string(),
                plugin_id: plugin_id.to_owned(),
                root_dir: plugin.root_dir.to_string_lossy().into_owned(),
                entry: js_entry.entry.clone(),
                bundle: None,
                export_name: method.to_owned(),
                params,
                permissions: plugin.manifest.permissions.clone(),
                file_grants: Vec::<PluginRuntimeFileGrant>::new(),
                blocked_fetch_origins: Vec::new(),
            };
            return self.js_runtime.call(request).await.map(|response| response.result).map_err(
                |error| {
                    AppCoreError::BadRequest(format!(
                        "plugin `{plugin_id}` JS call `{method}` failed: {error}"
                    ))
                },
            );
        }
        if let Some(python_entry) = plugin.manifest.runtime.python.as_ref() {
            let request = PluginRuntimeCallRequest {
                call_id: Uuid::new_v4().to_string(),
                plugin_id: plugin_id.to_owned(),
                root_dir: plugin.root_dir.to_string_lossy().into_owned(),
                entry: python_entry.entry.clone(),
                bundle: python_entry.bundle.clone(),
                export_name: method.to_owned(),
                params,
                permissions: plugin.manifest.permissions.clone(),
                file_grants: Vec::<PluginRuntimeFileGrant>::new(),
                blocked_fetch_origins: Vec::new(),
            };
            return self
                .python_runtime
                .call(request)
                .await
                .map(|response| response.result)
                .map_err(|error| {
                    AppCoreError::BadRequest(format!(
                        "plugin `{plugin_id}` Python call `{method}` failed: {error}"
                    ))
                });
        }

        let input = serde_json::to_string(&params).map_err(|error| {
            AppCoreError::BadRequest(format!(
                "failed to serialize plugin params for `{plugin_id}`: {error}"
            ))
        })?;

        let request = PluginCallRequest {
            plugin_id: plugin_id.to_owned(),
            function: method.to_owned(),
            input,
        };
        let response = self.runtime.call(&plugin, &request).await.map_err(|error| {
            AppCoreError::BadRequest(format!(
                "plugin `{plugin_id}` call `{method}` failed: {error}"
            ))
        })?;

        if response.output_text.trim().is_empty() {
            return Ok(serde_json::Value::Null);
        }

        Ok(serde_json::from_str(&response.output_text).unwrap_or_else(|_| {
            tracing::debug!(
                plugin_id = plugin_id,
                method = method,
                "plugin returned non-JSON output; wrapping as JSON string"
            );
            serde_json::Value::String(response.output_text)
        }))
    }

    fn plugin_registry(&self) -> Result<Arc<PluginRegistry>, AppCoreError> {
        let mut guard = self
            .registry
            .lock()
            .map_err(|_| AppCoreError::Internal("failed to lock plugin registry".to_string()))?;

        if let Some(registry) = guard.as_ref() {
            return Ok(Arc::clone(registry));
        }

        let registry = Arc::new(
            PluginRegistry::new(self.state.config().plugins_dir.clone())
                .map_err(AppCoreError::Internal)?,
        );
        *guard = Some(Arc::clone(&registry));
        Ok(registry)
    }

    async fn ensure_plugin_state(&self, plugin_id: &str) -> Result<(), AppCoreError> {
        let scans = self.scan_and_sync().await?;
        let scan = scans
            .iter()
            .find(|scan| scan.id == plugin_id)
            .ok_or_else(|| AppCoreError::NotFound(format!("plugin '{plugin_id}' not found")))?;
        if !scan.valid {
            return Err(AppCoreError::BadRequest(format!(
                "plugin '{plugin_id}' is invalid: {}",
                scan.error.clone().unwrap_or_else(|| "unknown plugin validation error".to_owned())
            )));
        }
        if let Some(state) = self.state.store().get_plugin_state(plugin_id).await?
            && !state.enabled
        {
            return Err(AppCoreError::BadRequest(format!("plugin '{plugin_id}' is disabled")));
        }
        Ok(())
    }

    async fn scan_and_sync(&self) -> Result<Vec<ScannedPlugin>, AppCoreError> {
        let scans = scan_plugins(&self.state.config().plugins_dir)?;
        let existing_states = self
            .state
            .store()
            .list_plugin_states()
            .await?
            .into_iter()
            .map(|record| (record.plugin_id.clone(), record))
            .collect::<HashMap<_, _>>();
        let now = Utc::now();

        for scan in &scans {
            let previous = existing_states.get(&scan.id);
            self.state
                .store()
                .upsert_plugin_state(PluginStateRecord {
                    plugin_id: scan.id.clone(),
                    source_kind: previous
                        .map(|record| record.source_kind.clone())
                        .unwrap_or_else(|| scan.source_kind.clone()),
                    source_ref: previous.and_then(|record| record.source_ref.clone()),
                    install_root: Some(scan.root_dir.to_string_lossy().into_owned()),
                    installed_version: scan
                        .manifest
                        .as_ref()
                        .map(|manifest| manifest.version.clone()),
                    manifest_hash: scan.manifest_hash.clone(),
                    enabled: previous.map(|record| record.enabled).unwrap_or(true),
                    runtime_status: if scan.valid {
                        previous
                            .map(|record| record.runtime_status.clone())
                            .unwrap_or_else(|| RUNTIME_STATUS_STOPPED.to_owned())
                    } else {
                        RUNTIME_STATUS_ERROR.to_owned()
                    },
                    last_error: if scan.valid {
                        previous.and_then(|record| record.last_error.clone())
                    } else {
                        scan.error.clone()
                    },
                    installed_at: previous.map(|record| record.installed_at).unwrap_or(now),
                    updated_at: now,
                    last_seen_at: Some(now),
                    last_started_at: previous.and_then(|record| record.last_started_at),
                    last_stopped_at: previous.and_then(|record| record.last_stopped_at),
                })
                .await?;
        }

        Ok(scans)
    }

    async fn resolve_install_source(
        &self,
        command: &InstallPluginCommand,
    ) -> Result<ResolvedInstallSource, AppCoreError> {
        if let Some(package_url) = command.package_url.clone() {
            return Ok(ResolvedInstallSource {
                source_id: command
                    .source_id
                    .clone()
                    .unwrap_or_else(|| DEFAULT_PACKAGE_SOURCE_ID.to_owned()),
                package_url,
                package_sha256: command.package_sha256.clone(),
            });
        }

        Err(AppCoreError::BadRequest(format!(
            "plugin '{}' install requires an explicit packageUrl",
            command.plugin_id
        )))
    }

    fn resolve_plugin_dir(
        &self,
        plugin_id: &str,
        install_root: Option<&str>,
    ) -> Result<PathBuf, AppCoreError> {
        let root = install_root
            .map(PathBuf::from)
            .unwrap_or_else(|| self.state.config().plugins_dir.join(plugin_id));
        ensure_path_within(&root, &self.state.config().plugins_dir)?;
        Ok(root)
    }
}

#[derive(Debug, Clone)]
struct ResolvedInstallSource {
    source_id: String,
    package_url: String,
    package_sha256: Option<String>,
}

#[cfg(test)]
#[cfg(test)]
mod tests;
