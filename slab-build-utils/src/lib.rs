use anyhow::{Context, Result, anyhow};
use cargo_metadata::MetadataCommand;
use slab_libfetch::{Api, Manifest};
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

fn emit_rerun_directives(manifest_path: &Path) {
    println!("cargo:rerun-if-changed={}", manifest_path.display());
    for var in ["HTTP_PROXY", "HTTPS_PROXY", "CUDA_PATH", "VULKAN_SDK"] {
        println!("cargo:rerun-if-env-changed={var}");
    }
}

fn workspace_root() -> Result<PathBuf> {
    let metadata =
        MetadataCommand::new().no_deps().exec().context("failed to query cargo metadata")?;
    Ok(metadata.workspace_root.into_std_path_buf())
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
