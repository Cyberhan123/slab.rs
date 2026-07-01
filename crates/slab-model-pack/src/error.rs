use std::io;
use std::string::FromUtf8Error;

use thiserror::Error;
use zip::result::ZipError;

#[derive(Debug, Error)]
pub enum ModelPackError {
    #[error("model pack path '{path}' must end with .slab")]
    InvalidPackExtension { path: String },

    #[error("failed to read model pack '{path}': {source}")]
    ReadPack {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("failed to open .slab archive: {source}")]
    OpenArchive {
        #[source]
        source: ZipError,
    },

    #[error("failed to access archive entry #{index}: {source}")]
    AccessArchiveEntry {
        index: usize,
        #[source]
        source: ZipError,
    },

    #[error("failed to read archive entry '{path}': {source}")]
    ReadArchiveEntry {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("invalid archive path '{path}'")]
    InvalidArchivePath { path: String },

    #[error("missing required manifest.json in .slab archive")]
    MissingManifest,

    #[error("invalid JSON document '{path}': {source}")]
    InvalidJsonDocument {
        path: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("unsupported model pack schema_version {found}; only schema_version 3 is supported")]
    UnsupportedSchemaVersion { found: u32 },

    #[error("invalid UTF-8 text asset '{path}': {source}")]
    InvalidTextAsset {
        path: String,
        #[source]
        source: FromUtf8Error,
    },

    #[error("duplicate JSON document path '{path}' in .slab archive")]
    DuplicateDocumentPath { path: String },

    #[error("duplicate {kind} id '{id}'")]
    DuplicateId { kind: &'static str, id: String },

    #[error("local model pack '{id}' must declare at least one engine")]
    MissingLocalEngines { id: String },

    #[error("local model pack '{id}' must declare at least one variant")]
    MissingLocalVariants { id: String },

    #[error("local model pack '{id}' must declare at least one preset")]
    MissingLocalPresets { id: String },

    #[error("invalid config reference '{value}': {reason}")]
    InvalidConfigRef { value: String, reason: String },

    #[error("document '{path}' referenced from '{from}' was not found")]
    MissingReferencedDocument { from: String, path: String },

    #[error("manifest default preset '{id}' was not found")]
    MissingDefaultPreset { id: String },

    #[error("manifest must declare default_preset when multiple presets exist")]
    MissingDefaultPresetDeclaration,

    #[error(
        "manifest must declare at least one runtime execution capability to build a runtime bridge"
    )]
    MissingRuntimeCapability,

    #[error("model pack could not determine a runtime backend for preset '{preset_id}'")]
    MissingRuntimeBackend { preset_id: String },

    #[error("model pack has conflicting runtime backends for preset '{preset_id}'")]
    ConflictingRuntimeBackend { preset_id: String },

    #[error("backend config '{id}' payload must be a JSON object")]
    InvalidBackendConfigPayloadShape { id: String },

    #[error("variant '{variant_id}' format '{format}' is not supported by any declared engine")]
    IncompatibleVariantFormat { variant_id: String, format: String },

    #[error("preset '{preset_id}' has no compatible engine candidates for variant '{variant_id}'")]
    MissingCompatibleEngines { preset_id: String, variant_id: String },

    #[error("backend config '{id}' field '{field}' is not supported: {message}")]
    UnsupportedBackendConfigField { id: String, field: String, message: String },

    #[error("backend config '{id}' field '{field}' has an invalid asset ref: {message}")]
    InvalidBackendConfigAssetRef { id: String, field: String, message: String },

    #[error("backend config '{id}' field '{field}' references missing asset '{path}'")]
    MissingBackendConfigAsset { id: String, field: String, path: String },

    #[error(
        "resolved preset '{preset_id}' could not build a typed runtime load command: {message}"
    )]
    InvalidRuntimeLoadCommand { preset_id: String, message: String },

    #[error("resolved preset '{preset_id}' does not expose a loadable primary model artifact")]
    MissingPrimaryArtifact { preset_id: String },

    #[error(
        "resolved preset '{preset_id}' uses non-materialized source kind '{source_kind}' and cannot build a runtime load spec yet"
    )]
    NonMaterializedSource { preset_id: String, source_kind: String },

    #[error("document '{path}' has kind '{found}' but '{expected}' was required")]
    UnexpectedDocumentKind { path: String, expected: &'static str, found: &'static str },

    #[error("backend config '{path}' has scope '{found}' but '{expected}' was required")]
    UnexpectedBackendConfigScope { path: String, expected: &'static str, found: &'static str },

    #[error("document '{path}' declares id '{found}' but manifest reference expects '{expected}'")]
    DocumentIdMismatch { path: String, expected: String, found: String },

    #[error("manifest references unknown {kind} id '{id}'")]
    MissingNamedDocument { kind: &'static str, id: String },
}
