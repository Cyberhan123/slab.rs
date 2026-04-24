use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};

use chrono::Utc;
use reqwest::Url;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zip::ZipArchive;

use crate::context::ModelState;
use crate::domain::models::{InstallPluginCommand, PluginMarketView, PluginView};
use crate::error::AppCoreError;
use crate::infra::db::{PluginStateRecord, PluginStateStore};
use slab_types::plugin::{
    PluginAgentCapabilityContribution, PluginCommandContribution, PluginManifest,
    PluginNetworkMode, PluginPermissionsManifest, PluginRouteContribution,
    PluginSettingsContribution, PluginSidebarContribution,
};

const SOURCE_KIND_DEV: &str = "dev";
const SOURCE_KIND_IMPORT_PACK: &str = "import_pack";
const SOURCE_KIND_MARKET_PACK: &str = "market_pack";
const RUNTIME_STATUS_RUNNING: &str = "running";
const RUNTIME_STATUS_STOPPED: &str = "stopped";
const RUNTIME_STATUS_ERROR: &str = "error";
const DEFAULT_MARKET_SOURCE_ID: &str = "default";
const IGNORED_PLUGIN_ROOT_NAMES: &[&str] = &["dist", ".git", "node_modules"];

#[derive(Clone)]
pub struct PluginService {
    state: ModelState,
}

impl PluginService {
    pub fn new(state: ModelState) -> Self {
        Self { state }
    }

    pub async fn list_plugins(&self) -> Result<Vec<PluginView>, AppCoreError> {
        let market = self.fetch_market_plugins().await?;
        let market_map =
            market.iter().map(|item| (item.id.clone(), item.clone())).collect::<HashMap<_, _>>();

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
            .map(|scan| build_plugin_view(scan, states.get(&scan.id), market_map.get(&scan.id)))
            .collect::<Vec<_>>();

        for (plugin_id, state) in &states {
            if scans.iter().any(|scan| scan.id == *plugin_id) {
                continue;
            }
            rows.push(build_missing_plugin_view(state, market_map.get(plugin_id)));
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

    pub async fn list_market(&self) -> Result<Vec<PluginMarketView>, AppCoreError> {
        let market = self.fetch_market_plugins().await?;
        let scans = self.scan_and_sync().await?;
        let states = self
            .state
            .store()
            .list_plugin_states()
            .await?
            .into_iter()
            .map(|record| (record.plugin_id.clone(), record))
            .collect::<HashMap<_, _>>();
        let scan_map =
            scans.into_iter().map(|plugin| (plugin.id.clone(), plugin)).collect::<HashMap<_, _>>();

        let mut rows = market
            .into_iter()
            .map(|item| {
                let installed_version = scan_map
                    .get(&item.id)
                    .and_then(|scan| {
                        scan.manifest.as_ref().map(|manifest| manifest.version.clone())
                    })
                    .or_else(|| {
                        states.get(&item.id).and_then(|state| state.installed_version.clone())
                    });
                let enabled = states.get(&item.id).map(|state| state.enabled).unwrap_or(true);
                let update_available = installed_version
                    .as_deref()
                    .map(|version| version != item.version)
                    .unwrap_or(false);

                PluginMarketView {
                    source_id: item.source_id,
                    id: item.id,
                    name: item.name,
                    version: item.version,
                    description: item.description,
                    package_url: item.package_url,
                    package_sha256: item.package_sha256,
                    homepage: item.homepage,
                    tags: item.tags,
                    installed_version,
                    enabled,
                    update_available,
                }
            })
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(rows)
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
            let actual = hash_bytes_hex(&package_bytes);
            if actual != expected_hash {
                return Err(AppCoreError::BadRequest(format!(
                    "plugin package sha256 mismatch for '{}': expected {expected_hash}, got {actual}",
                    source.package_url
                )));
            }
        }

        self.install_plugin_pack_bytes(
            &package_bytes,
            Some(command.plugin_id.as_str()),
            SOURCE_KIND_MARKET_PACK,
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
                    |plugin_id| {
                        format!("plugin '{plugin_id}' failed validation after extraction")
                    },
                )
            })));
        }

        let manifest = scanned.manifest.as_ref().ok_or_else(|| {
            AppCoreError::BadRequest("plugin pack is missing plugin.json".into())
        })?;
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
                    .unwrap_or_else(|| DEFAULT_MARKET_SOURCE_ID.to_owned()),
                package_url,
                package_sha256: command.package_sha256.clone(),
            });
        }

        let market = self.fetch_market_plugins().await?;
        let item = market
            .into_iter()
            .find(|item| {
                item.id == command.plugin_id
                    && command
                        .source_id
                        .as_ref()
                        .map(|source_id| source_id == &item.source_id)
                        .unwrap_or(true)
                    && command
                        .version
                        .as_ref()
                        .map(|version| version == &item.version)
                        .unwrap_or(true)
            })
            .ok_or_else(|| {
                AppCoreError::NotFound(format!(
                    "market plugin '{}' was not found in the configured catalog",
                    command.plugin_id
                ))
            })?;

        Ok(ResolvedInstallSource {
            source_id: item.source_id,
            package_url: item.package_url,
            package_sha256: item.package_sha256,
        })
    }

    async fn fetch_market_plugins(&self) -> Result<Vec<RemoteMarketPluginItem>, AppCoreError> {
        let Some(source_url) = self.state.config().plugin_market_url.as_deref() else {
            return Ok(Vec::new());
        };

        let raw = load_market_bytes(source_url).await?;
        let document: RemoteMarketDocument = serde_json::from_slice(&raw).map_err(|error| {
            AppCoreError::BadRequest(format!("failed to parse plugin market catalog: {error}"))
        })?;

        match document {
            RemoteMarketDocument::Catalog(catalog) => catalog
                .plugins
                .into_iter()
                .map(|plugin| plugin.into_market_plugin(catalog.source_id.clone(), source_url))
                .collect(),
            RemoteMarketDocument::Items(items) => items
                .into_iter()
                .map(|plugin| {
                    plugin.into_market_plugin(DEFAULT_MARKET_SOURCE_ID.to_owned(), source_url)
                })
                .collect(),
        }
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
struct ScannedPlugin {
    id: String,
    root_dir: PathBuf,
    source_kind: String,
    valid: bool,
    error: Option<String>,
    manifest: Option<PluginManifest>,
    manifest_hash: Option<String>,
}

#[derive(Debug, Clone)]
struct ResolvedInstallSource {
    source_id: String,
    package_url: String,
    package_sha256: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteMarketCatalog {
    #[serde(default = "default_market_source_id")]
    source_id: String,
    #[serde(default)]
    plugins: Vec<RemoteMarketItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteMarketItem {
    id: String,
    name: String,
    version: String,
    #[serde(default)]
    description: Option<String>,
    package_url: String,
    #[serde(default)]
    package_sha256: Option<String>,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

impl RemoteMarketItem {
    fn into_market_plugin(
        self,
        source_id: String,
        catalog_source: &str,
    ) -> Result<RemoteMarketPluginItem, AppCoreError> {
        Ok(RemoteMarketPluginItem {
            source_id,
            id: self.id,
            name: self.name,
            version: self.version,
            description: self.description,
            package_url: resolve_market_package_url(catalog_source, &self.package_url)?,
            package_sha256: self.package_sha256,
            homepage: self.homepage,
            tags: self.tags,
        })
    }
}

#[derive(Debug, Clone)]
struct RemoteMarketPluginItem {
    source_id: String,
    id: String,
    name: String,
    version: String,
    description: Option<String>,
    package_url: String,
    package_sha256: Option<String>,
    homepage: Option<String>,
    tags: Vec<String>,
}

fn resolve_market_package_url(
    catalog_source: &str,
    package_url: &str,
) -> Result<String, AppCoreError> {
    let package_url = package_url.trim();
    if package_url.is_empty() {
        return Err(AppCoreError::BadRequest(
            "plugin market item is missing a packageUrl".to_owned(),
        ));
    }

    let package_path = Path::new(package_url);
    if package_path.is_absolute() {
        return Ok(package_url.to_owned());
    }

    if let Ok(url) = Url::parse(package_url) {
        return Ok(url.to_string());
    }

    if let Ok(base_url) = Url::parse(catalog_source) {
        return base_url.join(package_url).map(|url| url.to_string()).map_err(|error| {
            AppCoreError::BadRequest(format!(
                "failed to resolve plugin package URL '{package_url}' against '{catalog_source}': {error}"
            ))
        });
    }

    let catalog_path = Path::new(catalog_source);
    let base_dir = catalog_path.parent().unwrap_or_else(|| Path::new("."));
    Ok(base_dir.join(package_url).to_string_lossy().into_owned())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RemoteMarketDocument {
    Catalog(RemoteMarketCatalog),
    Items(Vec<RemoteMarketItem>),
}

fn default_market_source_id() -> String {
    DEFAULT_MARKET_SOURCE_ID.to_owned()
}

fn build_plugin_view(
    scan: &ScannedPlugin,
    state: Option<&PluginStateRecord>,
    market: Option<&RemoteMarketPluginItem>,
) -> PluginView {
    let manifest = scan.manifest.as_ref();
    let manifest_compatibility =
        manifest.map(|value| serde_json::to_value(&value.compatibility).unwrap_or(Value::Null));
    let manifest_contributions =
        manifest.map(|value| serde_json::to_value(&value.contributes).unwrap_or(Value::Null));
    let manifest_permissions =
        manifest.map(|value| serde_json::to_value(&value.permissions).unwrap_or(Value::Null));
    let installed_version = manifest
        .map(|value| value.version.clone())
        .or_else(|| state.and_then(|record| record.installed_version.clone()));
    let available_version = market.map(|item| item.version.clone());
    let update_available = matches!(
        (installed_version.as_deref(), available_version.as_deref()),
        (Some(installed), Some(available)) if installed != available
    );

    PluginView {
        id: scan.id.clone(),
        name: manifest.map(|value| value.name.clone()).unwrap_or_else(|| scan.id.clone()),
        version: manifest.map(|value| value.version.clone()).unwrap_or_else(|| {
            state
                .and_then(|record| record.installed_version.clone())
                .unwrap_or_else(|| "invalid".to_owned())
        }),
        valid: scan.valid,
        error: scan.error.clone(),
        manifest_version: manifest.map(|value| value.manifest_version).unwrap_or(0),
        compatibility: manifest_compatibility.unwrap_or(Value::Null),
        ui_entry: manifest.map(|value| value.runtime.ui.entry.clone()),
        has_wasm: manifest.and_then(|value| value.runtime.wasm.as_ref()).is_some(),
        network_mode: manifest
            .map(|value| network_mode_label(&value.permissions.network.mode).to_owned())
            .unwrap_or_else(|| "blocked".to_owned()),
        allow_hosts: manifest
            .map(|value| value.permissions.network.allow_hosts.clone())
            .unwrap_or_default(),
        contributions: manifest_contributions.unwrap_or(Value::Null),
        permissions: manifest_permissions.unwrap_or(Value::Null),
        source_kind: state
            .map(|record| record.source_kind.clone())
            .unwrap_or_else(|| scan.source_kind.clone()),
        source_ref: state.and_then(|record| record.source_ref.clone()),
        install_root: state
            .and_then(|record| record.install_root.clone())
            .or_else(|| Some(scan.root_dir.to_string_lossy().into_owned())),
        installed_version,
        manifest_hash: scan
            .manifest_hash
            .clone()
            .or_else(|| state.and_then(|record| record.manifest_hash.clone())),
        enabled: state.map(|record| record.enabled).unwrap_or(true),
        runtime_status: state.map(|record| record.runtime_status.clone()).unwrap_or_else(|| {
            if scan.valid { RUNTIME_STATUS_STOPPED } else { RUNTIME_STATUS_ERROR }.to_owned()
        }),
        last_error: state
            .and_then(|record| record.last_error.clone())
            .or_else(|| scan.error.clone()),
        installed_at: state.map(|record| record.installed_at.to_rfc3339()),
        updated_at: state.map(|record| record.updated_at.to_rfc3339()),
        last_seen_at: state.and_then(|record| record.last_seen_at.map(|value| value.to_rfc3339())),
        last_started_at: state
            .and_then(|record| record.last_started_at.map(|value| value.to_rfc3339())),
        last_stopped_at: state
            .and_then(|record| record.last_stopped_at.map(|value| value.to_rfc3339())),
        available_version,
        update_available,
        removable: state
            .is_some_and(|record| is_pack_managed_source_kind(record.source_kind.as_str())),
    }
}

fn build_missing_plugin_view(
    state: &PluginStateRecord,
    market: Option<&RemoteMarketPluginItem>,
) -> PluginView {
    let available_version = market.map(|item| item.version.clone());
    PluginView {
        id: state.plugin_id.clone(),
        name: state.plugin_id.clone(),
        version: state.installed_version.clone().unwrap_or_else(|| "missing".to_owned()),
        valid: false,
        error: Some("plugin is recorded in the database but missing on disk".to_owned()),
        manifest_version: 0,
        compatibility: Value::Null,
        ui_entry: None,
        has_wasm: false,
        network_mode: "blocked".to_owned(),
        allow_hosts: Vec::new(),
        contributions: Value::Null,
        permissions: Value::Null,
        source_kind: state.source_kind.clone(),
        source_ref: state.source_ref.clone(),
        install_root: state.install_root.clone(),
        installed_version: state.installed_version.clone(),
        manifest_hash: state.manifest_hash.clone(),
        enabled: state.enabled,
        runtime_status: state.runtime_status.clone(),
        last_error: state.last_error.clone(),
        installed_at: Some(state.installed_at.to_rfc3339()),
        updated_at: Some(state.updated_at.to_rfc3339()),
        last_seen_at: state.last_seen_at.map(|value| value.to_rfc3339()),
        last_started_at: state.last_started_at.map(|value| value.to_rfc3339()),
        last_stopped_at: state.last_stopped_at.map(|value| value.to_rfc3339()),
        available_version: available_version.clone(),
        update_available: matches!(
            (state.installed_version.as_deref(), available_version.as_deref()),
            (Some(installed), Some(available)) if installed != available
        ),
        removable: is_pack_managed_source_kind(&state.source_kind),
    }
}

fn scan_plugins(root_dir: &Path) -> Result<Vec<ScannedPlugin>, AppCoreError> {
    fs::create_dir_all(root_dir).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to create plugins directory {}: {error}",
            root_dir.display()
        ))
    })?;

    let mut rows = Vec::new();
    let entries = fs::read_dir(root_dir).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to scan plugins directory {}: {error}",
            root_dir.display()
        ))
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            AppCoreError::Internal(format!("failed to read plugins directory entry: {error}"))
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if IGNORED_PLUGIN_ROOT_NAMES.iter().any(|ignored| entry.file_name() == *ignored) {
            continue;
        }
        rows.push(scan_plugin_dir(&path, SOURCE_KIND_DEV)?);
    }
    rows.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(rows)
}

fn scan_plugin_dir(
    root_dir: &Path,
    default_source_kind: &str,
) -> Result<ScannedPlugin, AppCoreError> {
    let fallback_id =
        root_dir.file_name().and_then(|name| name.to_str()).unwrap_or("unknown-plugin").to_owned();
    let manifest_path = root_dir.join("plugin.json");
    let manifest_bytes = match fs::read(&manifest_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return Ok(ScannedPlugin {
                id: fallback_id,
                root_dir: root_dir.to_path_buf(),
                source_kind: default_source_kind.to_owned(),
                valid: false,
                error: Some(format!("failed to read {}: {error}", manifest_path.display())),
                manifest: None,
                manifest_hash: None,
            });
        }
    };
    let manifest_hash = hash_bytes_hex(&manifest_bytes);
    let manifest: PluginManifest = match serde_json::from_slice(&manifest_bytes) {
        Ok(manifest) => manifest,
        Err(error) => {
            return Ok(ScannedPlugin {
                id: fallback_id,
                root_dir: root_dir.to_path_buf(),
                source_kind: default_source_kind.to_owned(),
                valid: false,
                error: Some(format!("failed to parse plugin.json: {error}")),
                manifest: None,
                manifest_hash: Some(manifest_hash),
            });
        }
    };
    let plugin_id = manifest.id.clone();
    if let Err(error) = validate_plugin_manifest(root_dir, &manifest) {
        return Ok(ScannedPlugin {
            id: plugin_id,
            root_dir: root_dir.to_path_buf(),
            source_kind: default_source_kind.to_owned(),
            valid: false,
            error: Some(error),
            manifest: Some(manifest),
            manifest_hash: Some(manifest_hash),
        });
    }
    let scanned = ScannedPlugin {
        id: plugin_id,
        root_dir: root_dir.to_path_buf(),
        source_kind: default_source_kind.to_owned(),
        valid: true,
        error: None,
        manifest: Some(manifest),
        manifest_hash: Some(manifest_hash),
    };
    Ok(scanned)
}

fn validate_plugin_manifest(root_dir: &Path, manifest: &PluginManifest) -> Result<(), String> {
    if !is_valid_plugin_id(&manifest.id) {
        return Err(format!(
            "invalid plugin id `{}`: use lowercase letters, numbers, '-' or '_' and length 2..64",
            manifest.id
        ));
    }

    let folder_name = root_dir
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "invalid plugin directory path".to_owned())?;
    if folder_name != manifest.id {
        return Err(format!(
            "plugin folder `{folder_name}` does not match manifest id `{}`",
            manifest.id
        ));
    }

    validate_declared_file(
        root_dir,
        &manifest.integrity.files_sha256,
        &manifest.runtime.ui.entry,
        "runtime.ui.entry",
    )?;
    if let Some(wasm) = &manifest.runtime.wasm {
        validate_declared_file(
            root_dir,
            &manifest.integrity.files_sha256,
            &wasm.entry,
            "runtime.wasm.entry",
        )?;
    }

    if manifest.permissions.network.mode == PluginNetworkMode::Blocked
        && !manifest.permissions.network.allow_hosts.is_empty()
    {
        return Err("permissions.network.allowHosts must be empty when mode is blocked".to_owned());
    }

    validate_contributions(root_dir, manifest)?;
    Ok(())
}

fn validate_contributions(root_dir: &Path, manifest: &PluginManifest) -> Result<(), String> {
    validate_duplicate_ids(
        "contributes.routes",
        manifest.contributes.routes.iter().map(|route| &route.id),
    )?;
    validate_duplicate_ids(
        "contributes.sidebar",
        manifest.contributes.sidebar.iter().map(|item| &item.id),
    )?;
    validate_duplicate_ids(
        "contributes.commands",
        manifest.contributes.commands.iter().map(|item| &item.id),
    )?;
    validate_duplicate_ids(
        "contributes.settings",
        manifest.contributes.settings.iter().map(|item| &item.id),
    )?;
    validate_duplicate_ids(
        "contributes.agentCapabilities",
        manifest.contributes.agent_capabilities.iter().map(|item| &item.id),
    )?;

    if !manifest.contributes.routes.is_empty() {
        ensure_permission(
            &manifest.permissions,
            "route:create",
            "contributes.routes requires permissions.ui to include route:create",
        )?;
    }
    if !manifest.contributes.sidebar.is_empty() {
        ensure_permission(
            &manifest.permissions,
            "sidebar:item:create",
            "contributes.sidebar requires permissions.ui to include sidebar:item:create",
        )?;
    }
    if !manifest.contributes.commands.is_empty() {
        ensure_permission(
            &manifest.permissions,
            "command:create",
            "contributes.commands requires permissions.ui to include command:create",
        )?;
    }
    if !manifest.contributes.settings.is_empty() {
        ensure_permission(
            &manifest.permissions,
            "settings:section:create",
            "contributes.settings requires permissions.ui to include settings:section:create",
        )?;
    }
    if !manifest.contributes.agent_capabilities.is_empty() {
        ensure_agent_permission(
            &manifest.permissions,
            "capability:declare",
            "contributes.agentCapabilities requires permissions.agent to include capability:declare",
        )?;
    }

    let route_ids =
        manifest.contributes.routes.iter().map(|route| route.id.clone()).collect::<HashSet<_>>();
    let path_prefix = format!("/plugins/{}", manifest.id);

    for route in &manifest.contributes.routes {
        validate_route(root_dir, route, manifest, &path_prefix)?;
    }
    for command in &manifest.contributes.commands {
        validate_command(command, &route_ids)?;
    }
    for setting in &manifest.contributes.settings {
        validate_setting(root_dir, setting, manifest)?;
    }
    for capability in &manifest.contributes.agent_capabilities {
        validate_agent_capability(root_dir, capability, manifest)?;
    }
    for sidebar in &manifest.contributes.sidebar {
        validate_sidebar(sidebar, &route_ids)?;
    }

    Ok(())
}

fn validate_route(
    root_dir: &Path,
    route: &PluginRouteContribution,
    manifest: &PluginManifest,
    path_prefix: &str,
) -> Result<(), String> {
    if !(route.path == *path_prefix || route.path.starts_with(&format!("{path_prefix}/"))) {
        return Err(format!("route `{}` must use a path inside `{path_prefix}`", route.id));
    }
    if let Some(entry) = &route.entry {
        validate_declared_file(
            root_dir,
            &manifest.integrity.files_sha256,
            entry,
            "contributes.routes[].entry",
        )?;
    }
    Ok(())
}

fn validate_command(
    command: &PluginCommandContribution,
    route_ids: &HashSet<String>,
) -> Result<(), String> {
    if command.action.as_deref() == Some("openRoute") {
        let route = command.route.as_deref().ok_or_else(|| {
            format!("command `{}` with action `openRoute` must declare route", command.id)
        })?;
        if !route_ids.contains(route) {
            return Err(format!("command `{}` references unknown route `{route}`", command.id));
        }
    }
    Ok(())
}

fn validate_sidebar(
    sidebar: &PluginSidebarContribution,
    route_ids: &HashSet<String>,
) -> Result<(), String> {
    if let Some(route) = sidebar.route.as_deref()
        && !route_ids.contains(route)
    {
        return Err(format!(
            "sidebar contribution `{}` references unknown route `{route}`",
            sidebar.id
        ));
    }
    Ok(())
}

fn validate_setting(
    root_dir: &Path,
    setting: &PluginSettingsContribution,
    manifest: &PluginManifest,
) -> Result<(), String> {
    validate_declared_file(
        root_dir,
        &manifest.integrity.files_sha256,
        &setting.schema,
        "contributes.settings[].schema",
    )?;
    Ok(())
}

fn validate_agent_capability(
    root_dir: &Path,
    capability: &PluginAgentCapabilityContribution,
    manifest: &PluginManifest,
) -> Result<(), String> {
    if let Some(input_schema) = &capability.input_schema {
        validate_declared_file(
            root_dir,
            &manifest.integrity.files_sha256,
            input_schema,
            "contributes.agentCapabilities[].inputSchema",
        )?;
    }
    if let Some(output_schema) = &capability.output_schema {
        validate_declared_file(
            root_dir,
            &manifest.integrity.files_sha256,
            output_schema,
            "contributes.agentCapabilities[].outputSchema",
        )?;
    }
    if capability.expose_as_mcp_tool {
        ensure_agent_permission(
            &manifest.permissions,
            "mcpTool:expose",
            "contributes.agentCapabilities[].exposeAsMcpTool requires permissions.agent to include mcpTool:expose",
        )?;
    }
    Ok(())
}

fn validate_duplicate_ids<'a>(
    context: &str,
    ids: impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id.clone()) {
            return Err(format!("duplicated contribution id `{id}` in {context}"));
        }
    }
    Ok(())
}

fn ensure_permission(
    permissions: &PluginPermissionsManifest,
    expected: &str,
    error: &str,
) -> Result<(), String> {
    if permissions.ui.iter().any(|value| value == expected) {
        Ok(())
    } else {
        Err(error.to_owned())
    }
}

fn ensure_agent_permission(
    permissions: &PluginPermissionsManifest,
    expected: &str,
    error: &str,
) -> Result<(), String> {
    if permissions.agent.iter().any(|value| value == expected) {
        Ok(())
    } else {
        Err(error.to_owned())
    }
}

fn validate_declared_file(
    root_dir: &Path,
    files_sha256: &HashMap<String, String>,
    raw_path: &str,
    context: &str,
) -> Result<String, String> {
    let normalized_path = normalize_relative_path(raw_path)?;
    let expected_hash = files_sha256.get(&normalized_path).ok_or_else(|| {
        format!("{context} `{normalized_path}` is missing from integrity.filesSha256")
    })?;
    let file_path = root_dir.join(&normalized_path);
    if !file_path.is_file() {
        return Err(format!("{context} `{normalized_path}` does not exist on disk"));
    }
    let actual_hash = hash_file_hex(&file_path)
        .map_err(|error| format!("failed to hash `{normalized_path}`: {error}"))?;
    if actual_hash != *expected_hash {
        return Err(format!(
            "integrity.filesSha256 mismatch for `{normalized_path}`: expected {expected_hash}, got {actual_hash}"
        ));
    }
    Ok(normalized_path)
}

fn normalize_relative_path(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim().trim_start_matches('/');
    if trimmed.is_empty() {
        return Err("empty path is not allowed".to_owned());
    }
    let mut components = Vec::new();
    for component in Path::new(trimmed).components() {
        match component {
            Component::Normal(segment) => {
                let segment = segment.to_string_lossy();
                if segment.is_empty() {
                    return Err("empty path segment is not allowed".to_owned());
                }
                components.push(segment.to_string());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("path `{raw}` is invalid"));
            }
        }
    }
    if components.is_empty() {
        return Err("path is invalid".to_owned());
    }
    Ok(components.join("/"))
}

fn is_valid_plugin_id(id: &str) -> bool {
    if !(2..=64).contains(&id.len()) {
        return false;
    }
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }
    chars.all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || character == '-'
            || character == '_'
    })
}

fn is_pack_managed_source_kind(source_kind: &str) -> bool {
    matches!(source_kind, SOURCE_KIND_IMPORT_PACK | SOURCE_KIND_MARKET_PACK)
}

fn network_mode_label(mode: &PluginNetworkMode) -> &'static str {
    match mode {
        PluginNetworkMode::Blocked => "blocked",
        PluginNetworkMode::Allowlist => "allowlist",
    }
}

fn hash_bytes_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

fn hash_file_hex(path: &Path) -> Result<String, std::io::Error> {
    let bytes = fs::read(path)?;
    Ok(hash_bytes_hex(&bytes))
}

async fn load_package_bytes(source: &str) -> Result<Vec<u8>, AppCoreError> {
    if let Ok(url) = Url::parse(source) {
        match url.scheme() {
            "http" | "https" => {
                let response = reqwest::get(source).await.map_err(|error| {
                    AppCoreError::Internal(format!("failed to download plugin package: {error}"))
                })?;
                let status = response.status();
                if !status.is_success() {
                    return Err(AppCoreError::BadRequest(format!(
                        "plugin package download failed with HTTP {status}"
                    )));
                }
                return response.bytes().await.map(|body| body.to_vec()).map_err(|error| {
                    AppCoreError::Internal(format!(
                        "failed to read downloaded plugin package bytes: {error}"
                    ))
                });
            }
            "file" => {
                let path = url.to_file_path().map_err(|_| {
                    AppCoreError::BadRequest(format!(
                        "unsupported file URL for plugin package: {source}"
                    ))
                })?;
                return fs::read(path).map_err(|error| {
                    AppCoreError::Internal(format!(
                        "failed to read plugin package from disk: {error}"
                    ))
                });
            }
            _ => {}
        }
    }

    fs::read(source).map_err(|error| {
        AppCoreError::Internal(format!("failed to read plugin package `{source}`: {error}"))
    })
}

async fn load_market_bytes(source: &str) -> Result<Vec<u8>, AppCoreError> {
    load_package_bytes(source).await
}

fn create_staging_dir(plugins_dir: &Path) -> Result<PathBuf, AppCoreError> {
    let path = plugins_dir.join(format!(".staging-{}", Uuid::new_v4()));
    fs::create_dir_all(&path).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to create plugin staging directory {}: {error}",
            path.display()
        ))
    })?;
    Ok(path)
}

fn extract_plugin_pack_archive(bytes: &[u8], dest: &Path) -> Result<(), AppCoreError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|error| {
        AppCoreError::BadRequest(format!("failed to open .plugin.slab archive: {error}"))
    })?;

    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| {
            AppCoreError::BadRequest(format!(
                "failed to access .plugin.slab entry #{index}: {error}"
            ))
        })?;
        let Some(path) = file.enclosed_name().map(|value| value.to_path_buf()) else {
            continue;
        };
        let target = dest.join(path);
        if file.is_dir() {
            fs::create_dir_all(&target).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to create plugin directory {}: {error}",
                    target.display()
                ))
            })?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to create plugin parent directory {}: {error}",
                    parent.display()
                ))
            })?;
        }
        let mut output = fs::File::create(&target).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to create extracted plugin file {}: {error}",
                target.display()
            ))
        })?;
        std::io::copy(&mut file, &mut output).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to write extracted plugin file {}: {error}",
                target.display()
            ))
        })?;
    }

    Ok(())
}

fn locate_plugin_root(staging_root: &Path) -> Result<PathBuf, AppCoreError> {
    if staging_root.join("plugin.json").is_file() {
        return Ok(staging_root.to_path_buf());
    }

    let mut manifests = Vec::new();
    collect_manifest_parents(staging_root, &mut manifests)?;
    manifests.sort();
    manifests.dedup();

    match manifests.as_slice() {
        [only] => Ok(only.clone()),
        [] => {
            Err(AppCoreError::BadRequest(
                "plugin pack does not contain plugin.json".to_owned(),
            ))
        }
        _ => Err(AppCoreError::BadRequest(
            "plugin pack contains multiple plugin.json files".to_owned(),
        )),
    }
}

fn collect_manifest_parents(root: &Path, output: &mut Vec<PathBuf>) -> Result<(), AppCoreError> {
    for entry in fs::read_dir(root).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to scan extracted plugin directory {}: {error}",
            root.display()
        ))
    })? {
        let entry = entry.map_err(|error| {
            AppCoreError::Internal(format!("failed to read extracted plugin entry: {error}"))
        })?;
        let path = entry.path();
        if path.is_dir() {
            if path.join("plugin.json").is_file() {
                output.push(path.clone());
            }
            collect_manifest_parents(&path, output)?;
        }
    }
    Ok(())
}

fn move_directory(from: &Path, to: &Path) -> Result<(), AppCoreError> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to create plugin destination {}: {error}",
                parent.display()
            ))
        })?;
    }
    fs::rename(from, to).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to move plugin directory from {} to {}: {error}",
            from.display(),
            to.display()
        ))
    })
}

fn safe_remove_dir(path: &Path) -> Result<(), AppCoreError> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to remove plugin directory {}: {error}",
                path.display()
            ))
        })?;
    }
    Ok(())
}

fn ensure_path_within(path: &Path, root: &Path) -> Result<(), AppCoreError> {
    let root = root.canonicalize().map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to resolve plugins root {}: {error}",
            root.display()
        ))
    })?;
    let path = if path.is_absolute() { path.to_path_buf() } else { root.join(path) };
    if path.starts_with(&root) {
        Ok(())
    } else {
        Err(AppCoreError::BadRequest(format!(
            "plugin path {} escapes plugins root {}",
            path.display(),
            root.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        hash_bytes_hex, locate_plugin_root, normalize_relative_path, resolve_market_package_url,
        scan_plugin_dir, scan_plugins,
    };
    use serde_json::json;
    use std::fs;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("slab-plugin-service-{name}-{}", uuid::Uuid::new_v4()))
    }

    fn write(path: &std::path::Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, content).expect("write file");
    }

    #[test]
    fn normalize_relative_path_rejects_parent_segments() {
        assert!(normalize_relative_path("../plugin.json").is_err());
        assert_eq!(normalize_relative_path("ui/index.html").expect("normalize"), "ui/index.html");
    }

    #[test]
    fn scan_plugin_dir_validates_integrity() {
        let root = temp_dir("scan");
        let plugin_root = root.join("example-plugin");
        write(&plugin_root.join("ui/index.html"), "<html></html>");
        let html_hash = hash_bytes_hex(b"<html></html>");
        write(
            &plugin_root.join("plugin.json"),
            &serde_json::to_string_pretty(&json!({
                "manifestVersion": 1,
                "id": "example-plugin",
                "name": "Example Plugin",
                "version": "0.1.0",
                "runtime": { "ui": { "entry": "ui/index.html" } },
                "integrity": { "filesSha256": { "ui/index.html": html_hash } },
                "permissions": { "network": { "mode": "blocked", "allowHosts": [] } }
            }))
            .expect("manifest json"),
        );

        let scanned = scan_plugin_dir(&plugin_root, "dev").expect("scan plugin");
        assert!(scanned.valid);
        assert_eq!(scanned.id, "example-plugin");
    }

    #[test]
    fn locate_plugin_root_accepts_nested_archive_layout() {
        let root = temp_dir("locate");
        let nested = root.join("archive-root").join("example-plugin");
        write(&nested.join("plugin.json"), "{}");
        let located = locate_plugin_root(&root).expect("locate plugin root");
        assert_eq!(located, nested);
    }

    #[test]
    fn scan_plugins_ignores_dist_directory() {
        let root = temp_dir("scan-plugins");
        fs::create_dir_all(root.join("dist")).expect("create dist dir");
        fs::write(root.join("dist").join("example.plugin.slab"), b"pack").expect("write pack");

        let rows = scan_plugins(&root).expect("scan plugins");

        assert!(rows.is_empty());
    }

    #[test]
    fn resolve_market_package_url_supports_relative_file_paths() {
        let resolved = resolve_market_package_url(
            "C:/repo/plugins/dist/plugin-market.json",
            "video-subtitle-translator-0.1.0.plugin.slab",
        )
        .expect("resolve package url");

        assert!(
            resolved
                .replace('\\', "/")
                .ends_with("plugins/dist/video-subtitle-translator-0.1.0.plugin.slab")
        );
    }
}
