use slab_types::plugin::PluginNetworkMode;

use crate::domain::models::PluginView;
use crate::infra::db::PluginStateRecord;

use super::scan::ScannedPlugin;
use super::{
    RUNTIME_STATUS_ERROR, RUNTIME_STATUS_STOPPED, SOURCE_KIND_IMPORT_PACK, SOURCE_KIND_PACKAGE_URL,
};

pub(super) fn build_plugin_view(
    scan: &ScannedPlugin,
    state: Option<&PluginStateRecord>,
    api_base_url: &str,
) -> PluginView {
    let manifest = scan.manifest.as_ref();
    let source_kind =
        state.map(|record| record.source_kind.clone()).unwrap_or_else(|| scan.source_kind.clone());
    let source_ref = state.and_then(|record| record.source_ref.clone());
    let installed_version = manifest
        .map(|value| value.version.clone())
        .or_else(|| state.and_then(|record| record.installed_version.clone()));
    let ui_entry = manifest.map(|value| value.runtime.ui.entry.clone());

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
        compatibility: manifest.map(|value| value.compatibility.clone()),
        ui_url: ui_entry.as_deref().and_then(|entry| plugin_ui_url(api_base_url, &scan.id, entry)),
        ui_entry,
        has_wasm: manifest.and_then(|value| value.runtime.wasm.as_ref()).is_some(),
        network_mode: manifest
            .map(|value| network_mode_label(&value.permissions.network.mode).to_owned())
            .unwrap_or_else(|| "blocked".to_owned()),
        allow_hosts: manifest
            .map(|value| value.permissions.network.allow_hosts.clone())
            .unwrap_or_default(),
        contributions: manifest.map(|value| value.contributes.clone()),
        permissions: manifest.map(|value| value.permissions.clone()),
        source_kind: source_kind.clone(),
        source_ref: source_ref.clone(),
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
        available_version: None,
        update_available: has_reinstall_source(&source_kind, source_ref.as_deref()),
        removable: state
            .is_some_and(|record| is_pack_managed_source_kind(record.source_kind.as_str())),
    }
}

pub(super) fn build_missing_plugin_view(state: &PluginStateRecord) -> PluginView {
    PluginView {
        id: state.plugin_id.clone(),
        name: state.plugin_id.clone(),
        version: state.installed_version.clone().unwrap_or_else(|| "missing".to_owned()),
        valid: false,
        error: Some("plugin is recorded in the database but missing on disk".to_owned()),
        manifest_version: 0,
        compatibility: None,
        ui_entry: None,
        ui_url: None,
        has_wasm: false,
        network_mode: "blocked".to_owned(),
        allow_hosts: Vec::new(),
        contributions: None,
        permissions: None,
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
        available_version: None,
        update_available: has_reinstall_source(&state.source_kind, state.source_ref.as_deref()),
        removable: is_pack_managed_source_kind(&state.source_kind),
    }
}

pub(super) fn plugin_ui_url(api_base_url: &str, plugin_id: &str, ui_entry: &str) -> Option<String> {
    let mut url = reqwest::Url::parse(api_base_url).ok()?;
    {
        let mut path_segments = url.path_segments_mut().ok()?;
        path_segments.pop_if_empty();
        path_segments.extend(["v1", "plugins", plugin_id, "ui"]);
        for segment in ui_entry.split('/') {
            path_segments.push(segment);
        }
    }
    Some(url.to_string())
}

pub(super) fn is_pack_managed_source_kind(source_kind: &str) -> bool {
    matches!(source_kind, SOURCE_KIND_IMPORT_PACK | SOURCE_KIND_PACKAGE_URL)
}

fn has_reinstall_source(source_kind: &str, source_ref: Option<&str>) -> bool {
    source_kind == SOURCE_KIND_PACKAGE_URL
        && source_ref.is_some_and(|value| !value.trim().is_empty())
}

fn network_mode_label(mode: &PluginNetworkMode) -> &'static str {
    match mode {
        PluginNetworkMode::Blocked => "blocked",
        PluginNetworkMode::Allowlist => "allowlist",
    }
}
