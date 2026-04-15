use std::collections::{BTreeMap, btree_map::Entry};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::fsops::{collect_files_recursive, normalize_relative_path, sha256_file};
use crate::ggml_manifest::resolve_ggml_runtime_packages;

const PACKAGED_PAYLOAD_MANIFEST_VERSION: u32 = 1;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeVariant {
    #[default]
    Base,
    Cuda,
    Hip,
}

impl RuntimeVariant {
    pub fn cab_name(self) -> &'static str {
        match self {
            Self::Base => "base.cab",
            Self::Cuda => "cuda.cab",
            Self::Hip => "hip.cab",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Cuda => "cuda",
            Self::Hip => "hip",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum RequestedVariant {
    #[default]
    Auto,
    Base,
    Cuda,
    Hip,
}

#[derive(Clone, Debug)]
pub struct ResolvedPayloadFile {
    pub source_path: PathBuf,
    pub source_relative_path: String,
    pub dest_relative_path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Clone, Debug)]
pub struct CabPackage {
    pub variant: RuntimeVariant,
    pub files: Vec<ResolvedPayloadFile>,
}

#[derive(Clone, Debug)]
pub struct RuntimePayloadPlan {
    pub packages: Vec<CabPackage>,
    pub packaged_manifest: PackagedPayloadManifest,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagedPayloadManifest {
    pub format_version: u32,
    pub version: String,
    pub packages: Vec<PackagedPayloadPackage>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagedPayloadPackage {
    pub variant: RuntimeVariant,
    pub cab_name: String,
    pub files: Vec<PackagedPayloadFile>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagedPayloadFile {
    pub source_relative_path: String,
    pub dest_relative_path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedPayloadManifest {
    pub format_version: u32,
    pub version: String,
    pub selected_packages: Vec<RuntimeVariant>,
    pub files: Vec<SelectedPayloadFile>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedPayloadFile {
    pub source_relative_path: String,
    pub dest_relative_path: String,
    pub size: u64,
    pub sha256: String,
}

impl PackagedPayloadManifest {
    pub fn selected_for(&self, packages: &[RuntimeVariant]) -> Result<SelectedPayloadManifest> {
        let mut files = BTreeMap::new();
        let package_set: BTreeMap<_, _> =
            self.packages.iter().map(|package| (package.variant, package)).collect();

        for package in packages {
            let package_manifest = package_set.get(package).copied().with_context(|| {
                format!("payload package '{}' missing from manifest", package.as_str())
            })?;

            for file in &package_manifest.files {
                match files.entry(file.dest_relative_path.clone()) {
                    Entry::Vacant(entry) => {
                        entry.insert(SelectedPayloadFile {
                            source_relative_path: file.source_relative_path.clone(),
                            dest_relative_path: file.dest_relative_path.clone(),
                            size: file.size,
                            sha256: file.sha256.clone(),
                        });
                    }
                    Entry::Occupied(entry) => {
                        let existing = entry.get();
                        if existing.sha256 != file.sha256 || existing.size != file.size {
                            bail!(
                                "conflicting payload file '{}' between selected packages",
                                file.dest_relative_path
                            );
                        }
                    }
                }
            }
        }

        Ok(SelectedPayloadManifest {
            format_version: PACKAGED_PAYLOAD_MANIFEST_VERSION,
            version: self.version.clone(),
            selected_packages: packages.to_vec(),
            files: files.into_values().collect(),
        })
    }
}

pub fn build_runtime_payload_plan(
    workspace_root: &Path,
    version: &str,
) -> Result<RuntimePayloadPlan> {
    let vendor_root = workspace_root.join("vendor");
    let mut package_files: BTreeMap<RuntimeVariant, Vec<ResolvedPayloadFile>> = BTreeMap::new();

    package_files.insert(RuntimeVariant::Base, Vec::new());
    package_files.insert(RuntimeVariant::Cuda, Vec::new());
    package_files.insert(RuntimeVariant::Hip, Vec::new());

    for artifact in ["llama", "whisper", "diffusion"] {
        let artifact_root = vendor_root.join(artifact).join("bin");
        let files = resolve_vendor_runtime_tree(&artifact_root)
            .with_context(|| format!("failed to load vendor runtime files for '{artifact}'"))?;
        package_files
            .get_mut(&RuntimeVariant::Base)
            .expect("base package initialized")
            .extend(files);
    }

    let ggml_packages = resolve_ggml_runtime_packages(
        &vendor_root.join("ggml").join("manifests.json"),
        &vendor_root.join("ggml"),
    )?;

    for (variant, files) in ggml_packages {
        package_files.entry(variant).or_default().extend(files);
    }

    let mut packages = Vec::new();
    let mut packaged_packages = Vec::new();
    for variant in [RuntimeVariant::Base, RuntimeVariant::Cuda, RuntimeVariant::Hip] {
        let deduped = dedupe_payload_files(package_files.remove(&variant).unwrap_or_default())?;

        let manifest_files = deduped
            .iter()
            .map(|file| PackagedPayloadFile {
                source_relative_path: file.source_relative_path.clone(),
                dest_relative_path: file.dest_relative_path.clone(),
                size: file.size,
                sha256: file.sha256.clone(),
            })
            .collect();

        packages.push(CabPackage { variant, files: deduped });
        packaged_packages.push(PackagedPayloadPackage {
            variant,
            cab_name: variant.cab_name().to_string(),
            files: manifest_files,
        });
    }

    Ok(RuntimePayloadPlan {
        packages,
        packaged_manifest: PackagedPayloadManifest {
            format_version: PACKAGED_PAYLOAD_MANIFEST_VERSION,
            version: version.to_string(),
            packages: packaged_packages,
        },
    })
}

fn resolve_vendor_runtime_tree(root: &Path) -> Result<Vec<ResolvedPayloadFile>> {
    if !root.is_dir() {
        bail!("vendor runtime root is missing: {}", root.display());
    }

    let files = collect_files_recursive(root)?;
    let mut resolved = Vec::with_capacity(files.len());
    for source_path in files {
        let relative = source_path.strip_prefix(root).with_context(|| {
            format!("failed to strip vendor runtime root for {}", source_path.display())
        })?;
        let relative = normalize_relative_path(relative)?;
        let source_relative_path = format!("resources/libs/{relative}");
        let metadata = fs::metadata(&source_path)
            .with_context(|| format!("failed to read metadata for {}", source_path.display()))?;
        resolved.push(ResolvedPayloadFile {
            source_path: source_path.clone(),
            source_relative_path,
            dest_relative_path: relative,
            size: metadata.len(),
            sha256: sha256_file(&source_path)?,
        });
    }

    Ok(resolved)
}

fn dedupe_payload_files(files: Vec<ResolvedPayloadFile>) -> Result<Vec<ResolvedPayloadFile>> {
    let mut deduped = BTreeMap::new();

    for file in files {
        match deduped.entry(file.dest_relative_path.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(file);
            }
            Entry::Occupied(entry) => {
                let existing = entry.get();
                if existing.sha256 != file.sha256
                    || existing.size != file.size
                    || existing.source_relative_path != file.source_relative_path
                {
                    return Err(anyhow!(
                        "conflicting payload definitions for '{}'",
                        existing.dest_relative_path
                    ));
                }
            }
        }
    }

    Ok(deduped.into_values().collect())
}
