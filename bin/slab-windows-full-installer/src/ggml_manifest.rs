use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;

use crate::fsops::normalize_relative_path;
use crate::payload::{ResolvedPayloadFile, RuntimeVariant};

const GGML_BASE_DLL: &str = "bin/ggml-base.dll";
const GGML_VULKAN_DLL: &str = "bin/ggml-vulkan.dll";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GgmlManifest {
    schema_version: u32,
    artifact: GgmlArtifact,
    components: Vec<GgmlComponent>,
    #[serde(alias = "entries")]
    files: Vec<GgmlEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GgmlArtifact {
    os: String,
    arch: String,
}

#[derive(Debug, Deserialize)]
struct GgmlComponent {
    id: String,
    files: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GgmlEntry {
    path: String,
    role: String,
    size: u64,
    sha256: String,
}

pub fn resolve_ggml_runtime_packages(
    manifest_path: &Path,
    vendor_root: &Path,
) -> Result<BTreeMap<RuntimeVariant, Vec<ResolvedPayloadFile>>> {
    let manifest: GgmlManifest = serde_json::from_reader(
        std::fs::File::open(manifest_path)
            .with_context(|| format!("failed to open {}", manifest_path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", manifest_path.display()))?;

    validate_manifest(&manifest, manifest_path)?;

    let components_by_id: HashMap<_, _> =
        manifest.components.iter().map(|component| (component.id.as_str(), component)).collect();
    let entry_by_path: HashMap<_, _> =
        manifest.files.iter().map(|entry| (entry.path.as_str(), entry)).collect();

    let mut packages = BTreeMap::new();
    packages.insert(RuntimeVariant::Base, Vec::new());
    packages.insert(RuntimeVariant::Cuda, Vec::new());
    packages.insert(RuntimeVariant::Hip, Vec::new());

    for (variant, component_id) in [
        (RuntimeVariant::Base, "base"),
        (RuntimeVariant::Cuda, "cuda"),
        (RuntimeVariant::Hip, "hip"),
    ] {
        let component = components_by_id
            .get(component_id)
            .copied()
            .with_context(|| format!("GGML component '{component_id}' is missing"))?;

        for file in &component.files {
            let entry = entry_by_path
                .get(file.as_str())
                .copied()
                .with_context(|| format!("GGML manifest entry '{}' is missing metadata", file))?;

            if !matches!(entry.role.as_str(), "runtime" | "runtime-library") {
                continue;
            }

            let target_variant = remap_variant(file, variant);
            let runtime_relative = file
                .strip_prefix("bin/")
                .ok_or_else(|| anyhow!("GGML runtime file '{}' is not under bin/", file))?;
            let source_path = vendor_root.join(pathbuf_from_forward_slashes(file));
            let source_relative_path =
                format!("resources/libs/{}", runtime_relative.replace('\\', "/"));

            packages.get_mut(&target_variant).expect("package initialized").push(
                ResolvedPayloadFile {
                    source_path,
                    source_relative_path,
                    dest_relative_path: normalize_relative_path(Path::new(runtime_relative))?,
                    size: entry.size,
                    sha256: entry.sha256.clone(),
                },
            );
        }
    }

    ensure_remapped_file_present(&packages, RuntimeVariant::Base, GGML_BASE_DLL, "ggml-base.dll")?;
    ensure_remapped_file_present(
        &packages,
        RuntimeVariant::Base,
        GGML_VULKAN_DLL,
        "ggml-vulkan.dll",
    )?;
    ensure_file_absent(&packages, RuntimeVariant::Hip, GGML_BASE_DLL)?;

    Ok(packages)
}

fn validate_manifest(manifest: &GgmlManifest, manifest_path: &Path) -> Result<()> {
    if manifest.schema_version != 2 {
        bail!(
            "GGML manifest '{}' has unsupported schemaVersion {}; expected 2",
            manifest_path.display(),
            manifest.schema_version
        );
    }

    if manifest.artifact.os != "windows" {
        bail!(
            "GGML manifest '{}' targets os='{}'; expected 'windows'",
            manifest_path.display(),
            manifest.artifact.os
        );
    }

    if manifest.artifact.arch != "x86_64" {
        bail!(
            "GGML manifest '{}' targets arch='{}'; expected 'x86_64'",
            manifest_path.display(),
            manifest.artifact.arch
        );
    }

    for required_component in ["base", "cuda", "hip"] {
        if !manifest.components.iter().any(|component| component.id == required_component) {
            bail!(
                "GGML manifest '{}' is missing required component '{}'",
                manifest_path.display(),
                required_component
            );
        }
    }

    Ok(())
}

fn remap_variant(path: &str, source_variant: RuntimeVariant) -> RuntimeVariant {
    match path {
        GGML_BASE_DLL | GGML_VULKAN_DLL => RuntimeVariant::Base,
        _ => source_variant,
    }
}

fn ensure_remapped_file_present(
    packages: &BTreeMap<RuntimeVariant, Vec<ResolvedPayloadFile>>,
    variant: RuntimeVariant,
    manifest_path: &str,
    label: &str,
) -> Result<()> {
    let runtime_relative = manifest_path
        .strip_prefix("bin/")
        .ok_or_else(|| anyhow!("invalid GGML manifest path '{}'", manifest_path))?;
    let package = packages
        .get(&variant)
        .ok_or_else(|| anyhow!("GGML package '{}' is missing", variant.as_str()))?;

    if package.iter().any(|file| file.dest_relative_path == runtime_relative) {
        return Ok(());
    }

    bail!(
        "GGML package '{}' does not contain required remapped file '{}'",
        variant.as_str(),
        label
    );
}

fn ensure_file_absent(
    packages: &BTreeMap<RuntimeVariant, Vec<ResolvedPayloadFile>>,
    variant: RuntimeVariant,
    manifest_path: &str,
) -> Result<()> {
    let runtime_relative = manifest_path
        .strip_prefix("bin/")
        .ok_or_else(|| anyhow!("invalid GGML manifest path '{}'", manifest_path))?;
    let package = packages
        .get(&variant)
        .ok_or_else(|| anyhow!("GGML package '{}' is missing", variant.as_str()))?;

    if package.iter().any(|file| file.dest_relative_path == runtime_relative) {
        bail!("GGML package '{}' unexpectedly contains '{}'", variant.as_str(), manifest_path);
    }

    Ok(())
}

fn pathbuf_from_forward_slashes(path: &str) -> PathBuf {
    let mut output = PathBuf::new();
    for segment in path.split('/') {
        output.push(segment);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn remaps_ggml_base_into_base_package() -> Result<()> {
        let root = std::env::temp_dir().join(format!("slab-ggml-test-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("bin"))?;
        fs::write(root.join("manifests.json"), minimal_manifest())?;
        fs::write(root.join("bin").join("ggml.dll"), b"ggml")?;
        fs::write(root.join("bin").join("ggml-vulkan.dll"), b"vulkan")?;
        fs::write(root.join("bin").join("ggml-base.dll"), b"base")?;
        fs::write(root.join("bin").join("ggml-hip.dll"), b"hip")?;
        fs::write(root.join("bin").join("ggml-cuda.dll"), b"cuda")?;

        let packages = resolve_ggml_runtime_packages(&root.join("manifests.json"), &root)?;

        let base = packages.get(&RuntimeVariant::Base).unwrap();
        let hip = packages.get(&RuntimeVariant::Hip).unwrap();

        assert!(base.iter().any(|file| file.dest_relative_path == "ggml-base.dll"));
        assert!(base.iter().any(|file| file.dest_relative_path == "ggml-vulkan.dll"));
        assert!(!hip.iter().any(|file| file.dest_relative_path == "ggml-base.dll"));

        fs::remove_dir_all(root)?;
        Ok(())
    }

    fn minimal_manifest() -> &'static str {
        r#"{
  "schemaVersion": 2,
  "artifact": {
    "os": "windows",
    "arch": "x86_64"
  },
  "components": [
    { "id": "base", "files": ["bin/ggml.dll", "bin/ggml-vulkan.dll"] },
    { "id": "cuda", "files": ["bin/ggml-cuda.dll"] },
    { "id": "hip", "files": ["bin/ggml-base.dll", "bin/ggml-hip.dll"] }
  ],
  "files": [
    { "path": "bin/ggml.dll", "role": "runtime-library", "size": 4, "sha256": "01020304" },
    { "path": "bin/ggml-vulkan.dll", "role": "runtime-library", "size": 6, "sha256": "02030405" },
    { "path": "bin/ggml-cuda.dll", "role": "runtime-library", "size": 4, "sha256": "03040506" },
    { "path": "bin/ggml-base.dll", "role": "runtime-library", "size": 4, "sha256": "04050607" },
    { "path": "bin/ggml-hip.dll", "role": "runtime-library", "size": 3, "sha256": "05060708" }
  ]
}"#
    }
}
