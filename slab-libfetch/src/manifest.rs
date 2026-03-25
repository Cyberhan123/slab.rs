use crate::error::FetchError;
use crate::platform::Platform;
use crate::variant::Variant;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Top-level structure of `slab-artifacts.toml`.
#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub metadata: ManifestMetadata,
    pub artifacts: HashMap<String, ArtifactSpec>,
}

/// Manifest-level metadata.
#[derive(Debug, Deserialize)]
pub struct ManifestMetadata {
    pub schema_version: String,
}

/// Declaration for a single downloadable artifact (e.g. `llama`, `whisper`).
#[derive(Debug, Deserialize)]
pub struct ArtifactSpec {
    /// GitHub repository in `owner/repo` form.
    pub repo: String,
    /// Release tag to use (e.g. `"b8069"` or `"v1.8.3"`).
    pub version: String,
    /// Base name of the library (used for informational purposes).
    pub lib_name: String,
    /// Asset file name template.  Supports `{version}`, `{os}`, `{arch}`,
    /// `{variant}` placeholders.
    pub asset_pattern: String,
    /// Supported OS/arch combinations per variant.
    pub variants: HashMap<String, VariantMatrix>,
    /// Optional SHA256 checksums keyed by `"{os}-{variant}-{arch}"`.
    #[serde(default)]
    pub checksums: HashMap<String, String>,
}

/// Availability matrix for one variant (e.g. `cpu`, `cuda`).
#[derive(Debug, Deserialize)]
pub struct VariantMatrix {
    pub os: Vec<String>,
    pub arch: Vec<String>,
}

/// A fully resolved artifact ready to be downloaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedArtifact {
    pub repo: String,
    pub version: String,
    pub asset_name: String,
    /// Optional expected SHA256 checksum in `"sha256:<hex>"` format.
    pub checksum: Option<String>,
}

impl Manifest {
    /// Parse a manifest from a TOML file on disk.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, FetchError> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(|e| {
            FetchError::ManifestError(format!(
                "cannot read manifest {:?}: {}",
                path.as_ref(),
                e
            ))
        })?;
        Self::from_str(&content)
    }

    /// Parse a manifest from a TOML string.
    pub fn from_str(content: &str) -> Result<Self, FetchError> {
        let manifest: Self = toml::from_str(content)
            .map_err(|e| FetchError::ManifestError(format!("TOML parse error: {}", e)))?;

        if manifest.metadata.schema_version != "1" {
            return Err(FetchError::ManifestError(format!(
                "unsupported manifest schema_version '{}', expected '1'",
                manifest.metadata.schema_version
            )));
        }

        Ok(manifest)
    }

    /// Look up an artifact by name.
    pub fn artifact(&self, name: &str) -> Result<&ArtifactSpec, FetchError> {
        self.artifacts.get(name).ok_or_else(|| {
            FetchError::ManifestError(format!("artifact '{}' not found in manifest", name))
        })
    }
}

impl ArtifactSpec {
    /// Resolve this spec against a concrete `platform` and `variant`.
    ///
    /// Validates that the variant is supported on the platform, renders the
    /// `asset_pattern` template, and looks up any stored checksum.
    pub fn resolve(
        &self,
        platform: &Platform,
        variant: &Variant,
    ) -> Result<ResolvedArtifact, FetchError> {
        let variant_key = variant.to_string();
        let matrix = self.variants.get(&variant_key).ok_or_else(|| {
            FetchError::ManifestError(format!(
                "variant '{}' is not declared for artifact '{}'",
                variant_key, self.lib_name
            ))
        })?;

        let os_str = platform.os.to_string();
        let arch_str = platform.arch.to_string();

        if !matrix.os.contains(&os_str) {
            return Err(FetchError::ManifestError(format!(
                "variant '{}' is not available for OS '{}'",
                variant_key, os_str
            )));
        }
        if !matrix.arch.contains(&arch_str) {
            return Err(FetchError::ManifestError(format!(
                "variant '{}' is not available for arch '{}'",
                variant_key, arch_str
            )));
        }

        let asset_name = self
            .asset_pattern
            .replace("{version}", &self.version)
            .replace("{os}", &os_str)
            .replace("{arch}", &arch_str)
            .replace("{variant}", &variant_key);

        let checksum_key = format!("{}-{}-{}", os_str, variant_key, arch_str);
        let checksum = self.checksums.get(&checksum_key).cloned();

        Ok(ResolvedArtifact {
            repo: self.repo.clone(),
            version: self.version.clone(),
            asset_name,
            checksum,
        })
    }
}

/// Convenience: resolve using automatically detected platform and variant.
pub fn resolve_current(
    manifest: &Manifest,
    artifact_name: &str,
) -> Result<ResolvedArtifact, FetchError> {
    let platform = Platform::current().ok_or_else(|| {
        FetchError::ManifestError("unsupported OS or architecture".to_string())
    })?;
    let variant = Variant::detect_best(&platform);
    manifest.artifact(artifact_name)?.resolve(&platform, &variant)
}

/// Helpers for constructing `Os` / `Arch` from string slices (used in tests).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{Arch, Os};

    const SAMPLE_TOML: &str = r#"
[metadata]
schema_version = "1"

[artifacts.llama]
repo = "ggml-org/llama.cpp"
version = "b8069"
lib_name = "llama"
asset_pattern = "llama-{version}-bin-{os}-{variant}-{arch}.zip"

[artifacts.llama.variants]
cpu    = { os = ["windows", "linux", "macos"], arch = ["x86_64", "aarch64"] }
cuda   = { os = ["windows", "linux"],          arch = ["x86_64"] }
vulkan = { os = ["windows", "linux"],          arch = ["x86_64"] }
metal  = { os = ["macos"],                     arch = ["aarch64"] }

[artifacts.llama.checksums]
"linux-cpu-x86_64" = "sha256:deadbeef"
"#;

    fn make_platform(os: Os, arch: Arch) -> Platform {
        Platform { os, arch }
    }

    #[test]
    fn test_parse_manifest() {
        let m = Manifest::from_str(SAMPLE_TOML).unwrap();
        assert_eq!(m.metadata.schema_version, "1");
        assert!(m.artifacts.contains_key("llama"));
    }

    #[test]
    fn test_resolve_cpu_linux_x86_64() {
        let m = Manifest::from_str(SAMPLE_TOML).unwrap();
        let spec = m.artifact("llama").unwrap();
        let platform = make_platform(Os::Linux, Arch::X86_64);
        let resolved = spec.resolve(&platform, &Variant::Cpu).unwrap();
        assert_eq!(resolved.asset_name, "llama-b8069-bin-linux-cpu-x86_64.zip");
        assert_eq!(resolved.checksum, Some("sha256:deadbeef".to_string()));
        assert_eq!(resolved.repo, "ggml-org/llama.cpp");
        assert_eq!(resolved.version, "b8069");
    }

    #[test]
    fn test_resolve_metal_macos_aarch64() {
        let m = Manifest::from_str(SAMPLE_TOML).unwrap();
        let spec = m.artifact("llama").unwrap();
        let platform = make_platform(Os::MacOS, Arch::Aarch64);
        let resolved = spec.resolve(&platform, &Variant::Metal).unwrap();
        assert_eq!(resolved.asset_name, "llama-b8069-bin-macos-metal-aarch64.zip");
        assert_eq!(resolved.checksum, None);
    }

    #[test]
    fn test_resolve_invalid_variant_for_os() {
        let m = Manifest::from_str(SAMPLE_TOML).unwrap();
        let spec = m.artifact("llama").unwrap();
        let platform = make_platform(Os::Linux, Arch::X86_64);
        let err = spec.resolve(&platform, &Variant::Metal).unwrap_err();
        assert!(err.to_string().contains("metal"), "error should mention 'metal'");
    }

    #[test]
    fn test_resolve_invalid_arch_for_variant() {
        let m = Manifest::from_str(SAMPLE_TOML).unwrap();
        let spec = m.artifact("llama").unwrap();
        // CUDA is only available for x86_64
        let platform = make_platform(Os::Linux, Arch::Aarch64);
        let err = spec.resolve(&platform, &Variant::Cuda).unwrap_err();
        assert!(err.to_string().contains("arch"), "error should mention arch");
    }

    #[test]
    fn test_missing_artifact_name() {
        let m = Manifest::from_str(SAMPLE_TOML).unwrap();
        let err = m.artifact("nonexistent").unwrap_err();
        assert!(err.to_string().contains("nonexistent"));
    }

    #[test]
    fn test_parse_error_on_invalid_toml() {
        let err = Manifest::from_str("not valid toml ][").unwrap_err();
        assert!(err.to_string().contains("TOML"));
    }

    #[test]
    fn test_unsupported_schema_version_rejected() {
        let toml = r#"
[metadata]
schema_version = "2"

[artifacts.llama]
repo = "ggml-org/llama.cpp"
version = "b8069"
lib_name = "llama"
asset_pattern = "llama-{version}-bin-{os}-{variant}-{arch}.zip"

[artifacts.llama.variants]
cpu = { os = ["linux"], arch = ["x86_64"] }
"#;
        let err = Manifest::from_str(toml).unwrap_err();
        assert!(
            err.to_string().contains("schema_version"),
            "error should mention schema_version, got: {}",
            err
        );
    }

    #[test]
    fn test_unknown_fields_ignored() {
        // serde by default ignores unknown fields; ensure this works
        let toml_with_extra = r#"
[metadata]
schema_version = "1"
extra_field = "should be ignored"

[artifacts.llama]
repo = "ggml-org/llama.cpp"
version = "b8069"
lib_name = "llama"
asset_pattern = "llama-{version}-bin-{os}-{variant}-{arch}.zip"

[artifacts.llama.variants]
cpu = { os = ["linux"], arch = ["x86_64"] }
"#;
        let m = Manifest::from_str(toml_with_extra).unwrap();
        assert_eq!(m.metadata.schema_version, "1");
    }

    #[test]
    fn test_os_arch_from_str_helpers() {
        fn os_from_str(s: &str) -> Option<Os> {
            match s {
                "windows" => Some(Os::Windows),
                "linux" => Some(Os::Linux),
                "macos" => Some(Os::MacOS),
                _ => None,
            }
        }
        fn arch_from_str(s: &str) -> Option<Arch> {
            match s {
                "x86_64" => Some(Arch::X86_64),
                "aarch64" => Some(Arch::Aarch64),
                _ => None,
            }
        }

        assert_eq!(os_from_str("linux"), Some(Os::Linux));
        assert_eq!(os_from_str("macos"), Some(Os::MacOS));
        assert_eq!(os_from_str("windows"), Some(Os::Windows));
        assert_eq!(os_from_str("freebsd"), None);

        assert_eq!(arch_from_str("x86_64"), Some(Arch::X86_64));
        assert_eq!(arch_from_str("aarch64"), Some(Arch::Aarch64));
        assert_eq!(arch_from_str("mips"), None);
    }
}
