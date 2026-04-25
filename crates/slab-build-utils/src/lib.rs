use anyhow::{Context, Result, anyhow};
use bindgen::Builder as BindgenBuilder;
use cargo_metadata::MetadataCommand;
use slab_libfetch::{Api, Manifest};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ArtifactLayout {
    pub name: String,
    pub root_dir: PathBuf,
    pub include_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct VendorLayout {
    pub vendor_root: PathBuf,
    pub manifest_path: PathBuf,
    pub primary: ArtifactLayout,
    pub include_deps: Vec<ArtifactLayout>,
}

impl VendorLayout {
    pub fn artifact(&self, name: &str) -> Option<&ArtifactLayout> {
        if self.primary.name == name {
            return Some(&self.primary);
        }

        self.include_deps.iter().find(|artifact| artifact.name == name)
    }
}

pub fn ensure_vendor_layout(primary: &str, include_deps: &[&str]) -> Result<VendorLayout> {
    let workspace_root = workspace_root()?;
    let vendor_root = workspace_root.join("vendor");
    let manifest_path = vendor_root.join("slab-artifacts.toml");
    emit_rerun_directives(&manifest_path);
    std::fs::create_dir_all(&vendor_root)
        .with_context(|| format!("failed to create vendor directory {}", vendor_root.display()))?;

    let manifest = Manifest::from_file(&manifest_path)
        .with_context(|| format!("failed to load manifest {}", manifest_path.display()))?;

    let include_deps = include_deps
        .iter()
        .map(|artifact| install_artifact(&manifest, &vendor_root, artifact))
        .collect::<Result<Vec<_>>>()?;
    let primary = install_artifact(&manifest, &vendor_root, primary)?;

    Ok(VendorLayout { vendor_root, manifest_path, primary, include_deps })
}

pub fn workspace_target_dir(profile: &str) -> Result<PathBuf> {
    Ok(workspace_root()?.join("target").join(profile))
}

pub fn configure_bindgen_builder<I, P>(
    header: &str,
    include_dirs: I,
    dynamic_library_name: &str,
) -> BindgenBuilder
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let mut builder = BindgenBuilder::default()
        .header(header)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .dynamic_library_name(dynamic_library_name);

    for include_dir in include_dirs {
        builder = builder.clang_arg(clang_include_arg(include_dir.as_ref()));
    }

    builder
}

pub fn generate_or_copy_bindings(
    builder: BindgenBuilder,
    out_dir: &Path,
    fallback_source: &Path,
) -> Result<()> {
    let output_path = out_dir.join("bindings.rs");

    match builder.generate() {
        Ok(bindings) => {
            bindings.write_to_file(&output_path).with_context(|| {
                format!("failed to write generated bindings to {}", output_path.display())
            })?;
        }
        Err(error) => {
            println!("cargo:warning=Unable to generate bindings: {error}");
            println!(
                "cargo:warning=Using bundled bindings.rs fallback at {}",
                fallback_source.display()
            );
            copy_fallback_bindings(fallback_source, &output_path)?;
        }
    }

    Ok(())
}

pub fn sync_tauri_sidecar(
    bin_name: &str,
    target: &str,
    src_dir: &Path,
    tauri_manifest_dir: &Path,
) -> Result<()> {
    let extension = if target.contains("windows") { ".exe" } else { "" };
    let src_path = src_dir.join(format!("{bin_name}{extension}"));

    let sidecar_dir = tauri_manifest_dir.join("binaries");
    let dst_path = sidecar_dir.join(format!("{bin_name}-{target}{extension}"));

    fs::create_dir_all(&sidecar_dir).with_context(|| {
        format!("failed to create tauri sidecar directory {}", sidecar_dir.display())
    })?;

    if src_path.exists() {
        if should_copy_file(&src_path, &dst_path) {
            fs::copy(&src_path, &dst_path).with_context(|| {
                format!(
                    "failed to copy sidecar binary {} -> {}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
        }
        println!("cargo:rerun-if-changed={}", src_path.display());
    } else {
        println!(
            "cargo:warning=Sidecar [{}] not found at {}. Build it before packaging.",
            bin_name,
            src_path.display()
        );
    }

    Ok(())
}

pub fn sync_tauri_vendor_runtime_artifacts(target: &str, tauri_manifest_dir: &Path) -> Result<()> {
    let vendor_dir = workspace_root()?.join("vendor");
    let resources_dir = tauri_manifest_dir.join("resources").join("libs");
    let source_subdir = runtime_source_subdir(target);

    println!("cargo:rerun-if-changed={}", vendor_dir.display());

    fs::create_dir_all(&resources_dir).with_context(|| {
        format!("failed to create tauri runtime resources directory {}", resources_dir.display())
    })?;

    let mut expected_files = HashSet::new();
    let mut found_runtime_sources = false;

    for artifact in ["ggml", "llama", "whisper", "diffusion"] {
        let source_root = vendor_dir.join(artifact).join(source_subdir);
        if !source_root.exists() {
            println!(
                "cargo:warning=Vendored runtime artifact root missing for {} at {}",
                artifact,
                source_root.display()
            );
            continue;
        }

        found_runtime_sources = true;
        sync_runtime_tree(target, &source_root, &resources_dir, &mut expected_files)?;
    }

    if !found_runtime_sources {
        println!(
            "cargo:warning=No vendored runtime artifacts found under {}",
            vendor_dir.display()
        );
    }

    prune_stale_runtime_files(&resources_dir, &expected_files)?;
    Ok(())
}

pub fn sync_tauri_bundled_plugins(tauri_manifest_dir: &Path) -> Result<()> {
    let source_root = workspace_root()?.join("plugins").join("dist");
    let resources_root = tauri_manifest_dir.join("resources").join("plugins");
    let resources_dir = resources_root.join("dist");

    println!("cargo:rerun-if-changed={}", source_root.display());

    fs::create_dir_all(&resources_root).with_context(|| {
        format!("failed to create tauri bundled plugins root {}", resources_root.display())
    })?;

    let mut expected_files = HashSet::new();
    ensure_plugin_resource_placeholder(&resources_root, &mut expected_files)?;

    if !source_root.exists() {
        prune_stale_plugin_files(&resources_root, &expected_files)?;
        return Ok(());
    }

    fs::create_dir_all(&resources_dir).with_context(|| {
        format!("failed to create tauri bundled plugin packs directory {}", resources_dir.display())
    })?;

    sync_plugin_tree(&source_root, &resources_dir, &mut expected_files)?;
    prune_stale_plugin_files(&resources_root, &expected_files)?;
    Ok(())
}

fn emit_rerun_directives(manifest_path: &Path) {
    println!("cargo:rerun-if-changed={}", manifest_path.display());
    for var in ["HTTP_PROXY", "HTTPS_PROXY", "CUDA_PATH", "VULKAN_SDK"] {
        println!("cargo:rerun-if-env-changed={var}");
    }
}

pub fn workspace_root() -> Result<PathBuf> {
    let metadata =
        MetadataCommand::new().no_deps().exec().context("failed to query cargo metadata")?;
    Ok(metadata.workspace_root.into_std_path_buf())
}

fn should_copy_file(src_path: &Path, dst_path: &Path) -> bool {
    if !dst_path.exists() {
        return true;
    }

    let src_meta = match fs::metadata(src_path) {
        Ok(meta) => meta,
        Err(_) => return true,
    };
    let dst_meta = match fs::metadata(dst_path) {
        Ok(meta) => meta,
        Err(_) => return true,
    };

    if src_meta.len() != dst_meta.len() {
        return true;
    }

    let src_mtime = src_meta.modified().ok();
    let dst_mtime = dst_meta.modified().ok();
    match (src_mtime, dst_mtime) {
        (Some(src), Some(dst)) => src > dst,
        _ => true,
    }
}

fn runtime_source_subdir(target: &str) -> &'static str {
    if target.contains("windows") { "bin" } else { "lib" }
}

fn sync_plugin_runtime_file(
    src_path: &Path,
    dst_path: &Path,
    expected: &mut HashSet<PathBuf>,
) -> Result<()> {
    if let Some(parent) = dst_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("failed to create bundled plugin parent directory {}", parent.display())
        })?;
    }

    if should_copy_file(src_path, dst_path) {
        fs::copy(src_path, dst_path).with_context(|| {
            format!(
                "failed to copy bundled plugin runtime file {} -> {}",
                src_path.display(),
                dst_path.display()
            )
        })?;
    }
    expected.insert(dst_path.to_path_buf());
    Ok(())
}

fn ensure_plugin_resource_placeholder(
    resources_root: &Path,
    expected: &mut HashSet<PathBuf>,
) -> Result<()> {
    let placeholder_path = resources_root.join("placeholder.txt");
    let placeholder_contents = "Generated plugin resource root for Tauri bundles.\n";

    let should_write = match fs::read_to_string(&placeholder_path) {
        Ok(existing) => existing != placeholder_contents,
        Err(_) => true,
    };
    if should_write {
        fs::write(&placeholder_path, placeholder_contents).with_context(|| {
            format!(
                "failed to write bundled plugin placeholder file {}",
                placeholder_path.display()
            )
        })?;
    }

    expected.insert(placeholder_path);
    Ok(())
}

fn sync_plugin_tree(src_dir: &Path, dst_dir: &Path, expected: &mut HashSet<PathBuf>) -> Result<()> {
    fs::create_dir_all(dst_dir).with_context(|| {
        format!("failed to create bundled plugin directory {}", dst_dir.display())
    })?;

    let entries = fs::read_dir(src_dir).with_context(|| {
        format!("failed to read bundled plugin directory {}", src_dir.display())
    })?;

    for entry in entries {
        let entry = entry.with_context(|| {
            format!("failed to read entry under bundled plugin directory {}", src_dir.display())
        })?;
        let src_path = entry.path();
        let dst_path = dst_dir.join(entry.file_name());
        if src_path.is_dir() {
            sync_plugin_tree(&src_path, &dst_path, expected)?;
        } else {
            sync_plugin_runtime_file(&src_path, &dst_path, expected)?;
        }
    }

    Ok(())
}

fn prune_stale_plugin_files(root: &Path, expected: &HashSet<PathBuf>) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }

    prune_stale_plugin_files_inner(root, expected)?;
    Ok(())
}

fn prune_stale_plugin_files_inner(current_dir: &Path, expected: &HashSet<PathBuf>) -> Result<bool> {
    let mut has_entries = false;
    let entries = fs::read_dir(current_dir).with_context(|| {
        format!("failed to read bundled plugin directory {}", current_dir.display())
    })?;

    for entry in entries {
        let entry = entry.with_context(|| {
            format!("failed to read entry under bundled plugin directory {}", current_dir.display())
        })?;
        let path = entry.path();
        if path.is_dir() {
            let child_has_entries = prune_stale_plugin_files_inner(&path, expected)?;
            if !child_has_entries {
                fs::remove_dir(&path).with_context(|| {
                    format!("failed to remove stale bundled plugin directory {}", path.display())
                })?;
            } else {
                has_entries = true;
            }
            continue;
        }

        if expected.contains(&path) {
            has_entries = true;
            continue;
        }

        fs::remove_file(&path).with_context(|| {
            format!("failed to remove stale bundled plugin file {}", path.display())
        })?;
    }

    Ok(has_entries)
}

fn sync_runtime_tree(
    target: &str,
    src_root: &Path,
    dst_root: &Path,
    expected: &mut HashSet<PathBuf>,
) -> Result<()> {
    sync_runtime_tree_inner(target, src_root, src_root, dst_root, expected)
}

fn sync_runtime_tree_inner(
    target: &str,
    src_root: &Path,
    current_dir: &Path,
    dst_root: &Path,
    expected: &mut HashSet<PathBuf>,
) -> Result<()> {
    let entries = fs::read_dir(current_dir).with_context(|| {
        format!("failed to read vendored runtime directory {}", current_dir.display())
    })?;

    for entry in entries {
        let entry = entry.with_context(|| {
            format!(
                "failed to read entry under vendored runtime directory {}",
                current_dir.display()
            )
        })?;
        let src_path = entry.path();
        let relative = src_path.strip_prefix(src_root).with_context(|| {
            format!("failed to compute runtime relative path for {}", src_path.display())
        })?;
        let dst_path = dst_root.join(relative);

        if src_path.is_dir() {
            if should_descend_into_runtime_dir(target) {
                fs::create_dir_all(&dst_path).with_context(|| {
                    format!("failed to create runtime resource directory {}", dst_path.display())
                })?;
                sync_runtime_tree_inner(target, src_root, &src_path, dst_root, expected)?;
            }
            continue;
        }

        if !should_sync_runtime_file(target, &src_path) {
            continue;
        }

        if let Some(parent) = dst_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create runtime resource parent directory {}", parent.display())
            })?;
        }

        if should_copy_file(&src_path, &dst_path) {
            fs::copy(&src_path, &dst_path).with_context(|| {
                format!(
                    "failed to copy vendored runtime file {} -> {}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
        }

        expected.insert(dst_path);
    }

    Ok(())
}

fn should_descend_into_runtime_dir(target: &str) -> bool {
    target.contains("windows")
}

fn should_sync_runtime_file(target: &str, path: &Path) -> bool {
    if target.contains("windows") {
        return true;
    }

    let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
    if target.contains("apple") {
        return file_name.ends_with(".dylib");
    }

    file_name.contains(".so")
}

fn prune_stale_runtime_files(root: &Path, expected: &HashSet<PathBuf>) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }

    prune_stale_runtime_files_inner(root, expected)?;
    Ok(())
}

fn prune_stale_runtime_files_inner(
    current_dir: &Path,
    expected: &HashSet<PathBuf>,
) -> Result<bool> {
    let mut has_entries = false;
    let entries = fs::read_dir(current_dir).with_context(|| {
        format!("failed to read runtime resource directory {}", current_dir.display())
    })?;

    for entry in entries {
        let entry = entry.with_context(|| {
            format!(
                "failed to read entry under runtime resource directory {}",
                current_dir.display()
            )
        })?;
        let path = entry.path();
        if path.is_dir() {
            let child_has_entries = prune_stale_runtime_files_inner(&path, expected)?;
            if !child_has_entries {
                fs::remove_dir(&path).with_context(|| {
                    format!("failed to remove stale runtime directory {}", path.display())
                })?;
            } else {
                has_entries = true;
            }
            continue;
        }

        if expected.contains(&path) {
            has_entries = true;
            continue;
        }

        fs::remove_file(&path)
            .with_context(|| format!("failed to remove stale runtime file {}", path.display()))?;
    }

    Ok(has_entries)
}

fn install_artifact(
    manifest: &Manifest,
    vendor_root: &Path,
    artifact_name: &str,
) -> Result<ArtifactLayout> {
    let root_dir = vendor_root.join(artifact_name);
    let include_dir = root_dir.join("include");

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to create tokio runtime for artifact download")?;
    runtime.block_on(async {
        Api::new()
            .set_install_dir(&root_dir)
            .from_manifest(manifest, artifact_name)?
            .install_with_platform()
            .await
    })?;

    if !include_dir.is_dir() {
        return Err(anyhow!(
            "artifact '{}' is missing include directory at {}",
            artifact_name,
            include_dir.display()
        ));
    }

    Ok(ArtifactLayout { name: artifact_name.to_string(), root_dir, include_dir })
}

fn clang_include_arg(include_dir: &Path) -> String {
    format!("-I{}", include_dir.display())
}

fn copy_fallback_bindings(fallback_source: &Path, output_path: &Path) -> Result<()> {
    if !fallback_source.is_file() {
        return Err(anyhow!(
            "Unable to generate bindings and bundled fallback is missing at {}",
            fallback_source.display()
        ));
    }

    fs::copy(fallback_source, output_path).with_context(|| {
        format!(
            "failed to copy fallback bindings {} -> {}",
            fallback_source.display(),
            output_path.display()
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{clang_include_arg, copy_fallback_bindings};
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn clang_include_arg_prefixes_include_flag() {
        let include_dir = PathBuf::from("vendor").join("ggml").join("include");

        assert_eq!(clang_include_arg(&include_dir), format!("-I{}", include_dir.display()));
    }

    #[test]
    fn copy_fallback_bindings_copies_existing_file() {
        let root = temp_dir("copy-fallback");
        let fallback_source = root.join("src").join("bindings.rs");
        let output_path = root.join("out").join("bindings.rs");
        let contents = "// fallback bindings\n";

        fs::create_dir_all(fallback_source.parent().expect("fallback parent")).unwrap();
        fs::create_dir_all(output_path.parent().expect("out parent")).unwrap();
        fs::write(&fallback_source, contents).unwrap();

        copy_fallback_bindings(&fallback_source, &output_path).unwrap();

        assert_eq!(fs::read_to_string(&output_path).unwrap(), contents);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn copy_fallback_bindings_errors_when_missing() {
        let root = temp_dir("missing-fallback");
        let fallback_source = root.join("src").join("bindings.rs");
        let output_path = root.join("out").join("bindings.rs");

        let error = copy_fallback_bindings(&fallback_source, &output_path).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("Unable to generate bindings and bundled fallback is missing")
        );
        fs::remove_dir_all(root).unwrap();
    }

    fn temp_dir(label: &str) -> PathBuf {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH).expect("system time").as_nanos();
        let root = env::temp_dir().join(format!("slab-build-utils-{label}-{suffix}"));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
