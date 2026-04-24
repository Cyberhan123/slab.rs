use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::RwLock;

use percent_encoding::percent_decode_str;
use sha2::{Digest, Sha256};
use tauri::Manager;

use super::types::{
    PluginCapabilityTransportType, PluginCommandContribution, PluginContributesManifest,
    PluginInfo, PluginManifest, PluginNetworkMode, PluginPermissionsManifest,
    PluginSettingsContribution, PluginSidebarContribution,
};

pub const DEFAULT_PLUGINS_DIR: &str = "plugins";
const IGNORED_PLUGIN_ROOT_NAMES: &[&str] = &["dist", ".git", "node_modules"];

#[derive(Clone)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub root_dir: PathBuf,
    pub ui_entry: String,
    pub wasm_entry_path: Option<PathBuf>,
    pub files_sha256: HashMap<String, String>,
    #[allow(dead_code)]
    pub extension_registry: ExtensionPointRegistry,
    pub capability_registry: CapabilityRegistry,
}

#[derive(Clone, Debug, Default)]
pub struct ExtensionPointRegistry {
    contribution_ids: HashSet<String>,
    route_ids: HashSet<String>,
    route_paths: HashSet<String>,
    command_ids: HashSet<String>,
}

impl ExtensionPointRegistry {
    fn contains_route_reference(&self, target: &str) -> bool {
        self.route_ids.contains(target) || self.route_paths.contains(target)
    }

    fn contains_command(&self, target: &str) -> bool {
        self.command_ids.contains(target)
    }
}

#[derive(Clone, Debug, Default)]
pub struct CapabilityRegistry {
    capability_ids: HashSet<String>,
}

#[derive(Default)]
struct PluginRegistrySnapshot {
    loaded: HashMap<String, LoadedPlugin>,
    invalid: HashMap<String, String>,
}

pub struct PluginRegistryState {
    root_dir: PathBuf,
    snapshot: RwLock<PluginRegistrySnapshot>,
}

impl PluginRegistryState {
    pub fn new(root_dir: PathBuf) -> Result<Self, String> {
        if !root_dir.exists() {
            fs::create_dir_all(&root_dir).map_err(|e| {
                format!("failed to create plugins directory {}: {e}", root_dir.display())
            })?;
        }

        let registry = Self { root_dir, snapshot: RwLock::new(PluginRegistrySnapshot::default()) };
        registry.refresh()?;
        Ok(registry)
    }

    pub fn refresh(&self) -> Result<(), String> {
        let fresh = scan_plugins(&self.root_dir)?;
        let mut guard = self
            .snapshot
            .write()
            .map_err(|_| "failed to lock plugin registry for write".to_string())?;
        *guard = fresh;
        Ok(())
    }

    pub fn get_plugin(&self, plugin_id: &str) -> Result<LoadedPlugin, String> {
        let guard = self
            .snapshot
            .read()
            .map_err(|_| "failed to lock plugin registry for read".to_string())?;
        guard
            .loaded
            .get(plugin_id)
            .cloned()
            .ok_or_else(|| format!("plugin `{plugin_id}` is not available"))
    }

    pub fn list(&self) -> Result<Vec<PluginInfo>, String> {
        let guard = self
            .snapshot
            .read()
            .map_err(|_| "failed to lock plugin registry for read".to_string())?;

        let mut rows = Vec::new();

        for plugin in guard.loaded.values() {
            rows.push(PluginInfo {
                id: plugin.manifest.id.clone(),
                name: plugin.manifest.name.clone(),
                version: plugin.manifest.version.clone(),
                valid: true,
                error: None,
                manifest_version: plugin.manifest.manifest_version,
                compatibility: plugin.manifest.compatibility.clone(),
                ui_entry: Some(plugin.ui_entry.clone()),
                has_wasm: plugin.wasm_entry_path.is_some(),
                network_mode: network_mode_label(&plugin.manifest.permissions.network.mode)
                    .to_string(),
                allow_hosts: plugin.manifest.permissions.network.allow_hosts.clone(),
                contributions: plugin.manifest.contributes.clone(),
                permissions: plugin.manifest.permissions.clone(),
            });
        }

        for (id, error) in &guard.invalid {
            rows.push(PluginInfo {
                id: id.clone(),
                name: id.clone(),
                version: "invalid".to_string(),
                valid: false,
                error: Some(error.clone()),
                manifest_version: 0,
                compatibility: Default::default(),
                ui_entry: None,
                has_wasm: false,
                network_mode: "blocked".to_string(),
                allow_hosts: Vec::new(),
                contributions: PluginContributesManifest::default(),
                permissions: PluginPermissionsManifest::default(),
            });
        }

        rows.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(rows)
    }
}

pub fn resolve_plugins_root<R: tauri::Runtime>(app: &tauri::App<R>) -> Result<PathBuf, String> {
    let app_data_plugins_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data directory for plugins: {e}"))?
        .join(DEFAULT_PLUGINS_DIR);
    let settings_path = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("failed to resolve app config directory for plugins: {e}"))?
        .join("settings.json");

    Ok(resolve_plugins_root_with(
        std::env::var("SLAB_PLUGINS_DIR").ok().map(PathBuf::from),
        plugin_install_dir_from_settings(&settings_path),
        app_data_plugins_dir,
    ))
}

fn resolve_plugins_root_with(
    explicit_root: Option<PathBuf>,
    settings_install_dir: Option<PathBuf>,
    app_data_plugins_dir: PathBuf,
) -> PathBuf {
    if let Some(path) = explicit_root {
        return path;
    }

    if let Some(path) = settings_install_dir {
        return path;
    }

    app_data_plugins_dir
}

fn plugin_install_dir_from_settings(settings_path: &Path) -> Option<PathBuf> {
    let raw = fs::read_to_string(settings_path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value
        .pointer("/plugin/install_dir")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub fn normalize_relative_path(raw: &str) -> Result<String, String> {
    let decoded = percent_decode_str(raw)
        .decode_utf8()
        .map_err(|_| format!("path `{raw}` is not valid utf-8"))?;

    let trimmed = decoded.trim().trim_start_matches('/');
    if trimmed.is_empty() {
        return Err("empty path is not allowed".to_string());
    }

    let mut components = Vec::new();
    for component in Path::new(trimmed).components() {
        match component {
            Component::Normal(segment) => {
                let segment = segment.to_string_lossy();
                if segment.is_empty() {
                    return Err("empty path segment is not allowed".to_string());
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
        return Err("path is invalid".to_string());
    }

    Ok(components.join("/"))
}

pub fn is_path_within_root(root: &Path, path: &Path) -> bool {
    let Ok(canonical_root) = root.canonicalize() else {
        return false;
    };
    let Ok(canonical_path) = path.canonicalize() else {
        return false;
    };
    canonical_path.starts_with(canonical_root)
}

fn scan_plugins(root_dir: &Path) -> Result<PluginRegistrySnapshot, String> {
    let mut loaded = HashMap::new();
    let mut invalid = HashMap::new();
    let mut seen_capability_ids = HashMap::<String, String>::new();

    let entries = fs::read_dir(root_dir)
        .map_err(|e| format!("failed to scan plugins directory {}: {e}", root_dir.display()))?;

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                invalid.insert(
                    "unknown".to_string(),
                    format!("failed to read plugins directory entry: {error}"),
                );
                continue;
            }
        };

        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                invalid.insert(
                    entry.file_name().to_string_lossy().to_string(),
                    format!("failed to read entry metadata: {error}"),
                );
                continue;
            }
        };

        if !file_type.is_dir() {
            continue;
        }

        let folder_name = entry.file_name().to_string_lossy().to_string();
        if IGNORED_PLUGIN_ROOT_NAMES.iter().any(|ignored| ignored == &folder_name.as_str()) {
            continue;
        }
        let plugin_dir = entry.path();
        let manifest_path = plugin_dir.join("plugin.json");
        if !manifest_path.exists() {
            continue;
        }

        let raw_manifest = match fs::read_to_string(&manifest_path) {
            Ok(content) => content,
            Err(error) => {
                invalid.insert(
                    folder_name,
                    format!("failed to read plugin.json at {}: {error}", manifest_path.display()),
                );
                continue;
            }
        };

        let manifest: PluginManifest = match serde_json::from_str(&raw_manifest) {
            Ok(manifest) => manifest,
            Err(error) => {
                invalid.insert(
                    folder_name,
                    format!("invalid plugin.json at {}: {error}", manifest_path.display()),
                );
                continue;
            }
        };

        let plugin_id = manifest.id.clone();
        match validate_and_load_plugin(&plugin_dir, manifest) {
            Ok(plugin) => {
                if loaded.contains_key(&plugin_id) {
                    invalid.insert(plugin_id, "duplicated plugin id".to_string());
                    continue;
                }

                for capability_id in &plugin.capability_registry.capability_ids {
                    if let Some(existing_plugin) = seen_capability_ids.get(capability_id) {
                        invalid.insert(
                            plugin_id.clone(),
                            format!(
                                "duplicated capability id `{capability_id}` already provided by plugin `{existing_plugin}`"
                            ),
                        );
                    }
                }

                if invalid.contains_key(&plugin_id) {
                    continue;
                }

                for capability_id in &plugin.capability_registry.capability_ids {
                    seen_capability_ids.insert(capability_id.clone(), plugin_id.clone());
                }
                loaded.insert(plugin_id, plugin);
            }
            Err(error) => {
                invalid.insert(plugin_id, error);
            }
        }
    }

    Ok(PluginRegistrySnapshot { loaded, invalid })
}

fn validate_and_load_plugin(
    plugin_dir: &Path,
    manifest: PluginManifest,
) -> Result<LoadedPlugin, String> {
    if !is_valid_plugin_id(&manifest.id) {
        return Err(format!(
            "invalid plugin id `{}`: use lowercase letters, numbers, '-' or '_' and length 2..64",
            manifest.id
        ));
    }

    let folder_name = plugin_dir
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .ok_or_else(|| "invalid plugin directory path".to_string())?;
    if folder_name != manifest.id {
        return Err(format!(
            "plugin folder `{folder_name}` does not match manifest id `{}`",
            manifest.id
        ));
    }

    if manifest.integrity.files_sha256.is_empty() {
        return Err("integrity.filesSha256 must not be empty".to_string());
    }

    let mut files_sha256 = HashMap::new();
    for (raw_path, expected_hash) in &manifest.integrity.files_sha256 {
        let normalized_path = normalize_relative_path(raw_path)?;
        validate_sha256_hash(expected_hash)?;

        let file_path = plugin_dir.join(&normalized_path);
        if !file_path.is_file() {
            return Err(format!("integrity target `{normalized_path}` does not exist as a file"));
        }

        let computed_hash = compute_file_sha256(&file_path)?;
        if !expected_hash.eq_ignore_ascii_case(&computed_hash) {
            return Err(format!(
                "integrity mismatch on `{normalized_path}`: expected {expected_hash}, got {computed_hash}"
            ));
        }

        files_sha256.insert(normalized_path, expected_hash.to_ascii_lowercase());
    }

    let ui_entry = validate_declared_file(
        plugin_dir,
        &files_sha256,
        &manifest.runtime.ui.entry,
        "runtime.ui.entry",
    )?
    .0;

    let wasm_entry_path = match manifest.runtime.wasm.as_ref() {
        Some(wasm) => Some(
            validate_declared_file(plugin_dir, &files_sha256, &wasm.entry, "runtime.wasm.entry")?.1,
        ),
        None => None,
    };

    if manifest.permissions.network.mode == PluginNetworkMode::Blocked
        && !manifest.permissions.network.allow_hosts.is_empty()
    {
        return Err(
            "permissions.network.allowHosts must be empty when mode is `blocked`".to_string()
        );
    }

    let extension_registry = build_extension_registry(plugin_dir, &manifest, &files_sha256)?;
    let capability_registry = build_capability_registry(plugin_dir, &manifest, &files_sha256)?;

    Ok(LoadedPlugin {
        manifest,
        root_dir: plugin_dir.to_path_buf(),
        ui_entry,
        wasm_entry_path,
        files_sha256,
        extension_registry,
        capability_registry,
    })
}

fn build_extension_registry(
    plugin_dir: &Path,
    manifest: &PluginManifest,
    files_sha256: &HashMap<String, String>,
) -> Result<ExtensionPointRegistry, String> {
    let mut registry = ExtensionPointRegistry::default();
    let path_prefix = format!("/plugins/{}", manifest.id);

    ensure_ui_permission(
        manifest,
        !manifest.contributes.routes.is_empty(),
        "route:create",
        "contributes.routes",
    )?;
    ensure_ui_permission(
        manifest,
        !manifest.contributes.sidebar.is_empty(),
        "sidebar:item:create",
        "contributes.sidebar",
    )?;
    ensure_ui_permission(
        manifest,
        !manifest.contributes.commands.is_empty(),
        "command:create",
        "contributes.commands",
    )?;
    ensure_ui_permission(
        manifest,
        !manifest.contributes.settings.is_empty(),
        "settings:section:create",
        "contributes.settings",
    )?;

    for route in &manifest.contributes.routes {
        validate_contribution_id(&route.id, "route id")?;
        insert_contribution_id(&mut registry.contribution_ids, &route.id)?;
        if !(route.path == path_prefix || route.path.starts_with(&(path_prefix.clone() + "/"))) {
            return Err(format!("route `{}` must use a path inside `{}`", route.id, path_prefix));
        }
        if !registry.route_ids.insert(route.id.clone()) {
            return Err(format!("duplicated route id `{}`", route.id));
        }
        registry.route_paths.insert(route.path.clone());
        if let Some(entry) = route.entry.as_deref() {
            validate_declared_file(plugin_dir, files_sha256, entry, "contributes.routes[].entry")?;
        }
    }

    for command in &manifest.contributes.commands {
        validate_command_contribution(&mut registry, command)?;
    }

    for sidebar in &manifest.contributes.sidebar {
        validate_sidebar_contribution(&mut registry, sidebar)?;
    }

    for setting in &manifest.contributes.settings {
        validate_settings_contribution(plugin_dir, files_sha256, &mut registry, setting)?;
    }

    Ok(registry)
}

fn build_capability_registry(
    plugin_dir: &Path,
    manifest: &PluginManifest,
    files_sha256: &HashMap<String, String>,
) -> Result<CapabilityRegistry, String> {
    let mut registry = CapabilityRegistry::default();
    ensure_agent_permission(
        manifest,
        !manifest.contributes.agent_capabilities.is_empty(),
        "capability:declare",
        "contributes.agentCapabilities",
    )?;

    for capability in &manifest.contributes.agent_capabilities {
        validate_capability_id(&capability.id)?;
        if !registry.capability_ids.insert(capability.id.clone()) {
            return Err(format!("duplicated capability id `{}`", capability.id));
        }
        if capability.transport.transport_type != PluginCapabilityTransportType::PluginCall {
            return Err(format!(
                "capability `{}` uses an unsupported transport type",
                capability.id
            ));
        }
        if capability.transport.function.trim().is_empty() {
            return Err(format!(
                "capability `{}` must declare a transport.function",
                capability.id
            ));
        }
        if let Some(path) = capability.input_schema.as_deref() {
            validate_declared_file(
                plugin_dir,
                files_sha256,
                path,
                "contributes.agentCapabilities[].inputSchema",
            )?;
        }
        if let Some(path) = capability.output_schema.as_deref() {
            validate_declared_file(
                plugin_dir,
                files_sha256,
                path,
                "contributes.agentCapabilities[].outputSchema",
            )?;
        }
        if capability.expose_as_mcp_tool {
            ensure_agent_permission(
                manifest,
                true,
                "mcpTool:expose",
                "contributes.agentCapabilities[].exposeAsMcpTool",
            )?;
        }
    }

    Ok(registry)
}

fn validate_command_contribution(
    registry: &mut ExtensionPointRegistry,
    command: &PluginCommandContribution,
) -> Result<(), String> {
    validate_contribution_id(&command.id, "command id")?;
    insert_contribution_id(&mut registry.contribution_ids, &command.id)?;
    registry.command_ids.insert(command.id.clone());

    if command.action.as_deref() == Some("openRoute") {
        let Some(route_target) = command.route.as_deref() else {
            return Err(format!(
                "command `{}` with action `openRoute` must declare route",
                command.id
            ));
        };
        if !registry.contains_route_reference(route_target) {
            return Err(format!(
                "command `{}` references unknown route `{}`",
                command.id, route_target
            ));
        }
    }

    Ok(())
}

fn validate_sidebar_contribution(
    registry: &mut ExtensionPointRegistry,
    sidebar: &PluginSidebarContribution,
) -> Result<(), String> {
    validate_contribution_id(&sidebar.id, "sidebar id")?;
    insert_contribution_id(&mut registry.contribution_ids, &sidebar.id)?;

    match (sidebar.route.as_deref(), sidebar.command.as_deref()) {
        (Some(route), None) => {
            if !registry.contains_route_reference(route) {
                return Err(format!(
                    "sidebar `{}` references unknown route `{}`",
                    sidebar.id, route
                ));
            }
        }
        (None, Some(command)) => {
            if !registry.contains_command(command) {
                return Err(format!(
                    "sidebar `{}` references unknown command `{}`",
                    sidebar.id, command
                ));
            }
        }
        _ => {
            return Err(format!(
                "sidebar `{}` must reference exactly one of `route` or `command`",
                sidebar.id
            ));
        }
    }

    Ok(())
}

fn validate_settings_contribution(
    plugin_dir: &Path,
    files_sha256: &HashMap<String, String>,
    registry: &mut ExtensionPointRegistry,
    setting: &PluginSettingsContribution,
) -> Result<(), String> {
    validate_contribution_id(&setting.id, "settings id")?;
    insert_contribution_id(&mut registry.contribution_ids, &setting.id)?;
    validate_declared_file(
        plugin_dir,
        files_sha256,
        &setting.schema,
        "contributes.settings[].schema",
    )?;
    Ok(())
}

fn validate_declared_file(
    plugin_dir: &Path,
    files_sha256: &HashMap<String, String>,
    raw_path: &str,
    label: &str,
) -> Result<(String, PathBuf), String> {
    let normalized_path = normalize_relative_path(raw_path)?;
    if !files_sha256.contains_key(&normalized_path) {
        return Err(format!("integrity.filesSha256 must contain {label}"));
    }
    let file_path = plugin_dir.join(&normalized_path);
    if !file_path.is_file() {
        return Err(format!("{label} file does not exist at {}", file_path.display()));
    }
    Ok((normalized_path, file_path))
}

fn validate_contribution_id(id: &str, label: &str) -> Result<(), String> {
    if !is_valid_extension_id(id) {
        return Err(format!(
            "invalid {label} `{id}`: use lowercase letters, numbers, '.', '-' or '_' and length 2..128"
        ));
    }
    Ok(())
}

fn validate_capability_id(id: &str) -> Result<(), String> {
    validate_contribution_id(id, "capability id")
}

fn insert_contribution_id(ids: &mut HashSet<String>, id: &str) -> Result<(), String> {
    if !ids.insert(id.to_string()) {
        return Err(format!("duplicated contribution id `{id}`"));
    }
    Ok(())
}

fn ensure_ui_permission(
    manifest: &PluginManifest,
    needed: bool,
    permission: &str,
    contribution_name: &str,
) -> Result<(), String> {
    if needed && !manifest.permissions.ui.iter().any(|entry| entry == permission) {
        return Err(format!(
            "permissions.ui must include `{permission}` when `{contribution_name}` is declared"
        ));
    }
    Ok(())
}

fn ensure_agent_permission(
    manifest: &PluginManifest,
    needed: bool,
    permission: &str,
    contribution_name: &str,
) -> Result<(), String> {
    if needed && !manifest.permissions.agent.iter().any(|entry| entry == permission) {
        return Err(format!(
            "permissions.agent must include `{permission}` when `{contribution_name}` is declared"
        ));
    }
    Ok(())
}

fn is_valid_plugin_id(id: &str) -> bool {
    if id.len() < 2 || id.len() > 64 {
        return false;
    }

    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }

    chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
}

fn is_valid_extension_id(id: &str) -> bool {
    if id.len() < 2 || id.len() > 128 {
        return false;
    }

    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }

    chars.all(|ch| {
        ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_' || ch == '.'
    })
}

fn validate_sha256_hash(hash: &str) -> Result<(), String> {
    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("invalid SHA-256 hash `{hash}`"));
    }
    Ok(())
}

fn compute_file_sha256(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path)
        .map_err(|e| format!("failed to open `{}` for hashing: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];

    loop {
        let read = file
            .read(&mut buf)
            .map_err(|e| format!("failed to read `{}` for hashing: {e}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }

    Ok(hex::encode(hasher.finalize()))
}

fn network_mode_label(mode: &PluginNetworkMode) -> &'static str {
    match mode {
        PluginNetworkMode::Blocked => "blocked",
        PluginNetworkMode::Allowlist => "allowlist",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("slab-plugin-{name}-{suffix}"));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn write_plugin_file(
        root: &Path,
        plugin_id: &str,
        relative_path: &str,
        content: &str,
    ) -> String {
        let path = root.join(plugin_id).join(relative_path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
        compute_file_sha256(&path).unwrap()
    }

    fn write_manifest(root: &Path, plugin_id: &str, manifest: serde_json::Value) {
        let plugin_dir = root.join(plugin_id);
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(plugin_dir.join("plugin.json"), serde_json::to_string_pretty(&manifest).unwrap())
            .unwrap();
    }

    #[test]
    fn plugin_id_validation_works() {
        assert!(is_valid_plugin_id("plugin-01"));
        assert!(is_valid_plugin_id("a1"));
        assert!(!is_valid_plugin_id("A1"));
        assert!(!is_valid_plugin_id("a"));
        assert!(!is_valid_plugin_id("a*b"));
        assert!(!is_valid_plugin_id("-abc"));
    }

    #[test]
    fn relative_path_normalization_rejects_parent_dir() {
        assert!(normalize_relative_path("../test").is_err());
        assert!(normalize_relative_path("a/../../b").is_err());
        assert!(normalize_relative_path("/ui/index.html").is_ok());
    }

    #[test]
    fn plugins_root_prefers_settings_install_dir() {
        let root = temp_root("root-prefers-settings");
        let install_dir = root.join("configured-plugins");
        let app_data_plugins_dir = root.join("app-data").join(DEFAULT_PLUGINS_DIR);

        let resolved =
            resolve_plugins_root_with(None, Some(install_dir.clone()), app_data_plugins_dir);

        assert_eq!(resolved, install_dir);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn plugins_root_falls_back_to_app_data_without_settings_install_dir() {
        let root = temp_root("root-fallback");
        let app_data_plugins_dir = root.join("app-data").join(DEFAULT_PLUGINS_DIR);

        let resolved = resolve_plugins_root_with(None, None, app_data_plugins_dir.clone());

        assert_eq!(resolved, app_data_plugins_dir);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn plugins_root_prefers_explicit_env_over_settings_install_dir() {
        let root = temp_root("root-prefers-env");
        let explicit_dir = root.join("env-plugins");
        let install_dir = root.join("configured-plugins");
        let app_data_plugins_dir = root.join("app-data").join(DEFAULT_PLUGINS_DIR);

        let resolved = resolve_plugins_root_with(
            Some(explicit_dir.clone()),
            Some(install_dir),
            app_data_plugins_dir,
        );

        assert_eq!(resolved, explicit_dir);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn registry_ignores_dist_directory() {
        let root = temp_root("registry-ignores-dist");
        fs::create_dir_all(root.join("dist")).unwrap();
        fs::write(root.join("dist").join("example.plugin.slab"), b"pack").unwrap();

        let snapshot = scan_plugins(&root).expect("scan plugins");

        assert!(snapshot.loaded.is_empty());
        assert!(snapshot.invalid.is_empty());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn registry_ignores_non_plugin_directories() {
        let root = temp_root("registry-ignores-non-plugin-directories");
        fs::create_dir_all(root.join("scripts")).unwrap();
        fs::write(root.join("scripts").join("generate-plugin-packs.ts"), b"export {};").unwrap();

        let snapshot = scan_plugins(&root).expect("scan plugins");

        assert!(snapshot.loaded.is_empty());
        assert!(snapshot.invalid.is_empty());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn registry_lists_legacy_ui_only_plugin_without_wasm() {
        let root = temp_root("legacy-ui-only");
        let html_hash = write_plugin_file(
            &root,
            "ui-only-plugin",
            "ui/index.html",
            "<!doctype html><title>ui only</title>",
        );

        write_manifest(
            &root,
            "ui-only-plugin",
            serde_json::json!({
                "id": "ui-only-plugin",
                "name": "UI Only Plugin",
                "version": "0.1.0",
                "ui": { "entry": "ui/index.html" },
                "integrity": { "filesSha256": { "ui/index.html": html_hash } },
                "network": { "mode": "blocked", "allowHosts": [] }
            }),
        );

        let registry = PluginRegistryState::new(root.clone()).unwrap();
        let plugins = registry.list().unwrap();
        let plugin = plugins.iter().find(|plugin| plugin.id == "ui-only-plugin").unwrap();

        assert!(plugin.valid);
        assert_eq!(plugin.manifest_version, 0);
        assert!(!plugin.has_wasm);
        assert_eq!(plugin.ui_entry.as_deref(), Some("ui/index.html"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn registry_accepts_v1_manifest_with_contributions_and_capabilities() {
        let root = temp_root("v1-manifest");
        let html_hash = write_plugin_file(
            &root,
            "video-subtitle-translator",
            "ui/index.html",
            "<!doctype html><title>v1</title>",
        );
        let settings_hash = write_plugin_file(
            &root,
            "video-subtitle-translator",
            "schemas/settings.schema.json",
            "{\"type\":\"object\"}",
        );
        let input_hash = write_plugin_file(
            &root,
            "video-subtitle-translator",
            "schemas/translate-input.schema.json",
            "{\"type\":\"object\"}",
        );
        let output_hash = write_plugin_file(
            &root,
            "video-subtitle-translator",
            "schemas/translate-output.schema.json",
            "{\"type\":\"object\"}",
        );

        write_manifest(
            &root,
            "video-subtitle-translator",
            serde_json::json!({
                "manifestVersion": 1,
                "id": "video-subtitle-translator",
                "name": "Video Subtitle Translator",
                "version": "0.1.0",
                "runtime": { "ui": { "entry": "ui/index.html" } },
                "integrity": {
                    "filesSha256": {
                        "ui/index.html": html_hash,
                        "schemas/settings.schema.json": settings_hash,
                        "schemas/translate-input.schema.json": input_hash,
                        "schemas/translate-output.schema.json": output_hash
                    }
                },
                "permissions": {
                    "network": { "mode": "blocked", "allowHosts": [] },
                    "ui": ["route:create", "sidebar:item:create", "command:create", "settings:section:create"],
                    "agent": ["capability:declare", "mcpTool:expose"]
                },
                "contributes": {
                    "routes": [{ "id": "subtitle.translate.page", "path": "/plugins/video-subtitle-translator" }],
                    "commands": [{ "id": "subtitle.translate.open", "action": "openRoute", "route": "subtitle.translate.page" }],
                    "sidebar": [{ "id": "subtitle.translate.nav", "route": "subtitle.translate.page" }],
                    "settings": [{ "id": "subtitle.translate.settings", "schema": "schemas/settings.schema.json" }],
                    "agentCapabilities": [{
                        "id": "subtitle.translate_video",
                        "kind": "workflow",
                        "inputSchema": "schemas/translate-input.schema.json",
                        "outputSchema": "schemas/translate-output.schema.json",
                        "transport": { "type": "pluginCall", "function": "translateVideo" },
                        "exposeAsMcpTool": true
                    }]
                }
            }),
        );

        let registry = PluginRegistryState::new(root.clone()).unwrap();
        let plugins = registry.list().unwrap();
        let plugin =
            plugins.iter().find(|plugin| plugin.id == "video-subtitle-translator").unwrap();

        assert!(plugin.valid);
        assert_eq!(plugin.manifest_version, 1);
        assert_eq!(plugin.contributions.routes.len(), 1);
        assert_eq!(plugin.contributions.agent_capabilities.len(), 1);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn registry_rejects_missing_integrity_entries_for_settings_schema() {
        let root = temp_root("missing-integrity");
        let html_hash = write_plugin_file(
            &root,
            "settings-plugin",
            "ui/index.html",
            "<!doctype html><title>ui only</title>",
        );
        write_plugin_file(
            &root,
            "settings-plugin",
            "schemas/settings.schema.json",
            "{\"type\":\"object\"}",
        );

        write_manifest(
            &root,
            "settings-plugin",
            serde_json::json!({
                "manifestVersion": 1,
                "id": "settings-plugin",
                "name": "Settings Plugin",
                "version": "0.1.0",
                "runtime": { "ui": { "entry": "ui/index.html" } },
                "integrity": { "filesSha256": { "ui/index.html": html_hash } },
                "permissions": {
                    "network": { "mode": "blocked", "allowHosts": [] },
                    "ui": ["settings:section:create"]
                },
                "contributes": {
                    "settings": [{ "id": "settings.page", "schema": "schemas/settings.schema.json" }]
                }
            }),
        );

        let registry = PluginRegistryState::new(root.clone()).unwrap();
        let plugins = registry.list().unwrap();
        let plugin = plugins.iter().find(|plugin| plugin.id == "settings-plugin").unwrap();
        assert!(!plugin.valid);
        assert!(plugin.error.as_deref().unwrap().contains("integrity.filesSha256"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn registry_rejects_duplicate_contribution_ids() {
        let root = temp_root("duplicate-contribution");
        let html_hash = write_plugin_file(
            &root,
            "duplicate-plugin",
            "ui/index.html",
            "<!doctype html><title>dup</title>",
        );

        write_manifest(
            &root,
            "duplicate-plugin",
            serde_json::json!({
                "manifestVersion": 1,
                "id": "duplicate-plugin",
                "name": "Duplicate Plugin",
                "version": "0.1.0",
                "runtime": { "ui": { "entry": "ui/index.html" } },
                "integrity": { "filesSha256": { "ui/index.html": html_hash } },
                "permissions": {
                    "network": { "mode": "blocked", "allowHosts": [] },
                    "ui": ["route:create", "command:create"]
                },
                "contributes": {
                    "routes": [{ "id": "duplicate.id", "path": "/plugins/duplicate-plugin" }],
                    "commands": [{ "id": "duplicate.id" }]
                }
            }),
        );

        let registry = PluginRegistryState::new(root.clone()).unwrap();
        let plugins = registry.list().unwrap();
        let plugin = plugins.iter().find(|plugin| plugin.id == "duplicate-plugin").unwrap();
        assert!(!plugin.valid);
        assert!(plugin.error.as_deref().unwrap().contains("duplicated contribution id"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn registry_rejects_route_outside_plugin_namespace() {
        let root = temp_root("bad-route");
        let html_hash = write_plugin_file(
            &root,
            "bad-route-plugin",
            "ui/index.html",
            "<!doctype html><title>bad route</title>",
        );

        write_manifest(
            &root,
            "bad-route-plugin",
            serde_json::json!({
                "manifestVersion": 1,
                "id": "bad-route-plugin",
                "name": "Bad Route Plugin",
                "version": "0.1.0",
                "runtime": { "ui": { "entry": "ui/index.html" } },
                "integrity": { "filesSha256": { "ui/index.html": html_hash } },
                "permissions": {
                    "network": { "mode": "blocked", "allowHosts": [] },
                    "ui": ["route:create"]
                },
                "contributes": {
                    "routes": [{ "id": "bad.route", "path": "/settings" }]
                }
            }),
        );

        let registry = PluginRegistryState::new(root.clone()).unwrap();
        let plugins = registry.list().unwrap();
        let plugin = plugins.iter().find(|plugin| plugin.id == "bad-route-plugin").unwrap();
        assert!(!plugin.valid);
        assert!(plugin.error.as_deref().unwrap().contains("must use a path inside"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn registry_requires_ui_permission_for_route_contributions() {
        let root = temp_root("missing-permission");
        let html_hash = write_plugin_file(
            &root,
            "permission-plugin",
            "ui/index.html",
            "<!doctype html><title>perm</title>",
        );

        write_manifest(
            &root,
            "permission-plugin",
            serde_json::json!({
                "manifestVersion": 1,
                "id": "permission-plugin",
                "name": "Permission Plugin",
                "version": "0.1.0",
                "runtime": { "ui": { "entry": "ui/index.html" } },
                "integrity": { "filesSha256": { "ui/index.html": html_hash } },
                "permissions": {
                    "network": { "mode": "blocked", "allowHosts": [] },
                    "ui": []
                },
                "contributes": {
                    "routes": [{ "id": "permission.route", "path": "/plugins/permission-plugin" }]
                }
            }),
        );

        let registry = PluginRegistryState::new(root.clone()).unwrap();
        let plugins = registry.list().unwrap();
        let plugin = plugins.iter().find(|plugin| plugin.id == "permission-plugin").unwrap();
        assert!(!plugin.valid);
        assert!(plugin.error.as_deref().unwrap().contains("permissions.ui"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn registry_rejects_missing_capability_schema_file() {
        let root = temp_root("missing-capability-schema");
        let html_hash = write_plugin_file(
            &root,
            "capability-plugin",
            "ui/index.html",
            "<!doctype html><title>cap</title>",
        );

        write_manifest(
            &root,
            "capability-plugin",
            serde_json::json!({
                "manifestVersion": 1,
                "id": "capability-plugin",
                "name": "Capability Plugin",
                "version": "0.1.0",
                "runtime": { "ui": { "entry": "ui/index.html" } },
                "integrity": { "filesSha256": { "ui/index.html": html_hash } },
                "permissions": {
                    "network": { "mode": "blocked", "allowHosts": [] },
                    "agent": ["capability:declare"]
                },
                "contributes": {
                    "agentCapabilities": [{
                        "id": "subtitle.translate_video",
                        "kind": "workflow",
                        "inputSchema": "schemas/missing.schema.json",
                        "transport": { "type": "pluginCall", "function": "translateVideo" }
                    }]
                }
            }),
        );

        let registry = PluginRegistryState::new(root.clone()).unwrap();
        let plugins = registry.list().unwrap();
        let plugin = plugins.iter().find(|plugin| plugin.id == "capability-plugin").unwrap();
        assert!(!plugin.valid);
        assert!(plugin.error.as_deref().unwrap().contains("integrity.filesSha256"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn registry_rejects_duplicate_capability_ids_across_plugins() {
        let root = temp_root("duplicate-capability");
        let first_html = write_plugin_file(
            &root,
            "first-plugin",
            "ui/index.html",
            "<!doctype html><title>first</title>",
        );
        let second_html = write_plugin_file(
            &root,
            "second-plugin",
            "ui/index.html",
            "<!doctype html><title>second</title>",
        );

        write_manifest(
            &root,
            "first-plugin",
            serde_json::json!({
                "manifestVersion": 1,
                "id": "first-plugin",
                "name": "First Plugin",
                "version": "0.1.0",
                "runtime": { "ui": { "entry": "ui/index.html" } },
                "integrity": { "filesSha256": { "ui/index.html": first_html } },
                "permissions": {
                    "network": { "mode": "blocked", "allowHosts": [] },
                    "agent": ["capability:declare"]
                },
                "contributes": {
                    "agentCapabilities": [{
                        "id": "shared.capability",
                        "kind": "tool",
                        "transport": { "type": "pluginCall", "function": "run" }
                    }]
                }
            }),
        );

        write_manifest(
            &root,
            "second-plugin",
            serde_json::json!({
                "manifestVersion": 1,
                "id": "second-plugin",
                "name": "Second Plugin",
                "version": "0.1.0",
                "runtime": { "ui": { "entry": "ui/index.html" } },
                "integrity": { "filesSha256": { "ui/index.html": second_html } },
                "permissions": {
                    "network": { "mode": "blocked", "allowHosts": [] },
                    "agent": ["capability:declare"]
                },
                "contributes": {
                    "agentCapabilities": [{
                        "id": "shared.capability",
                        "kind": "tool",
                        "transport": { "type": "pluginCall", "function": "run" }
                    }]
                }
            }),
        );

        let registry = PluginRegistryState::new(root.clone()).unwrap();
        let plugins = registry.list().unwrap();
        let invalid_duplicate = plugins
            .iter()
            .find(|plugin| {
                !plugin.valid
                    && plugin
                        .error
                        .as_deref()
                        .is_some_and(|error| error.contains("duplicated capability id"))
            })
            .expect("one plugin should be rejected for duplicate capability id");
        assert!(matches!(invalid_duplicate.id.as_str(), "first-plugin" | "second-plugin"));

        fs::remove_dir_all(root).unwrap();
    }
}
