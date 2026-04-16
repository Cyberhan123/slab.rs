use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::RwLock;

use percent_encoding::percent_decode_str;
use sha2::{Digest, Sha256};
use tauri::Manager;
use tauri::path::BaseDirectory;

use super::types::{PluginInfo, PluginManifest, PluginNetworkMode};

pub const DEFAULT_PLUGINS_DIR: &str = "plugins";

#[derive(Clone)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub root_dir: PathBuf,
    pub ui_entry: String,
    pub wasm_entry_path: PathBuf,
    pub files_sha256: HashMap<String, String>,
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
                ui_entry: Some(plugin.ui_entry.clone()),
                network_mode: network_mode_label(&plugin.manifest.network.mode).to_string(),
                allow_hosts: plugin.manifest.network.allow_hosts.clone(),
            });
        }

        for (id, error) in &guard.invalid {
            rows.push(PluginInfo {
                id: id.clone(),
                name: id.clone(),
                version: "invalid".to_string(),
                valid: false,
                error: Some(error.clone()),
                ui_entry: None,
                network_mode: "blocked".to_string(),
                allow_hosts: Vec::new(),
            });
        }

        rows.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(rows)
    }
}

pub fn resolve_plugins_root<R: tauri::Runtime>(app: &tauri::App<R>) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data directory for plugins: {e}"))?;
    let bundled_plugins_dir = app.path().resolve(DEFAULT_PLUGINS_DIR, BaseDirectory::Resource).ok();
    let current_dir = std::env::current_dir().ok();

    Ok(resolve_plugins_root_with(
        std::env::var("SLAB_PLUGINS_DIR").ok().map(PathBuf::from),
        bundled_plugins_dir,
        current_dir,
        app_data_dir,
    ))
}

fn resolve_plugins_root_with(
    explicit_root: Option<PathBuf>,
    bundled_root: Option<PathBuf>,
    current_dir: Option<PathBuf>,
    app_data_dir: PathBuf,
) -> PathBuf {
    if let Some(path) = explicit_root {
        return path;
    }

    if let Some(path) = bundled_root.filter(|path| path.exists()) {
        return path;
    }

    if let Some(path) =
        current_dir.map(|dir| dir.join(DEFAULT_PLUGINS_DIR)).filter(|path| path.exists())
    {
        return path;
    }

    app_data_dir.join(DEFAULT_PLUGINS_DIR)
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
        let plugin_dir = entry.path();
        let manifest_path = plugin_dir.join("plugin.json");
        if !manifest_path.exists() {
            invalid.insert(folder_name, "missing plugin.json".to_string());
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

    let ui_entry = normalize_relative_path(&manifest.ui.entry)?;
    let wasm_entry = normalize_relative_path(&manifest.wasm.entry)?;
    let ui_entry_path = plugin_dir.join(&ui_entry);
    let wasm_entry_path = plugin_dir.join(&wasm_entry);

    if !ui_entry_path.is_file() {
        return Err(format!("missing UI entry file at {}", ui_entry_path.display()));
    }
    if !wasm_entry_path.is_file() {
        return Err(format!("missing wasm entry file at {}", wasm_entry_path.display()));
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

    if !files_sha256.contains_key(&ui_entry) {
        return Err("integrity.filesSha256 must contain ui.entry".to_string());
    }
    if !files_sha256.contains_key(&wasm_entry) {
        return Err("integrity.filesSha256 must contain wasm.entry".to_string());
    }

    if manifest.network.mode == PluginNetworkMode::Blocked
        && !manifest.network.allow_hosts.is_empty()
    {
        return Err("network.allowHosts must be empty when mode is `blocked`".to_string());
    }

    Ok(LoadedPlugin {
        manifest,
        root_dir: plugin_dir.to_path_buf(),
        ui_entry,
        wasm_entry_path,
        files_sha256,
    })
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
    fn plugins_root_prefers_existing_dev_directory() {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("slab-plugin-root-{suffix}"));
        let cwd = root.join("repo");
        let app_data_dir = root.join("app-data");
        let dev_plugins_dir = cwd.join(DEFAULT_PLUGINS_DIR);
        fs::create_dir_all(&dev_plugins_dir).unwrap();

        let resolved =
            resolve_plugins_root_with(None, None, Some(cwd.clone()), app_data_dir.clone());

        assert_eq!(resolved, dev_plugins_dir);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn plugins_root_falls_back_to_app_data_when_cwd_has_no_plugins() {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("slab-plugin-root-{suffix}"));
        let cwd = root.join("launchd-cwd");
        let app_data_dir = root.join("app-data");
        fs::create_dir_all(&cwd).unwrap();

        let resolved = resolve_plugins_root_with(None, None, Some(cwd), app_data_dir.clone());

        assert_eq!(resolved, app_data_dir.join(DEFAULT_PLUGINS_DIR));

        fs::remove_dir_all(root).unwrap();
    }
}
