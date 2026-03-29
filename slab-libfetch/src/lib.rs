pub mod api;
pub mod downloader;
pub mod error;
pub mod install;
pub mod manifest;
pub mod platform;
pub mod variant;
pub mod verify;

pub use api::{Api, RepoApi, VersionApi};
pub use error::FetchError;
pub use install::VersionInfo;
pub use manifest::{
    ArtifactSpec, ArtifactTarget, Manifest, ManifestMetadata, ResolvedArtifact, VariantMatrix,
};
pub use platform::{Arch, Os, Platform};
pub use variant::Variant;

use std::path::Path;

/// Download header files for a GitHub repository and extract them to
/// `target_include_path`.
///
/// This is a convenience wrapper around the builder API:
/// ```rust,ignore
/// Api::new().repo("owner/repo").version("vX.Y.Z").fetch_header(path).await
/// ```
///
/// When `tag` is `None` the latest release is used.  The download is skipped
/// if `target_include_path/version.json` already records the same version.
pub async fn fetch_header(
    owner: &str,
    repo: &str,
    tag: Option<&str>,
    target_include_path: &Path,
) -> Result<(), FetchError> {
    let repo_full = format!("{}/{}", owner, repo);
    let install_dir = target_include_path
        .to_str()
        .ok_or_else(|| FetchError::InvalidPath {
            message: format!(
                "target_include_path contains invalid UTF-8: {:?}",
                target_include_path
            ),
        })?
        .to_string();

    let version_api = match tag {
        Some(t) => Api::new().set_install_dir(install_dir).repo(repo_full).version(t),
        None => Api::new().set_install_dir(install_dir).repo(repo_full).latest(),
    };

    version_api.fetch_header(target_include_path).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[tokio::test]
    async fn test_extract_include_ggml() {
        fetch_header("ggml-org", "ggml", Some("v0.9.7"), Path::new("target/ggml")).await.unwrap();
    }

    #[tokio::test]
    async fn test_extract_include_whisper() {
        fetch_header("ggml-org", "whisper.cpp", Some("v1.8.3"), Path::new("target/whisper"))
            .await
            .unwrap();
    }

    /// Integration test: load the workspace-level `vendor/slab-artifacts.toml` and
    /// verify that all declared artifacts can be resolved against at least one
    /// valid platform/variant combination.
    #[test]
    fn test_load_workspace_slab_artifacts_toml() {
        // Locate the manifest relative to CARGO_MANIFEST_DIR (slab-libfetch/).
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let manifest_path = std::path::Path::new(manifest_dir)
            .parent()
            .expect("workspace root")
            .join("vendor")
            .join("slab-artifacts.toml");

        let manifest = Manifest::from_file(&manifest_path).unwrap_or_else(|e| {
            panic!("Failed to load {}: {}", manifest_path.display(), e);
        });

        assert_eq!(manifest.metadata.schema_version, "1");

        // Each declared artifact must be resolvable for at least one platform.
        let test_cases: &[(&str, Os, Arch, Variant, &str)] = &[
            ("ggml", Os::Windows, Arch::X86_64, Variant::Cpu, ".zip"),
            ("llama", Os::Linux, Arch::X86_64, Variant::Cpu, ".tar.gz"),
            ("whisper", Os::MacOS, Arch::Aarch64, Variant::Metal, ".tar.gz"),
            ("diffusion", Os::Linux, Arch::X86_64, Variant::Vulkan, ".tar.gz"),
        ];

        for (name, os, arch, variant, expected_extension) in test_cases {
            let platform = Platform { os: os.clone(), arch: arch.clone() };
            let spec = manifest.artifact(name).unwrap_or_else(|e| {
                panic!("artifact '{}' not found: {}", name, e);
            });
            let resolved = spec.resolve(&platform, variant).unwrap_or_else(|e| {
                panic!("failed to resolve {} for {}-{} ({}): {}", name, os, arch, variant, e);
            });
            assert!(
                resolved.asset_name.ends_with(expected_extension),
                "expected {} to end with {}, got {}",
                name,
                expected_extension,
                resolved.asset_name
            );
        }
    }
}
