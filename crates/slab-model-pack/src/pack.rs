use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use slab_types::{Capability, ModelFamily};
use zip::ZipArchive;

use crate::error::ModelPackError;
use crate::manifest::{
    BackendConfigDocument, BackendConfigScope, ComponentDocument, ConfigEntryRef,
    ModelPackManifest, PackDocument, PresetDocument, VariantDocument,
};
use crate::refs::ConfigRef;

pub const PACK_EXTENSION: &str = "slab";
pub const MANIFEST_FILE_NAME: &str = "manifest.json";

#[derive(Debug, Clone)]
pub struct ModelPack {
    manifest: ModelPackManifest,
    documents: BTreeMap<String, PackDocument>,
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

        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|source| ModelPackError::AccessArchiveEntry { index, source })?;

            if entry.is_dir() {
                continue;
            }

            let path = normalize_archive_path(entry.name())?;
            if !path.ends_with(".json") {
                continue;
            }

            let mut raw = String::new();
            entry.read_to_string(&mut raw).map_err(|source| ModelPackError::ReadArchiveEntry {
                path: path.clone(),
                source,
            })?;

            if path == MANIFEST_FILE_NAME {
                if manifest.is_some() {
                    return Err(ModelPackError::DuplicateDocumentPath { path });
                }

                manifest = Some(parse_json_document(&path, &raw)?);
                continue;
            }

            let document: PackDocument = parse_json_document(&path, &raw)?;
            if documents.insert(path.clone(), document).is_some() {
                return Err(ModelPackError::DuplicateDocumentPath { path });
            }
        }

        let mut manifest = manifest.ok_or(ModelPackError::MissingManifest)?;
        manifest.capabilities = normalized_manifest_capabilities(&manifest);

        let pack = Self { manifest, documents };

        pack.validate_manifest_references()?;
        Ok(pack)
    }

    pub fn manifest(&self) -> &ModelPackManifest {
        &self.manifest
    }

    pub fn documents(&self) -> &BTreeMap<String, PackDocument> {
        &self.documents
    }

    pub fn document(&self, config_ref: &ConfigRef) -> Result<&PackDocument, ModelPackError> {
        self.documents.get(config_ref.path()).ok_or_else(|| {
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
        for reference in &self.manifest.components {
            self.validate_entry_ref(reference, "component")?;
        }
        for reference in &self.manifest.variants {
            self.validate_entry_ref(reference, "variant")?;
        }
        for reference in &self.manifest.adapters {
            self.validate_entry_ref(reference, "adapter")?;
        }
        for reference in &self.manifest.presets {
            self.validate_entry_ref(reference, "preset")?;
        }

        let component_ids = self.collect_ids("component");
        let variant_ids = self.collect_ids("variant");
        let adapter_ids = self.collect_ids("adapter");

        for document in self.documents.values() {
            match document {
                PackDocument::Variant(variant) => {
                    self.validate_component_ids(&variant.component_ids, &component_ids)?;
                    if let Some(config_ref) = &variant.load_config {
                        self.resolve_backend_config(config_ref, BackendConfigScope::Load)?;
                    }
                    if let Some(config_ref) = &variant.inference_config {
                        self.resolve_backend_config(config_ref, BackendConfigScope::Inference)?;
                    }
                }
                PackDocument::Adapter(adapter) => {
                    self.validate_component_ids(&adapter.component_ids, &component_ids)?;
                }
                PackDocument::Preset(preset) => {
                    if let Some(variant_id) = &preset.variant_id
                        && !variant_ids.contains(variant_id)
                    {
                        return Err(ModelPackError::MissingNamedDocument {
                            kind: "variant",
                            id: variant_id.clone(),
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
                    if let Some(config_ref) = &preset.load_config {
                        self.resolve_backend_config(config_ref, BackendConfigScope::Load)?;
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

    fn validate_entry_ref(
        &self,
        reference: &ConfigEntryRef,
        expected_kind: &'static str,
    ) -> Result<(), ModelPackError> {
        let document = self.document(&reference.config_ref).map_err(|_| {
            ModelPackError::MissingReferencedDocument {
                from: reference.id.clone(),
                path: reference.config_ref.path().into(),
            }
        })?;

        if document.kind() != expected_kind {
            return Err(ModelPackError::UnexpectedDocumentKind {
                path: reference.config_ref.path().into(),
                expected: expected_kind,
                found: document.kind(),
            });
        }

        if document.id() != reference.id {
            return Err(ModelPackError::DocumentIdMismatch {
                path: reference.config_ref.path().into(),
                expected: reference.id.clone(),
                found: document.id().to_owned(),
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
    fn loads_manifest_and_referenced_documents_from_slab_bytes() {
        let bytes = build_pack(vec![
            (
                MANIFEST_FILE_NAME,
                json!({
                    "version": 2,
                    "id": "qwen2.5-7b-instruct",
                    "label": "Qwen2.5 7B Instruct",
                    "family": "llama",
                    "capabilities": ["text_generation"],
                    "backend_hints": {
                        "prefer_drivers": ["ggml.llama"],
                        "avoid_drivers": [],
                        "require_streaming": true
                    },
                    "metadata": {
                        "author": "slab"
                    },
                    "variants": [
                        {
                            "id": "q4_k_m",
                            "label": "Q4_K_M",
                            "$config": "ref://models/variants/q4_k_m.json"
                        }
                    ],
                    "components": [
                        {
                            "id": "model",
                            "label": "Primary model",
                            "$config": "ref://models/components/model.json"
                        }
                    ],
                    "presets": [
                        {
                            "id": "default",
                            "label": "Default",
                            "$config": "ref://models/presets/default.json"
                        }
                    ],
                    "default_preset": "default",
                    "footprint": {
                        "ram_mb": 4096,
                        "vram_mb": 8192
                    }
                })
                .to_string(),
            ),
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
                        "files": [
                            {
                                "id": "model",
                                "path": "Qwen2.5-7B-Instruct-Q4_K_M.gguf"
                            }
                        ]
                    }
                })
                .to_string(),
            ),
            (
                "models/configs/load.json",
                json!({
                    "kind": "backend_config",
                    "id": "load-default",
                    "label": "Default load",
                    "scope": "load",
                    "payload": {
                        "context_length": 8192,
                        "num_workers": 1
                    }
                })
                .to_string(),
            ),
            (
                "models/configs/inference.json",
                json!({
                    "kind": "backend_config",
                    "id": "inference-default",
                    "label": "Default inference",
                    // "backend": "ggml_llama",
                    "scope": "inference",
                    "payload": {
                        "temperature": 0.7,
                        "top_p": 0.95,
                        "max_tokens": 2048
                    }
                })
                .to_string(),
            ),
            (
                "models/variants/q4_k_m.json",
                json!({
                    "kind": "variant",
                    "id": "q4_k_m",
                    "label": "Q4_K_M",
                    "component_ids": ["model"],
                    "$load_config": "ref://models/configs/load.json",
                    "$inference_config": "ref://models/configs/inference.json"
                })
                .to_string(),
            ),
            (
                "models/presets/default.json",
                json!({
                    "kind": "preset",
                    "id": "default",
                    "label": "Default",
                    "$load_config": "ref://models/configs/load.json",
                    "$inference_config": "ref://models/configs/inference.json"
                })
                .to_string(),
            ),
        ]);

        let pack = ModelPack::from_bytes(&bytes).expect("pack should load");

        assert_eq!(pack.manifest().id, "qwen2.5-7b-instruct");
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
            None
        );
    }

    #[test]
    fn rejects_legacy_ref_field_names_without_dollar_prefix() {
        let bytes = build_pack(vec![
            (
                MANIFEST_FILE_NAME,
                json!({
                    "version": 2,
                    "id": "demo",
                    "label": "Demo",
                    "family": "llama",
                    "variants": [
                        {
                            "id": "q4_k_m",
                            "label": "Q4_K_M",
                            "$config": "ref://models/variants/q4_k_m.json"
                        }
                    ]
                })
                .to_string(),
            ),
            (
                "models/variants/q4_k_m.json",
                json!({
                    "kind": "variant",
                    "id": "q4_k_m",
                    "label": "Q4_K_M",
                    "load_config": "ref://models/configs/load.json"
                })
                .to_string(),
            ),
            (
                "models/configs/load.json",
                json!({
                    "kind": "backend_config",
                    "id": "load-default",
                    "label": "Default load",
                    "scope": "load",
                    "payload": {
                        "context_length": 8192
                    }
                })
                .to_string(),
            ),
        ]);

        let error = ModelPack::from_bytes(&bytes).unwrap_err();
        assert!(error.to_string().contains("unknown field `load_config`"));
    }

    #[test]
    fn rejects_removed_backend_field_in_sub_configs() {
        let bytes = build_pack(vec![
            (
                MANIFEST_FILE_NAME,
                json!({
                    "version": 2,
                    "id": "demo",
                    "label": "Demo",
                    "family": "llama",
                    "variants": [
                        {
                            "id": "q4_k_m",
                            "label": "Q4_K_M",
                            "$config": "ref://models/variants/q4_k_m.json"
                        }
                    ]
                })
                .to_string(),
            ),
            (
                "models/variants/q4_k_m.json",
                json!({
                    "kind": "variant",
                    "id": "q4_k_m",
                    "label": "Q4_K_M",
                    "$load_config": "ref://models/configs/load.json"
                })
                .to_string(),
            ),
            (
                "models/configs/load.json",
                json!({
                    "kind": "backend_config",
                    "id": "load-default",
                    "label": "Default load",
                    "backend": "ggml_llama",
                    "scope": "load",
                    "payload": {
                        "context_length": 8192
                    }
                })
                .to_string(),
            ),
        ]);

        let error = ModelPack::from_bytes(&bytes).unwrap_err();
        assert!(error.to_string().contains("unknown field `backend`"));
    }

    #[test]
    fn rejects_pack_without_manifest() {
        let bytes = build_pack(vec![(
            "models/variants/q4.json",
            json!({
                "kind": "variant",
                "id": "q4",
                "label": "Q4"
            })
            .to_string(),
        )]);

        let error = ModelPack::from_bytes(&bytes).unwrap_err();
        assert!(error.to_string().contains("manifest.json"));
    }

    #[test]
    fn normalizes_chat_only_capabilities_to_include_text_generation() {
        let bytes = build_pack(vec![(
            MANIFEST_FILE_NAME,
            json!({
                "version": 2,
                "id": "qwen2.5-0.5b-instruct",
                "label": "Qwen2.5 0.5B Instruct",
                "family": "llama",
                "capabilities": ["chat_generation"],
                "backend_hints": {
                    "prefer_drivers": ["ggml.llama"],
                    "avoid_drivers": [],
                    "require_streaming": true
                }
            })
            .to_string(),
        )]);

        let pack = ModelPack::from_bytes(&bytes).expect("pack should load");

        assert_eq!(
            pack.manifest().capabilities,
            vec![Capability::TextGeneration, Capability::ChatGeneration]
        );
    }

    #[test]
    fn infers_default_llama_capabilities_when_manifest_omits_them() {
        let bytes = build_pack(vec![(
            MANIFEST_FILE_NAME,
            json!({
                "version": 2,
                "id": "qwen2.5-0.5b-instruct",
                "label": "Qwen2.5 0.5B Instruct",
                "family": "llama",
                "backend_hints": {
                    "prefer_drivers": ["ggml.llama"],
                    "avoid_drivers": [],
                    "require_streaming": true
                }
            })
            .to_string(),
        )]);

        let pack = ModelPack::from_bytes(&bytes).expect("pack should load");

        assert_eq!(
            pack.manifest().capabilities,
            vec![Capability::TextGeneration, Capability::ChatGeneration]
        );
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
