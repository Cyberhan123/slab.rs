use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::RwLock;

use percent_encoding::percent_decode_str;
use sha2::{Digest, Sha256};

use crate::types::{LoadedPlugin, PluginInfo, PluginManifest, PluginNetworkMode};

const IGNORED_PLUGIN_ROOT_NAMES: &[&str] = &["dist", ".git", "node_modules"];

#[derive(Default)]
struct PluginRegistrySnapshot {
    loaded: HashMap<String, LoadedPlugin>,
    invalid: HashMap<String, String>,
}

pub struct PluginRegistry {
    root_dir: PathBuf,
    snapshot: RwLock<PluginRegistrySnapshot>,
}

impl PluginRegistry {
    pub fn new(root_dir: PathBuf) -> Result<Self, String> {
        if !root_dir.exists() {
            fs::create_dir_all(&root_dir).map_err(|error| {
                format!("failed to create plugins directory {}: {error}", root_dir.display())
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
                contributions: Default::default(),
                permissions: Default::default(),
            });
        }

        rows.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(rows)
    }
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
            Component::Normal(segment) => components.push(segment.to_string_lossy().to_string()),
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

    let entries = fs::read_dir(root_dir).map_err(|error| {
        format!("failed to scan plugins directory {}: {error}", root_dir.display())
    })?;

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                invalid.insert(
                    "unknown".to_string(),
                    format!("failed to read directory entry: {error}"),
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

    let mut files_sha256 = HashMap::new();
    for (raw_path, expected_hash) in &manifest.integrity.files_sha256 {
        let normalized_path = normalize_relative_path(raw_path)?;
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

    let js_entry_path = match manifest.runtime.js.as_ref() {
        Some(js) => {
            let (entry, path) =
                validate_declared_file(plugin_dir, &files_sha256, &js.entry, "runtime.js.entry")?;
            validate_js_entry_extension(&entry)?;
            Some(path)
        }
        None => None,
    };
    let python_entry_path = match manifest.runtime.python.as_ref() {
        Some(python) => {
            let (entry, path) = validate_declared_file(
                plugin_dir,
                &files_sha256,
                &python.entry,
                "runtime.python.entry",
            )?;
            validate_python_entry_extension(&entry)?;
            Some(path)
        }
        None => None,
    };

    Ok(LoadedPlugin {
        manifest,
        root_dir: plugin_dir.to_path_buf(),
        ui_entry,
        wasm_entry_path,
        js_entry_path,
        python_entry_path,
        files_sha256,
    })
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

fn validate_js_entry_extension(entry: &str) -> Result<(), String> {
    let extension = Path::new(entry)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(extension.as_str(), "ts" | "tsx" | "js" | "mjs") {
        return Ok(());
    }
    Err("runtime.js.entry must use .ts, .tsx, .js, or .mjs".to_owned())
}

fn validate_python_entry_extension(entry: &str) -> Result<(), String> {
    let extension = Path::new(entry)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension == "py" {
        return Ok(());
    }
    Err("runtime.python.entry must use .py".to_owned())
}

fn compute_file_sha256(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hex::encode(hasher.finalize()))
}

fn network_mode_label(mode: &PluginNetworkMode) -> &'static str {
    match mode {
        PluginNetworkMode::Blocked => "blocked",
        PluginNetworkMode::Allowlist => "allowlist",
    }
}
