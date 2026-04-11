use crate::error::FetchError;
use crate::platform::Platform;
use crate::variant::Variant;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

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
    /// and optional `{variant}` placeholders.
    pub asset_pattern: String,
    /// Supported OS/arch/variant combinations organised by OS first.
    ///
    /// Example:
    /// `[artifacts.llama.targets.windows.x86_64] variants = ["cpu", "cuda"]`
    ///
    /// When `variants` is omitted or `[]`, the artifact is treated as
    /// variant-less for that OS/arch pair.
    #[serde(default, alias = "platforms", alias = "os")]
    pub targets: HashMap<String, HashMap<String, ArtifactTarget>>,
    /// Supported OS/arch combinations per variant.
    ///
    /// Kept for backward compatibility with the older manifest layout.
    #[serde(default)]
    pub variants: HashMap<String, VariantMatrix>,
    /// Optional SHA256 checksums keyed by `"{os}-{variant}-{arch}"` or
    /// `"{os}-{arch}"` for variant-less artifacts.
    #[serde(default)]
    pub checksums: HashMap<String, String>,
}

/// One concrete OS/arch target entry in the manifest.
#[derive(Debug, Deserialize)]
pub struct ArtifactTarget {
    #[serde(default)]
    pub variants: Vec<String>,
    /// Archive suffix used by `{extension}` in `asset_pattern`.
    #[serde(default)]
    pub extension: Option<String>,
    /// Target-scoped checksum for variant-less artifacts.
    ///
    /// `checksums` is accepted as an alias to match existing manifests.
    #[serde(default, alias = "checksums")]
    pub checksum: Option<String>,
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
            FetchError::ManifestError(format!("cannot read manifest {:?}: {}", path.as_ref(), e))
        })?;
        Self::parse(&content)
    }

    /// Parse a manifest from a TOML string.
    pub fn parse(content: &str) -> Result<Self, FetchError> {
        content.parse()
    }

    /// Look up an artifact by name.
    pub fn artifact(&self, name: &str) -> Result<&ArtifactSpec, FetchError> {
        self.artifacts.get(name).ok_or_else(|| {
            FetchError::ManifestError(format!("artifact '{}' not found in manifest", name))
        })
    }
}

impl FromStr for Manifest {
    type Err = FetchError;

    fn from_str(content: &str) -> Result<Self, Self::Err> {
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
        let os_str = platform.os.to_string();
        let requested_arch = platform.arch.to_string();
        let resolved_target =
            self.resolve_variant_for_target(&os_str, &requested_arch, &variant.to_string())?;
        let asset_name = render_asset_name(
            &self.asset_pattern,
            &self.version,
            &os_str,
            &resolved_target.arch,
            resolved_target.variant.as_deref(),
            resolved_target.extension,
        );
        let checksum = resolved_target.checksum.map(str::to_string).or_else(|| {
            self.lookup_checksum(
                &os_str,
                checksum_arch_candidates(&requested_arch, &resolved_target.arch),
                resolved_target.variant.as_deref(),
            )
        });

        Ok(ResolvedArtifact {
            repo: self.repo.clone(),
            version: self.version.clone(),
            asset_name,
            checksum,
        })
    }

    fn resolve_variant_for_target(
        &self,
        os_str: &str,
        arch_str: &str,
        variant_key: &str,
    ) -> Result<ResolvedTarget<'_>, FetchError> {
        if !self.targets.is_empty() {
            let os_targets = self.targets.get(os_str).ok_or_else(|| {
                FetchError::ManifestError(format!(
                    "OS '{}' is not declared for artifact '{}'",
                    os_str, self.lib_name
                ))
            })?;
            let (resolved_arch, target) = find_declared_target(os_targets, arch_str).ok_or_else(|| {
                FetchError::ManifestError(format!(
                    "arch '{}' is not declared for OS '{}' in artifact '{}'",
                    arch_str, os_str, self.lib_name
                ))
            })?;

            if target.variants.is_empty() {
                return Ok(ResolvedTarget {
                    arch: resolved_arch.to_string(),
                    variant: None,
                    extension: target.extension.as_deref(),
                    checksum: target.checksum.as_deref(),
                });
            }

            if target.variants.iter().any(|declared| declared == variant_key) {
                return Ok(ResolvedTarget {
                    arch: resolved_arch.to_string(),
                    variant: Some(variant_key.to_string()),
                    extension: target.extension.as_deref(),
                    checksum: target.checksum.as_deref(),
                });
            }

            return Err(FetchError::ManifestError(format!(
                "variant '{}' is not available for platform '{}-{}'",
                variant_key, os_str, arch_str
            )));
        }

        let matrix = self.variants.get(variant_key).ok_or_else(|| {
            FetchError::ManifestError(format!(
                "variant '{}' is not declared for artifact '{}'",
                variant_key, self.lib_name
            ))
        })?;

        if !matrix.os.iter().any(|os| os == os_str) {
            return Err(FetchError::ManifestError(format!(
                "variant '{}' is not available for OS '{}'",
                variant_key, os_str
            )));
        }
        let resolved_arch = find_declared_arch(&matrix.arch, arch_str).ok_or_else(|| {
            FetchError::ManifestError(format!(
                "variant '{}' is not available for arch '{}'",
                variant_key, arch_str
            ))
        })?;

        Ok(ResolvedTarget {
            arch: resolved_arch.to_string(),
            variant: Some(variant_key.to_string()),
            extension: None,
            checksum: None,
        })
    }

    fn lookup_checksum(
        &self,
        os_str: &str,
        arch_candidates: Vec<&str>,
        variant: Option<&str>,
    ) -> Option<String> {
        let mut keys = Vec::new();
        if let Some(variant) = variant {
            for arch in &arch_candidates {
                keys.push(format!("{}-{}-{}", os_str, variant, arch));
            }
        }
        for arch in arch_candidates {
            keys.push(format!("{}-{}", os_str, arch));
        }

        keys.into_iter().find_map(|key| self.checksums.get(&key).cloned())
    }
}

struct ResolvedTarget<'a> {
    arch: String,
    variant: Option<String>,
    extension: Option<&'a str>,
    checksum: Option<&'a str>,
}

fn checksum_arch_candidates<'a>(requested_arch: &'a str, resolved_arch: &'a str) -> Vec<&'a str> {
    let mut candidates = Vec::new();
    push_unique_arch_candidates(&mut candidates, resolved_arch);
    push_unique_arch_candidates(&mut candidates, requested_arch);
    candidates
}

fn push_unique_arch_candidates<'a>(out: &mut Vec<&'a str>, arch: &'a str) {
    for candidate in arch_aliases(arch) {
        if !out.contains(&candidate) {
            out.push(candidate);
        }
    }
}

fn arch_aliases(arch: &str) -> Vec<&str> {
    match arch {
        "aarch64" => vec!["aarch64", "arm64"],
        "arm64" => vec!["arm64", "aarch64"],
        _ => vec![arch],
    }
}

fn find_declared_target<'a>(
    os_targets: &'a HashMap<String, ArtifactTarget>,
    arch_str: &str,
) -> Option<(&'a str, &'a ArtifactTarget)> {
    for candidate in arch_aliases(arch_str) {
        if let Some((declared_arch, target)) = os_targets.get_key_value(candidate) {
            return Some((declared_arch.as_str(), target));
        }
    }

    None
}

fn find_declared_arch<'a>(declared_arches: &'a [String], arch_str: &str) -> Option<&'a str> {
    for candidate in arch_aliases(arch_str) {
        if let Some(declared_arch) = declared_arches
            .iter()
            .find(|declared_arch| declared_arch.as_str() == candidate)
        {
            return Some(declared_arch.as_str());
        }
    }

    None
}

fn render_asset_name(
    asset_pattern: &str,
    version: &str,
    os_str: &str,
    arch_str: &str,
    variant: Option<&str>,
    extension: Option<&str>,
) -> String {
    asset_pattern
        .replace("{version}", version)
        .replace("{os}", os_str)
        .replace("{arch}", arch_str)
        .replace("{variant}", variant.unwrap_or(""))
        .replace("{extension}", extension.unwrap_or(""))
}

/// Convenience: resolve using automatically detected platform and variant.
pub fn resolve_current(
    manifest: &Manifest,
    artifact_name: &str,
) -> Result<ResolvedArtifact, FetchError> {
    let platform = Platform::current()
        .ok_or_else(|| FetchError::ManifestError("unsupported OS or architecture".to_string()))?;
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
asset_pattern = "llama-{version}-bin-{os}-{variant}-{arch}.{extension}"

[artifacts.llama.targets.windows.x86_64]
variants = ["cpu", "cuda", "vulkan"]
extension = "zip"

[artifacts.llama.targets.windows.aarch64]
variants = ["cpu"]
extension = "zip"

[artifacts.llama.targets.linux.x86_64]
variants = ["cpu", "cuda", "vulkan"]
extension = "tar.gz"

[artifacts.llama.targets.linux.aarch64]
variants = ["cpu"]
extension = "tar.gz"

[artifacts.llama.targets.macos.x86_64]
variants = ["cpu"]
extension = "tar.gz"

[artifacts.llama.targets.macos.aarch64]
variants = ["cpu", "metal"]
extension = "tar.gz"
checksum = "sha256:feedface"
"#;

    const SAMPLE_TOML_NO_VARIANTS: &str = r#"
[metadata]
schema_version = "1"

[artifacts.llama]
repo = "ggml-org/llama.cpp"
version = "b8069"
lib_name = "llama"
asset_pattern = "llama-{version}-bin-{os}-{arch}.{extension}"

[artifacts.llama.targets.linux.x86_64]
extension = "tar.gz"
checksums = "sha256:cafebabe"

[artifacts.llama.targets.macos.aarch64]
variants = []
extension = "tar.gz"
"#;

    const SAMPLE_TOML_TARGET_CHECKSUM_FALLBACK: &str = r#"
[metadata]
schema_version = "1"

[artifacts.llama]
repo = "ggml-org/llama.cpp"
version = "b8069"
lib_name = "llama"
asset_pattern = "llama-{version}-bin-{os}-{variant}-{arch}.{extension}"

[artifacts.llama.targets.linux.x86_64]
variants = ["cpu"]
extension = "tar.gz"

[artifacts.llama.checksums]
"linux-cpu-x86_64" = "sha256:deadbeef"
"#;

    const SAMPLE_TOML_LEGACY: &str = r#"
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
"#;

    const SAMPLE_TOML_TARGET_ARCH_ALIAS: &str = r#"
[metadata]
schema_version = "1"

[artifacts.whisper]
repo = "ggml-org/whisper.cpp"
version = "v1.8.4"
lib_name = "whisper"
asset_pattern = "whisper-sdk-{os}-{arch}.{extension}"

[artifacts.whisper.targets.macos.arm64]
variants = []
extension = "tar.gz"

[artifacts.whisper.checksums]
"macos-arm64" = "sha256:decafbad"
"#;

    const SAMPLE_TOML_LEGACY_ARCH_ALIAS: &str = r#"
[metadata]
schema_version = "1"

[artifacts.llama]
repo = "ggml-org/llama.cpp"
version = "b8069"
lib_name = "llama"
asset_pattern = "llama-{version}-bin-{os}-{variant}-{arch}.zip"

[artifacts.llama.variants]
metal = { os = ["macos"], arch = ["arm64"] }
"#;

    fn make_platform(os: Os, arch: Arch) -> Platform {
        Platform { os, arch }
    }

    #[test]
    fn test_parse_manifest() {
        let m = Manifest::parse(SAMPLE_TOML).unwrap();
        assert_eq!(m.metadata.schema_version, "1");
        assert!(m.artifacts.contains_key("llama"));
    }

    #[test]
    fn test_parse_legacy_manifest() {
        let m = Manifest::parse(SAMPLE_TOML_LEGACY).unwrap();
        assert_eq!(m.metadata.schema_version, "1");
        assert!(m.artifacts.contains_key("llama"));
    }

    #[test]
    fn test_resolve_cpu_linux_x86_64() {
        let m = Manifest::parse(SAMPLE_TOML).unwrap();
        let spec = m.artifact("llama").unwrap();
        let platform = make_platform(Os::Linux, Arch::X86_64);
        let resolved = spec.resolve(&platform, &Variant::Cpu).unwrap();
        assert_eq!(resolved.asset_name, "llama-b8069-bin-linux-cpu-x86_64.tar.gz");
        assert_eq!(resolved.checksum, None);
        assert_eq!(resolved.repo, "ggml-org/llama.cpp");
        assert_eq!(resolved.version, "b8069");
    }

    #[test]
    fn test_resolve_metal_macos_aarch64() {
        let m = Manifest::parse(SAMPLE_TOML).unwrap();
        let spec = m.artifact("llama").unwrap();
        let platform = make_platform(Os::MacOS, Arch::Aarch64);
        let resolved = spec.resolve(&platform, &Variant::Metal).unwrap();
        assert_eq!(resolved.asset_name, "llama-b8069-bin-macos-metal-aarch64.tar.gz");
        assert_eq!(resolved.checksum, Some("sha256:feedface".to_string()));
    }

    #[test]
    fn test_resolve_invalid_variant_for_os() {
        let m = Manifest::parse(SAMPLE_TOML).unwrap();
        let spec = m.artifact("llama").unwrap();
        let platform = make_platform(Os::Linux, Arch::X86_64);
        let err = spec.resolve(&platform, &Variant::Metal).unwrap_err();
        assert!(err.to_string().contains("metal"), "error should mention 'metal'");
    }

    #[test]
    fn test_resolve_invalid_arch_for_variant() {
        let m = Manifest::parse(SAMPLE_TOML_LEGACY).unwrap();
        let spec = m.artifact("llama").unwrap();
        // CUDA is only available for x86_64
        let platform = make_platform(Os::Linux, Arch::Aarch64);
        let err = spec.resolve(&platform, &Variant::Cuda).unwrap_err();
        assert!(err.to_string().contains("arch"), "error should mention arch");
    }

    #[test]
    fn test_missing_artifact_name() {
        let m = Manifest::parse(SAMPLE_TOML).unwrap();
        let err = m.artifact("nonexistent").unwrap_err();
        assert!(err.to_string().contains("nonexistent"));
    }

    #[test]
    fn test_parse_error_on_invalid_toml() {
        let err = Manifest::parse("not valid toml ][").unwrap_err();
        assert!(err.to_string().contains("TOML"));
    }

    #[test]
    fn test_resolve_variantless_asset_name_and_checksum() {
        let m = Manifest::parse(SAMPLE_TOML_NO_VARIANTS).unwrap();
        let spec = m.artifact("llama").unwrap();
        let platform = make_platform(Os::Linux, Arch::X86_64);
        let resolved = spec.resolve(&platform, &Variant::Cuda).unwrap();
        assert_eq!(resolved.asset_name, "llama-b8069-bin-linux-x86_64.tar.gz");
        assert_eq!(resolved.checksum, Some("sha256:cafebabe".to_string()));
    }

    #[test]
    fn test_resolve_target_uses_legacy_top_level_checksum_fallback() {
        let m = Manifest::parse(SAMPLE_TOML_TARGET_CHECKSUM_FALLBACK).unwrap();
        let spec = m.artifact("llama").unwrap();
        let platform = make_platform(Os::Linux, Arch::X86_64);
        let resolved = spec.resolve(&platform, &Variant::Cpu).unwrap();
        assert_eq!(resolved.asset_name, "llama-b8069-bin-linux-cpu-x86_64.tar.gz");
        assert_eq!(resolved.checksum, Some("sha256:deadbeef".to_string()));
    }

    #[test]
    fn test_resolve_target_arch_alias_uses_manifest_arch_name() {
        let m = Manifest::parse(SAMPLE_TOML_TARGET_ARCH_ALIAS).unwrap();
        let spec = m.artifact("whisper").unwrap();
        let platform = make_platform(Os::MacOS, Arch::Aarch64);
        let resolved = spec.resolve(&platform, &Variant::Metal).unwrap();
        assert_eq!(resolved.asset_name, "whisper-sdk-macos-arm64.tar.gz");
        assert_eq!(resolved.checksum, Some("sha256:decafbad".to_string()));
    }

    #[test]
    fn test_resolve_legacy_arch_alias_uses_manifest_arch_name() {
        let m = Manifest::parse(SAMPLE_TOML_LEGACY_ARCH_ALIAS).unwrap();
        let spec = m.artifact("llama").unwrap();
        let platform = make_platform(Os::MacOS, Arch::Aarch64);
        let resolved = spec.resolve(&platform, &Variant::Metal).unwrap();
        assert_eq!(resolved.asset_name, "llama-b8069-bin-macos-metal-arm64.zip");
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

[artifacts.llama.targets.linux.x86_64]
variants = ["cpu"]
"#;
        let err = Manifest::parse(toml).unwrap_err();
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

[artifacts.llama.targets.linux.x86_64]
variants = ["cpu"]
"#;
        let m = Manifest::parse(toml_with_extra).unwrap();
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
