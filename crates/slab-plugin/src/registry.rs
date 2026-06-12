use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::RwLock;

use percent_encoding::percent_decode_str;
use slab_utils::hash::{sha256_hex_file, verify_sha256_hex_expected};

use crate::types::{
    LoadedPlugin, PluginInfo, PluginLanguageServerTransport, PluginManifest, PluginNetworkMode,
};

const IGNORED_PLUGIN_ROOT_NAMES: &[&str] = &["dist", ".git", "node_modules"];
const BUILTIN_LANGUAGE_SERVER_PLUGIN_IDS: &[&str] =
    &["native-language-servers", "web-language-servers"];

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

    pub fn loaded_plugins(&self) -> Result<Vec<LoadedPlugin>, String> {
        let guard = self
            .snapshot
            .read()
            .map_err(|_| "failed to lock plugin registry for read".to_string())?;
        Ok(guard.loaded.values().cloned().collect())
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
        if is_builtin_language_server_plugin_id(&folder_name) {
            continue;
        }
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

fn is_builtin_language_server_plugin_id(plugin_id: &str) -> bool {
    BUILTIN_LANGUAGE_SERVER_PLUGIN_IDS.contains(&plugin_id)
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

    let files_sha256 = if manifest.integrity.files_sha256.is_empty() {
        compute_development_files_sha256(plugin_dir, &manifest)?
    } else {
        validate_integrity_files(plugin_dir, &manifest.integrity.files_sha256)?
    };

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
            let entry = normalize_relative_path(&python.entry)?;
            validate_python_entry_extension(&entry)?;
            if let Some(bundle) = &python.bundle {
                let (bundle, path) = validate_declared_file(
                    plugin_dir,
                    &files_sha256,
                    bundle,
                    "runtime.python.bundle",
                )?;
                validate_python_bundle_extension(&bundle)?;
                Some(path)
            } else {
                Some(
                    validate_declared_file(
                        plugin_dir,
                        &files_sha256,
                        &python.entry,
                        "runtime.python.entry",
                    )?
                    .1,
                )
            }
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

fn validate_integrity_files(
    plugin_dir: &Path,
    declared_files: &HashMap<String, String>,
) -> Result<HashMap<String, String>, String> {
    let mut files_sha256 = HashMap::new();
    for (raw_path, expected_hash) in declared_files {
        let normalized_path = normalize_relative_path(raw_path)?;
        let file_path = plugin_dir.join(&normalized_path);
        if !file_path.is_file() {
            return Err(format!("integrity target `{normalized_path}` does not exist as a file"));
        }

        let computed_hash = compute_file_sha256(&file_path)?;
        if verify_sha256_hex_expected(&computed_hash, expected_hash).is_err() {
            return Err(format!(
                "integrity mismatch on `{normalized_path}`: expected {expected_hash}, got {computed_hash}"
            ));
        }

        files_sha256.insert(normalized_path, computed_hash);
    }
    Ok(files_sha256)
}

fn compute_development_files_sha256(
    plugin_dir: &Path,
    manifest: &PluginManifest,
) -> Result<HashMap<String, String>, String> {
    let mut paths = Vec::new();
    collect_directory_files(plugin_dir, "ui", &mut paths)?;
    collect_directory_files(plugin_dir, "schemas", &mut paths)?;
    paths.push(manifest.runtime.ui.entry.clone());
    if let Some(wasm) = &manifest.runtime.wasm {
        paths.push(wasm.entry.clone());
    }
    if let Some(js) = &manifest.runtime.js {
        paths.push(js.entry.clone());
    }
    if let Some(python) = &manifest.runtime.python {
        if let Some(bundle) = &python.bundle {
            paths.push(bundle.clone());
        } else {
            paths.push(python.entry.clone());
        }
    }
    if has_node_package_language_server(manifest) && plugin_dir.join("package.json").is_file() {
        paths.push("package.json".to_owned());
    }

    paths.sort();
    paths.dedup();

    let mut files_sha256 = HashMap::new();
    for raw_path in paths {
        let normalized_path = normalize_relative_path(&raw_path)?;
        let file_path = plugin_dir.join(&normalized_path);
        if file_path.is_file() {
            files_sha256.insert(normalized_path, compute_file_sha256(&file_path)?);
        }
    }
    Ok(files_sha256)
}

fn collect_directory_files(
    plugin_dir: &Path,
    relative_dir: &str,
    output: &mut Vec<String>,
) -> Result<(), String> {
    let root = plugin_dir.join(relative_dir);
    if !root.is_dir() {
        return Ok(());
    }
    collect_directory_files_inner(plugin_dir, &root, output)
}

fn collect_directory_files_inner(
    plugin_dir: &Path,
    current_dir: &Path,
    output: &mut Vec<String>,
) -> Result<(), String> {
    for entry in fs::read_dir(current_dir)
        .map_err(|error| format!("failed to scan {}: {error}", current_dir.display()))?
    {
        let entry = entry.map_err(|error| format!("failed to read plugin file entry: {error}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_directory_files_inner(plugin_dir, &path, output)?;
        } else if path.is_file() {
            let relative_path = path
                .strip_prefix(plugin_dir)
                .map_err(|error| format!("failed to relativize {}: {error}", path.display()))?;
            output.push(relative_path.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn has_node_package_language_server(manifest: &PluginManifest) -> bool {
    manifest.contributes.language_servers.iter().any(|provider| {
        matches!(&provider.transport, PluginLanguageServerTransport::NodePackage { .. })
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

fn validate_python_bundle_extension(entry: &str) -> Result<(), String> {
    let extension = Path::new(entry)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension == "slabpy" {
        return Ok(());
    }
    Err("runtime.python.bundle must use .slabpy".to_owned())
}

fn compute_file_sha256(path: &Path) -> Result<String, String> {
    sha256_hex_file(path).map_err(|error| format!("failed to hash {}: {error}", path.display()))
}

fn network_mode_label(mode: &PluginNetworkMode) -> &'static str {
    match mode {
        PluginNetworkMode::Blocked => "blocked",
        PluginNetworkMode::Allowlist => "allowlist",
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{PluginRegistry, is_path_within_root, normalize_relative_path};

    static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("slab-plugin-registry-{label}-{}-{sequence}", std::process::id()));
            if path.exists() {
                fs::remove_dir_all(&path).expect("remove stale test directory");
            }
            fs::create_dir_all(&path).expect("create test directory");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn normalize_relative_path_rejects_encoded_traversal_and_absolute_paths() {
        assert_eq!(
            normalize_relative_path(" /ui/%69ndex.html ").expect("normalize path"),
            "ui/index.html"
        );
        assert_eq!(
            normalize_relative_path("./ui/./index.html").expect("normalize current dirs"),
            "ui/index.html"
        );

        for raw in ["../plugin.json", "ui/%2e%2e/plugin.json", "/../plugin.json"] {
            assert!(
                normalize_relative_path(raw).is_err(),
                "{raw} should not be accepted as a plugin-relative path"
            );
        }
    }

    #[test]
    fn path_within_root_requires_existing_canonical_child() {
        let root = TestDir::new("root-check");
        let child = root.path().join("ui").join("index.html");
        fs::create_dir_all(child.parent().expect("child parent")).expect("create child parent");
        fs::write(&child, "<html></html>").expect("write child file");

        assert!(is_path_within_root(root.path(), &child));
        assert!(!is_path_within_root(root.path(), &root.path().join("missing.html")));
        assert!(!is_path_within_root(root.path(), &std::env::temp_dir()));
    }

    #[test]
    fn registry_loads_development_plugin_without_packaged_integrity() {
        let root = TestDir::new("dev-plugin");
        let plugin_dir = root.path().join("sample-plugin");
        fs::create_dir_all(plugin_dir.join("ui")).expect("create ui directory");
        fs::create_dir_all(plugin_dir.join("dist")).expect("create js directory");
        fs::write(plugin_dir.join("ui").join("index.html"), "<html></html>")
            .expect("write ui entry");
        fs::write(plugin_dir.join("dist").join("plugin.mjs"), "export function ping() {}")
            .expect("write js entry");
        fs::write(
            plugin_dir.join("plugin.json"),
            r#"{
                "manifestVersion": 1,
                "id": "sample-plugin",
                "name": "Sample Plugin",
                "version": "0.1.0",
                "runtime": {
                    "ui": { "entry": "ui/index.html" },
                    "js": { "entry": "dist/plugin.mjs" }
                },
                "permissions": {
                    "network": {
                        "mode": "allowlist",
                        "allowHosts": ["api.example.test"]
                    }
                }
            }"#,
        )
        .expect("write manifest");

        let registry = PluginRegistry::new(root.path().to_path_buf()).expect("create registry");
        let plugin = registry.get_plugin("sample-plugin").expect("load plugin");

        assert_eq!(plugin.ui_entry, "ui/index.html");
        assert!(plugin.js_entry_path.as_ref().is_some_and(|path| path.is_file()));
        assert!(plugin.files_sha256.contains_key("ui/index.html"));
        assert!(plugin.files_sha256.contains_key("dist/plugin.mjs"));

        let listed = registry.list().expect("list plugins");
        assert_eq!(listed.len(), 1);
        assert!(listed[0].valid);
        assert_eq!(listed[0].network_mode, "allowlist");
        assert_eq!(listed[0].allow_hosts, vec!["api.example.test".to_owned()]);
    }

    #[test]
    fn registry_reports_invalid_plugin_entry_without_loading_it() {
        let root = TestDir::new("invalid-plugin");
        let plugin_dir = root.path().join("sample-plugin");
        fs::create_dir_all(plugin_dir.join("ui")).expect("create ui directory");
        fs::create_dir_all(plugin_dir.join("dist")).expect("create js directory");
        fs::write(plugin_dir.join("ui").join("index.html"), "<html></html>")
            .expect("write ui entry");
        fs::write(plugin_dir.join("dist").join("plugin.txt"), "not javascript")
            .expect("write bad js entry");
        fs::write(
            plugin_dir.join("plugin.json"),
            r#"{
                "manifestVersion": 1,
                "id": "sample-plugin",
                "name": "Sample Plugin",
                "version": "0.1.0",
                "runtime": {
                    "ui": { "entry": "ui/index.html" },
                    "js": { "entry": "dist/plugin.txt" }
                }
            }"#,
        )
        .expect("write manifest");

        let registry = PluginRegistry::new(root.path().to_path_buf()).expect("create registry");
        assert!(registry.get_plugin("sample-plugin").is_err());

        let listed = registry.list().expect("list plugins");
        assert_eq!(listed.len(), 1);
        assert!(!listed[0].valid);
        assert!(
            listed[0]
                .error
                .as_deref()
                .is_some_and(|error| error.contains("runtime.js.entry must use"))
        );
    }
}
