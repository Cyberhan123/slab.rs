use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use serde_json::Value;
use slab_types::{ArtifactFormat, Capability, ModelFamily};
use zip::ZipArchive;

use crate::error::ModelPackError;
use crate::manifest::{
    BackendConfigDocument, BackendConfigScope, ComponentDocument, ConfigEntryRef,
    MODEL_PACK_SCHEMA_VERSION, ModelPackManifest, PackDeployment, PackDocument, PresetDocument,
    PresetEntryRef, VariantDocument,
};
use crate::refs::ConfigRef;

pub const PACK_EXTENSION: &str = "slab";
pub const MANIFEST_FILE_NAME: &str = "manifest.json";

#[derive(Debug, Clone)]
pub struct ModelPack {
    manifest: ModelPackManifest,
    documents: BTreeMap<String, PackDocument>,
    text_assets: BTreeMap<String, String>,
}

impl ModelPack {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ModelPackError> {
        let path = path.as_ref();
        let extension = path.extension().and_then(|value| value.to_str()).unwrap_or_default();
        if !extension.eq_ignore_ascii_case(PACK_EXTENSION) {
            return Err(ModelPackError::InvalidPackExtension { path: path.display().to_string() });
        }

        let bytes = fs::read(path).map_err(|source| ModelPackError::ReadPack {
            path: path.display().to_string(),
            source,
        })?;

        Self::from_bytes(&bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ModelPackError> {
        let cursor = Cursor::new(bytes);
        let mut archive =
            ZipArchive::new(cursor).map_err(|source| ModelPackError::OpenArchive { source })?;

        let mut manifest: Option<ModelPackManifest> = None;
        let mut documents = BTreeMap::new();
        let mut text_assets = BTreeMap::new();

        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|source| ModelPackError::AccessArchiveEntry { index, source })?;

            if entry.is_dir() {
                continue;
            }

            let path = normalize_archive_path(entry.name())?;
            let mut raw = Vec::new();
            entry.read_to_end(&mut raw).map_err(|source| ModelPackError::ReadArchiveEntry {
                path: path.clone(),
                source,
            })?;
            let raw = String::from_utf8(raw).map_err(|source| {
                ModelPackError::InvalidTextAsset { path: path.clone(), source }
            })?;

            if path.ends_with(".json") {
                if path == MANIFEST_FILE_NAME {
                    if manifest.is_some() {
                        return Err(ModelPackError::DuplicateDocumentPath { path });
                    }

                    manifest = Some(parse_manifest_document(&path, &raw)?);
                    continue;
                }

                let document: PackDocument = parse_json_document(&path, &raw)?;
                if documents.insert(path.clone(), document).is_some() {
                    return Err(ModelPackError::DuplicateDocumentPath { path });
                }
                continue;
            }

            if text_assets.insert(path.clone(), raw).is_some() {
                return Err(ModelPackError::DuplicateDocumentPath { path });
            }
        }

        let mut manifest = manifest.ok_or(ModelPackError::MissingManifest)?;
        manifest.capabilities = normalized_manifest_capabilities(&manifest);

        let pack = Self { manifest, documents, text_assets };

        pack.validate_manifest_shape()?;
        pack.validate_unique_ids()?;
        pack.validate_manifest_references()?;
        pack.validate_backend_config_payloads()?;
        Ok(pack)
    }

    pub fn manifest(&self) -> &ModelPackManifest {
        &self.manifest
    }

    pub fn documents(&self) -> &BTreeMap<String, PackDocument> {
        &self.documents
    }

    pub fn text_assets(&self) -> &BTreeMap<String, String> {
        &self.text_assets
    }

    pub fn document(&self, config_ref: &ConfigRef) -> Result<&PackDocument, ModelPackError> {
        self.documents.get(config_ref.path()).ok_or_else(|| {
            ModelPackError::MissingReferencedDocument {
                from: MANIFEST_FILE_NAME.into(),
                path: config_ref.path().into(),
            }
        })
    }

    pub fn text_asset(&self, config_ref: &ConfigRef) -> Result<&str, ModelPackError> {
        self.text_assets.get(config_ref.path()).map(String::as_str).ok_or_else(|| {
            ModelPackError::MissingReferencedDocument {
                from: MANIFEST_FILE_NAME.into(),
                path: config_ref.path().into(),
            }
        })
    }

    pub fn resolve_variant(
        &self,
        config_ref: &ConfigRef,
    ) -> Result<&VariantDocument, ModelPackError> {
        match self.document(config_ref)? {
            PackDocument::Variant(document) => Ok(document),
            other => Err(ModelPackError::UnexpectedDocumentKind {
                path: config_ref.path().into(),
                expected: "variant",
                found: other.kind(),
            }),
        }
    }

    pub fn resolve_component(
        &self,
        config_ref: &ConfigRef,
    ) -> Result<&ComponentDocument, ModelPackError> {
        match self.document(config_ref)? {
            PackDocument::Component(document) => Ok(document),
            other => Err(ModelPackError::UnexpectedDocumentKind {
                path: config_ref.path().into(),
                expected: "component",
                found: other.kind(),
            }),
        }
    }

    pub fn resolve_preset(
        &self,
        config_ref: &ConfigRef,
    ) -> Result<&PresetDocument, ModelPackError> {
        match self.document(config_ref)? {
            PackDocument::Preset(document) => Ok(document),
            other => Err(ModelPackError::UnexpectedDocumentKind {
                path: config_ref.path().into(),
                expected: "preset",
                found: other.kind(),
            }),
        }
    }

    pub fn resolve_backend_config(
        &self,
        config_ref: &ConfigRef,
        expected_scope: BackendConfigScope,
    ) -> Result<&BackendConfigDocument, ModelPackError> {
        match self.document(config_ref)? {
            PackDocument::BackendConfig(document) => {
                if document.scope != expected_scope {
                    return Err(ModelPackError::UnexpectedBackendConfigScope {
                        path: config_ref.path().into(),
                        expected: expected_scope.as_str(),
                        found: document.scope.as_str(),
                    });
                }
                Ok(document)
            }
            other => Err(ModelPackError::UnexpectedDocumentKind {
                path: config_ref.path().into(),
                expected: "backend_config",
                found: other.kind(),
            }),
        }
    }

    fn validate_manifest_references(&self) -> Result<(), ModelPackError> {
        if self.manifest.deployment == PackDeployment::Cloud {
            return Ok(());
        }

        for reference in &self.manifest.components {
            self.validate_entry_ref(&reference.id, &reference.config_ref, "component")?;
        }
        for reference in &self.manifest.variants {
            self.validate_entry_ref(&reference.id, &reference.config_ref, "variant")?;
        }
        for reference in &self.manifest.adapters {
            self.validate_entry_ref(&reference.id, &reference.config_ref, "adapter")?;
        }
        for reference in &self.manifest.presets {
            self.validate_entry_ref(&reference.id, &reference.config_ref, "preset")?;
        }

        let component_ids = self.collect_ids("component");
        let variant_ids = self.collect_ids("variant");
        let adapter_ids = self.collect_ids("adapter");

        for document in self.documents.values() {
            match document {
                PackDocument::Variant(variant) => {
                    self.validate_component_ids(&variant.component_ids, &component_ids)?;
                    if !self.manifest.engines.iter().any(|engine| engine.format == variant.format) {
                        return Err(ModelPackError::IncompatibleVariantFormat {
                            variant_id: variant.id.clone(),
                            format: artifact_format_name(variant.format).to_owned(),
                        });
                    }
                    if let Some(config_ref) = &variant.load_config {
                        self.resolve_backend_config(config_ref, BackendConfigScope::Load)?;
                    }
                }
                PackDocument::Adapter(adapter) => {
                    self.validate_component_ids(&adapter.component_ids, &component_ids)?;
                }
                PackDocument::Preset(preset) => {
                    if !variant_ids.contains(&preset.variant_id) {
                        return Err(ModelPackError::MissingNamedDocument {
                            kind: "variant",
                            id: preset.variant_id.clone(),
                        });
                    }
                    for adapter_id in &preset.adapter_ids {
                        if !adapter_ids.contains(adapter_id) {
                            return Err(ModelPackError::MissingNamedDocument {
                                kind: "adapter",
                                id: adapter_id.clone(),
                            });
                        }
                    }
                    if let Some(config_ref) = &preset.inference_config {
                        self.resolve_backend_config(config_ref, BackendConfigScope::Inference)?;
                    }
                }
                PackDocument::Component(_) | PackDocument::BackendConfig(_) => {}
            }
        }

        Ok(())
    }

    fn validate_manifest_shape(&self) -> Result<(), ModelPackError> {
        if self.manifest.schema_version != MODEL_PACK_SCHEMA_VERSION {
            return Err(ModelPackError::UnsupportedSchemaVersion {
                found: self.manifest.schema_version,
            });
        }

        match self.manifest.deployment {
            PackDeployment::Local => {
                if self.manifest.cloud.is_some() {
                    return Err(ModelPackError::UnexpectedCloudTarget {
                        id: self.manifest.id.clone(),
                    });
                }
                if self.manifest.engines.is_empty() {
                    return Err(ModelPackError::MissingLocalEngines {
                        id: self.manifest.id.clone(),
                    });
                }
                if self.manifest.variants.is_empty() {
                    return Err(ModelPackError::MissingLocalVariants {
                        id: self.manifest.id.clone(),
                    });
                }
                if self.manifest.presets.is_empty() {
                    return Err(ModelPackError::MissingLocalPresets {
                        id: self.manifest.id.clone(),
                    });
                }
            }
            PackDeployment::Cloud => {
                if self.manifest.cloud.is_none() {
                    return Err(ModelPackError::MissingCloudTarget {
                        id: self.manifest.id.clone(),
                    });
                }
                if !self.manifest.engines.is_empty()
                    || !self.manifest.sources.is_empty()
                    || !self.manifest.components.is_empty()
                    || !self.manifest.variants.is_empty()
                    || !self.manifest.adapters.is_empty()
                    || !self.manifest.presets.is_empty()
                    || self.manifest.default_preset.is_some()
                {
                    return Err(ModelPackError::UnexpectedLocalRuntimeFields {
                        id: self.manifest.id.clone(),
                    });
                }
            }
        }

        Ok(())
    }

    fn validate_unique_ids(&self) -> Result<(), ModelPackError> {
        validate_unique_config_entries("component", &self.manifest.components)?;
        validate_unique_config_entries("variant", &self.manifest.variants)?;
        validate_unique_config_entries("adapter", &self.manifest.adapters)?;
        validate_unique_preset_entries(&self.manifest.presets)?;
        validate_unique_engine_targets(&self.manifest.engines)?;

        let mut ids_by_kind: BTreeMap<&'static str, BTreeSet<String>> = BTreeMap::new();
        for document in self.documents.values() {
            let Some(id) = document.declared_id().filter(|id| !id.trim().is_empty()) else {
                continue;
            };
            let kind = document.kind();
            if !ids_by_kind.entry(kind).or_default().insert(id.to_owned()) {
                return Err(ModelPackError::DuplicateId { kind, id: id.to_owned() });
            }
        }

        Ok(())
    }

    fn validate_backend_config_payloads(&self) -> Result<(), ModelPackError> {
        for (path, document) in &self.documents {
            let PackDocument::BackendConfig(config) = document else {
                continue;
            };
            let config_id = backend_config_id(path, config);
            let Value::Object(payload) = &config.payload else {
                return Err(ModelPackError::InvalidBackendConfigPayloadShape { id: config_id });
            };
            for field in ["chat_template", "gbnf"] {
                validate_optional_asset_ref(
                    field,
                    &config_id,
                    payload.get(field),
                    &self.text_assets,
                )?;
            }
        }

        Ok(())
    }

    fn validate_entry_ref(
        &self,
        id: &str,
        config_ref: &ConfigRef,
        expected_kind: &'static str,
    ) -> Result<(), ModelPackError> {
        let document =
            self.document(config_ref).map_err(|_| ModelPackError::MissingReferencedDocument {
                from: id.to_owned(),
                path: config_ref.path().into(),
            })?;

        if document.kind() != expected_kind {
            return Err(ModelPackError::UnexpectedDocumentKind {
                path: config_ref.path().into(),
                expected: expected_kind,
                found: document.kind(),
            });
        }

        if document.declared_id().is_some_and(|document_id| document_id != id) {
            return Err(ModelPackError::DocumentIdMismatch {
                path: config_ref.path().into(),
                expected: id.to_owned(),
                found: document.declared_id().unwrap_or_default().to_owned(),
            });
        }

        Ok(())
    }

    fn collect_ids(&self, expected_kind: &'static str) -> BTreeSet<String> {
        self.documents
            .values()
            .filter(|document| document.kind() == expected_kind)
            .map(|document| document.id().to_owned())
            .collect()
    }

    fn validate_component_ids(
        &self,
        component_ids: &[String],
        known_components: &BTreeSet<String>,
    ) -> Result<(), ModelPackError> {
        for component_id in component_ids {
            if !known_components.contains(component_id) {
                return Err(ModelPackError::MissingNamedDocument {
                    kind: "component",
                    id: component_id.clone(),
                });
            }
        }

        Ok(())
    }
}

fn normalize_archive_path(path: &str) -> Result<String, ModelPackError> {
    let trimmed = path.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || trimmed.contains('\\')
        || trimmed.split('/').any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(ModelPackError::InvalidArchivePath { path: path.to_owned() });
    }

    Ok(trimmed.to_owned())
}

fn parse_json_document<T: serde::de::DeserializeOwned>(
    path: &str,
    raw: &str,
) -> Result<T, ModelPackError> {
    serde_json::from_str(raw)
        .map_err(|source| ModelPackError::InvalidJsonDocument { path: path.to_owned(), source })
}

fn parse_manifest_document(path: &str, raw: &str) -> Result<ModelPackManifest, ModelPackError> {
    let value: Value = parse_json_document(path, raw)?;
    let schema_version = value
        .get("schema_version")
        .and_then(Value::as_u64)
        .or_else(|| value.get("version").and_then(Value::as_u64))
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0);
    if schema_version != MODEL_PACK_SCHEMA_VERSION {
        return Err(ModelPackError::UnsupportedSchemaVersion { found: schema_version });
    }

    serde_json::from_value(value)
        .map_err(|source| ModelPackError::InvalidJsonDocument { path: path.to_owned(), source })
}

fn validate_unique_config_entries(
    kind: &'static str,
    entries: &[ConfigEntryRef],
) -> Result<(), ModelPackError> {
    let mut ids = BTreeSet::new();
    for entry in entries {
        if !ids.insert(entry.id.clone()) {
            return Err(ModelPackError::DuplicateId { kind, id: entry.id.clone() });
        }
    }
    Ok(())
}

fn validate_unique_preset_entries(entries: &[PresetEntryRef]) -> Result<(), ModelPackError> {
    let mut ids = BTreeSet::new();
    for entry in entries {
        if !ids.insert(entry.id.clone()) {
            return Err(ModelPackError::DuplicateId { kind: "preset", id: entry.id.clone() });
        }
    }
    Ok(())
}

fn validate_unique_engine_targets(
    entries: &[crate::manifest::EngineTarget],
) -> Result<(), ModelPackError> {
    let mut ids = BTreeSet::new();
    for entry in entries {
        let id = entry.id.canonical_id().to_owned();
        if !ids.insert((id.clone(), artifact_format_name(entry.format).to_owned())) {
            return Err(ModelPackError::DuplicateId { kind: "engine", id });
        }
    }
    Ok(())
}

fn validate_optional_asset_ref(
    field: &str,
    config_id: &str,
    value: Option<&Value>,
    text_assets: &BTreeMap<String, String>,
) -> Result<(), ModelPackError> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.is_null() {
        return Ok(());
    }

    let asset_ref: slab_types::AssetRef =
        serde_json::from_value(value.clone()).map_err(|error| {
            ModelPackError::InvalidBackendConfigAssetRef {
                id: config_id.to_owned(),
                field: field.to_owned(),
                message: error.to_string(),
            }
        })?;
    let Some(asset_ref) = asset_ref.validate_configured(field).map_err(|error| {
        ModelPackError::InvalidBackendConfigAssetRef {
            id: config_id.to_owned(),
            field: field.to_owned(),
            message: error.to_string(),
        }
    })?
    else {
        return Ok(());
    };
    let path = asset_ref.path.as_deref().expect("validated asset ref has path");
    let config_ref = ConfigRef::parse(path.to_owned()).map_err(|error| {
        ModelPackError::InvalidBackendConfigAssetRef {
            id: config_id.to_owned(),
            field: field.to_owned(),
            message: error.to_string(),
        }
    })?;
    if !text_assets.contains_key(config_ref.path()) {
        return Err(ModelPackError::MissingBackendConfigAsset {
            id: config_id.to_owned(),
            field: field.to_owned(),
            path: config_ref.path().to_owned(),
        });
    }

    Ok(())
}

fn backend_config_id(path: &str, config: &BackendConfigDocument) -> String {
    config.id.clone().unwrap_or_else(|| path.to_owned())
}

fn artifact_format_name(format: ArtifactFormat) -> &'static str {
    match format {
        ArtifactFormat::Gguf => "gguf",
        ArtifactFormat::Ggml => "ggml",
        ArtifactFormat::Safetensors => "safetensors",
        ArtifactFormat::Onnx => "onnx",
        ArtifactFormat::Ckpt => "ckpt",
    }
}

fn normalized_manifest_capabilities(manifest: &ModelPackManifest) -> Vec<Capability> {
    let mut capabilities = if manifest.capabilities.is_empty() {
        default_manifest_capabilities(manifest.family)
    } else {
        manifest.capabilities.clone()
    };

    if capabilities.contains(&Capability::ChatGeneration)
        && !capabilities.contains(&Capability::TextGeneration)
    {
        capabilities.insert(0, Capability::TextGeneration);
    }

    let mut deduped = Vec::with_capacity(capabilities.len());
    for capability in capabilities {
        if !deduped.contains(&capability) {
            deduped.push(capability);
        }
    }

    deduped
}

fn default_manifest_capabilities(family: ModelFamily) -> Vec<Capability> {
    match family {
        ModelFamily::Whisper => vec![Capability::AudioTranscription],
        ModelFamily::Diffusion => vec![Capability::ImageGeneration, Capability::VideoGeneration],
        ModelFamily::Llama | ModelFamily::Onnx => {
            vec![Capability::TextGeneration, Capability::ChatGeneration]
        }
        _ => vec![Capability::TextGeneration, Capability::ChatGeneration],
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use serde_json::json;
    use slab_types::Capability;
    use zip::CompressionMethod;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    use super::{MANIFEST_FILE_NAME, ModelPack};

    #[test]
    fn loads_v3_manifest_and_referenced_documents_from_slab_bytes() {
        let bytes = build_pack(valid_pack_entries());

        let pack = ModelPack::from_bytes(&bytes).expect("pack should load");

        assert_eq!(pack.manifest().id, "qwen2.5-7b-instruct");
        assert_eq!(pack.manifest().schema_version, 3);
        assert_eq!(pack.documents().len(), 5);
        assert_eq!(
            pack.resolve_variant(&pack.manifest().variants[0].config_ref)
                .expect("variant should resolve")
                .id,
            "q4_k_m"
        );
        assert_eq!(
            pack.resolve_preset(&pack.manifest().presets[0].config_ref)
                .expect("preset should resolve")
                .variant_id,
            "q4_k_m"
        );
    }

    #[test]
    fn rejects_v2_pack_before_legacy_alias_parsing() {
        let bytes = build_pack(vec![(
            MANIFEST_FILE_NAME,
            json!({
                "version": 2,
                "id": "demo",
                "label": "Demo",
                "family": "llama"
            })
            .to_string(),
        )]);

        let error = ModelPack::from_bytes(&bytes).unwrap_err();
        assert!(error.to_string().contains("only schema_version 3"));
    }

    #[test]
    fn rejects_legacy_config_ref_field() {
        let mut entries = valid_pack_entries();
        entries[0].1 = json!({
            "schema_version": 3,
            "deployment": "local",
            "id": "demo",
            "label": "Demo",
            "family": "llama",
            "engines": [{"id": "ggml.llama", "format": "gguf"}],
            "variants": [{
                "id": "q4_k_m",
                "label": "Q4_K_M",
                "$config": "ref://models/variants/q4_k_m.json"
            }],
            "presets": [{
                "id": "default",
                "label": "Default",
                "$ref": "ref://models/presets/default.json"
            }],
            "default_preset": "default"
        })
        .to_string();

        let error = ModelPack::from_bytes(&build_pack(entries)).unwrap_err();
        assert!(error.to_string().contains("$config"));
    }

    #[test]
    fn rejects_pack_without_manifest() {
        let bytes = build_pack(vec![("models/assets/readme.txt", "no manifest".to_owned())]);

        let error = ModelPack::from_bytes(&bytes).unwrap_err();
        assert!(error.to_string().contains("manifest.json"));
    }

    #[test]
    fn normalizes_chat_only_capabilities_to_include_text_generation() {
        let mut entries = valid_pack_entries();
        entries[0].1 = manifest_json(json!(["chat_generation"])).to_string();
        let bytes = build_pack(entries);

        let pack = ModelPack::from_bytes(&bytes).expect("pack should load");

        assert_eq!(
            pack.manifest().capabilities,
            vec![Capability::TextGeneration, Capability::ChatGeneration]
        );
    }

    #[test]
    fn infers_default_llama_capabilities_when_manifest_omits_them() {
        let mut entries = valid_pack_entries();
        entries[0].1 = json!({
            "schema_version": 3,
            "deployment": "local",
            "id": "qwen2.5-7b-instruct",
            "label": "Qwen2.5 7B Instruct",
            "family": "llama",
            "context_window": 8192,
            "engines": [{"id": "ggml.llama", "format": "gguf"}],
            "components": [{
                "id": "model",
                "label": "Primary model",
                "$ref": "ref://models/components/model.json"
            }],
            "variants": [{
                "id": "q4_k_m",
                "label": "Q4_K_M",
                "$ref": "ref://models/variants/q4_k_m.json"
            }],
            "presets": [{
                "id": "default",
                "label": "Default",
                "$ref": "ref://models/presets/default.json"
            }],
            "default_preset": "default"
        })
        .to_string();
        let bytes = build_pack(entries);

        let pack = ModelPack::from_bytes(&bytes).expect("pack should load");

        assert_eq!(
            pack.manifest().capabilities,
            vec![Capability::TextGeneration, Capability::ChatGeneration]
        );
    }

    #[test]
    fn rejects_duplicate_manifest_entry_ids() {
        let mut entries = valid_pack_entries();
        entries[0].1 = json!({
            "schema_version": 3,
            "deployment": "local",
            "id": "demo",
            "label": "Demo",
            "family": "llama",
            "engines": [{"id": "ggml.llama", "format": "gguf"}],
            "variants": [
                {"id": "q4_k_m", "label": "Q4", "$ref": "ref://models/variants/q4_k_m.json"},
                {"id": "q4_k_m", "label": "Q4 duplicate", "$ref": "ref://models/variants/q4_k_m.json"}
            ],
            "presets": [{"id": "default", "label": "Default", "$ref": "ref://models/presets/default.json"}],
            "default_preset": "default"
        })
        .to_string();

        let error = ModelPack::from_bytes(&build_pack(entries)).unwrap_err();
        assert!(error.to_string().contains("duplicate variant id"));
    }

    #[test]
    fn rejects_backend_config_payload_that_is_not_object() {
        let mut entries = valid_pack_entries();
        entries[2].1 = json!({
            "kind": "backend_config",
            "label": "Default load",
            "scope": "load",
            "payload": "nope"
        })
        .to_string();

        let error = ModelPack::from_bytes(&build_pack(entries)).unwrap_err();
        assert!(error.to_string().contains("payload must be a JSON object"));
    }

    #[test]
    fn rejects_missing_text_asset_reference() {
        let mut entries = valid_pack_entries();
        entries[2].1 = json!({
            "kind": "backend_config",
            "label": "Default load",
            "scope": "load",
            "payload": {
                "chat_template": {
                    "$path": "ref://models/assets/missing.jinja"
                }
            }
        })
        .to_string();

        let error = ModelPack::from_bytes(&build_pack(entries)).unwrap_err();
        assert!(error.to_string().contains("references missing asset"));
    }

    #[test]
    fn rejects_variant_format_without_matching_engine() {
        let mut entries = valid_pack_entries();
        entries[4].1 = json!({
            "kind": "variant",
            "id": "q4_k_m",
            "label": "Q4_K_M",
            "format": "safetensors",
            "component_ids": ["model"],
            "$load_config": "ref://models/configs/load.json"
        })
        .to_string();

        let error = ModelPack::from_bytes(&build_pack(entries)).unwrap_err();
        assert!(error.to_string().contains("not supported by any declared engine"));
    }

    fn valid_pack_entries() -> Vec<(&'static str, String)> {
        vec![
            (MANIFEST_FILE_NAME, manifest_json(json!(["text_generation"])).to_string()),
            (
                "models/components/model.json",
                json!({
                    "kind": "component",
                    "id": "model",
                    "label": "Primary model",
                    "source": {
                        "kind": "hugging_face",
                        "repo_id": "Qwen/Qwen2.5-7B-Instruct-GGUF",
                        "revision": "main",
                        "files": [{"id": "model", "path": "Qwen2.5-7B-Instruct-Q4_K_M.gguf"}]
                    }
                })
                .to_string(),
            ),
            (
                "models/configs/load.json",
                json!({
                    "kind": "backend_config",
                    "label": "Default load",
                    "scope": "load",
                    "payload": {
                        "num_workers": 1,
                        "chat_template": {
                            "$path": "ref://models/assets/chat_template.jinja"
                        }
                    }
                })
                .to_string(),
            ),
            (
                "models/configs/inference.json",
                json!({
                    "kind": "backend_config",
                    "label": "Default inference",
                    "scope": "inference",
                    "payload": {"temperature": 0.7}
                })
                .to_string(),
            ),
            (
                "models/variants/q4_k_m.json",
                json!({
                    "kind": "variant",
                    "id": "q4_k_m",
                    "label": "Q4_K_M",
                    "format": "gguf",
                    "component_ids": ["model"],
                    "$load_config": "ref://models/configs/load.json"
                })
                .to_string(),
            ),
            (
                "models/presets/default.json",
                json!({
                    "kind": "preset",
                    "id": "default",
                    "label": "Default",
                    "variant_id": "q4_k_m",
                    "$inference_config": "ref://models/configs/inference.json"
                })
                .to_string(),
            ),
            ("models/assets/chat_template.jinja", "{{ messages }}".to_owned()),
        ]
    }

    fn manifest_json(capabilities: serde_json::Value) -> serde_json::Value {
        json!({
            "schema_version": 3,
            "deployment": "local",
            "id": "qwen2.5-7b-instruct",
            "label": "Qwen2.5 7B Instruct",
            "family": "llama",
            "capabilities": capabilities,
            "context_window": 8192,
            "engines": [{"id": "ggml.llama", "format": "gguf"}],
            "metadata": {"author": "slab"},
            "components": [{
                "id": "model",
                "label": "Primary model",
                "$ref": "ref://models/components/model.json"
            }],
            "variants": [{
                "id": "q4_k_m",
                "label": "Q4_K_M",
                "$ref": "ref://models/variants/q4_k_m.json"
            }],
            "presets": [{
                "id": "default",
                "label": "Default",
                "$ref": "ref://models/presets/default.json"
            }],
            "default_preset": "default",
            "footprint": {"ram_mb": 4096, "vram_mb": 8192}
        })
    }

    fn build_pack(entries: Vec<(&str, String)>) -> Vec<u8> {
        let mut cursor = std::io::Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(&mut cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        for (path, content) in entries {
            writer.start_file(path, options).expect("start file");
            writer.write_all(content.as_bytes()).expect("write file");
        }

        writer.finish().expect("finish zip");
        cursor.into_inner()
    }
}
