use std::collections::BTreeMap;

use crate::error::ModelPackError;
use crate::manifest::{
    AdapterDocument, BackendConfigDocument, BackendConfigScope, ComponentDocument,
    ModelPackManifest, PackSource, PackSourceCandidate, PresetDocument, VariantDocument,
};
use crate::pack::ModelPack;
use crate::refs::ConfigRef;

#[derive(Debug, Clone)]
pub struct ResolvedModelPack {
    pub manifest: ModelPackManifest,
    pub components: BTreeMap<String, ResolvedComponent>,
    pub adapters: BTreeMap<String, ResolvedAdapter>,
    pub variants: BTreeMap<String, ResolvedVariant>,
    pub presets: BTreeMap<String, ResolvedPreset>,
    pub default_preset_id: Option<String>,
}

impl ResolvedModelPack {
    pub fn default_preset(&self) -> Option<&ResolvedPreset> {
        self.default_preset_id.as_ref().and_then(|id| self.presets.get(id))
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedComponent {
    pub document: ComponentDocument,
}

#[derive(Debug, Clone)]
pub struct ResolvedAdapter {
    pub document: AdapterDocument,
    pub components: BTreeMap<String, ResolvedComponent>,
}

#[derive(Debug, Clone)]
pub struct ResolvedVariant {
    pub document: VariantDocument,
    pub effective_sources: Vec<PackSourceCandidate>,
    pub components: BTreeMap<String, ResolvedComponent>,
    pub load_config: Option<BackendConfigDocument>,
    pub inference_config: Option<BackendConfigDocument>,
}

#[derive(Debug, Clone)]
pub struct ResolvedPreset {
    pub document: PresetDocument,
    pub variant: ResolvedVariant,
    pub adapters: BTreeMap<String, ResolvedAdapter>,
    pub effective_load_config: Option<BackendConfigDocument>,
    pub effective_inference_config: Option<BackendConfigDocument>,
}

impl ModelPack {
    pub fn resolve(&self) -> Result<ResolvedModelPack, ModelPackError> {
        let components = self.resolve_components()?;
        let adapters = self.resolve_adapters(&components)?;
        let variants = self.resolve_variants(&components)?;
        let presets = self.resolve_presets(&components, &variants, &adapters)?;
        let default_preset_id = self.resolve_default_preset_id(&presets)?;

        Ok(ResolvedModelPack {
            manifest: self.manifest().clone(),
            components,
            adapters,
            variants,
            presets,
            default_preset_id,
        })
    }

    fn resolve_components(&self) -> Result<BTreeMap<String, ResolvedComponent>, ModelPackError> {
        let mut resolved = BTreeMap::new();
        for entry in &self.manifest().components {
            let document = self.resolve_component(&entry.config_ref)?.clone();
            resolved.insert(entry.id.clone(), ResolvedComponent { document });
        }
        Ok(resolved)
    }

    fn resolve_adapters(
        &self,
        components: &BTreeMap<String, ResolvedComponent>,
    ) -> Result<BTreeMap<String, ResolvedAdapter>, ModelPackError> {
        let mut resolved = BTreeMap::new();
        for entry in &self.manifest().adapters {
            let document = match self.document(&entry.config_ref)? {
                crate::manifest::PackDocument::Adapter(document) => document.clone(),
                other => {
                    return Err(ModelPackError::UnexpectedDocumentKind {
                        path: entry.config_ref.path().into(),
                        expected: "adapter",
                        found: other.kind(),
                    });
                }
            };

            let resolved_components =
                resolve_named_components(components, &document.component_ids)?;
            resolved.insert(
                entry.id.clone(),
                ResolvedAdapter { document, components: resolved_components },
            );
        }
        Ok(resolved)
    }

    fn resolve_variants(
        &self,
        components: &BTreeMap<String, ResolvedComponent>,
    ) -> Result<BTreeMap<String, ResolvedVariant>, ModelPackError> {
        let mut resolved = BTreeMap::new();
        for entry in &self.manifest().variants {
            let document = self.resolve_variant(&entry.config_ref)?.clone();
            let resolved_components =
                resolve_named_components(components, &document.component_ids)?;
            let effective_sources =
                resolve_variant_effective_sources(&self.manifest().sources, &document);
            let load_config = document
                .load_config
                .as_ref()
                .map(|config_ref| {
                    self.resolve_backend_config(config_ref, BackendConfigScope::Load).cloned()
                })
                .transpose()?;
            let inference_config = document
                .inference_config
                .as_ref()
                .map(|config_ref| {
                    self.resolve_backend_config(config_ref, BackendConfigScope::Inference).cloned()
                })
                .transpose()?;

            resolved.insert(
                entry.id.clone(),
                ResolvedVariant {
                    document,
                    effective_sources,
                    components: resolved_components,
                    load_config,
                    inference_config,
                },
            );
        }
        Ok(resolved)
    }

    fn resolve_presets(
        &self,
        components: &BTreeMap<String, ResolvedComponent>,
        variants: &BTreeMap<String, ResolvedVariant>,
        adapters: &BTreeMap<String, ResolvedAdapter>,
    ) -> Result<BTreeMap<String, ResolvedPreset>, ModelPackError> {
        let mut resolved = BTreeMap::new();
        for entry in &self.manifest().presets {
            let mut document = self.resolve_preset(&entry.config_ref)?.clone();
            if document.variant_id.is_none() {
                document.variant_id = entry.variant_id.clone();
            }
            let variant = resolve_preset_variant(self, components, variants, &document)?;

            let mut resolved_adapters = BTreeMap::new();
            for adapter_id in &document.adapter_ids {
                let adapter = adapters.get(adapter_id).cloned().ok_or_else(|| {
                    ModelPackError::MissingNamedDocument { kind: "adapter", id: adapter_id.clone() }
                })?;
                resolved_adapters.insert(adapter_id.clone(), adapter);
            }

            let effective_load_config = resolve_effective_backend_config(
                self,
                document.load_config.as_ref(),
                variant.load_config.as_ref(),
                BackendConfigScope::Load,
            )?;
            let effective_inference_config = resolve_effective_backend_config(
                self,
                document.inference_config.as_ref(),
                variant.inference_config.as_ref(),
                BackendConfigScope::Inference,
            )?;

            resolved.insert(
                entry.id.clone(),
                ResolvedPreset {
                    document,
                    variant,
                    adapters: resolved_adapters,
                    effective_load_config,
                    effective_inference_config,
                },
            );
        }
        Ok(resolved)
    }

    fn resolve_default_preset_id(
        &self,
        presets: &BTreeMap<String, ResolvedPreset>,
    ) -> Result<Option<String>, ModelPackError> {
        if let Some(default_preset_id) = &self.manifest().default_preset {
            if presets.contains_key(default_preset_id) {
                return Ok(Some(default_preset_id.clone()));
            }
            return Err(ModelPackError::MissingDefaultPreset { id: default_preset_id.clone() });
        }

        match presets.len() {
            0 => Ok(None),
            1 => Ok(presets.keys().next().cloned()),
            _ => Err(ModelPackError::MissingDefaultPresetDeclaration),
        }
    }
}

fn resolve_variant_effective_sources(
    manifest_sources: &[PackSourceCandidate],
    document: &VariantDocument,
) -> Vec<PackSourceCandidate> {
    let sources = if document.sources.is_empty() {
        manifest_sources.to_vec()
    } else {
        document.sources.clone()
    };

    ordered_source_candidates(sources)
        .into_iter()
        .map(|candidate| {
            candidate.with_source(select_variant_source(&candidate.source, &document.id))
        })
        .collect()
}

fn select_variant_source(source: &PackSource, variant_id: &str) -> PackSource {
    match source {
        PackSource::LocalFiles { files } => files
            .iter()
            .find(|file| file.id == variant_id)
            .cloned()
            .map(|file| PackSource::LocalFiles { files: vec![file] })
            .unwrap_or_else(|| source.clone()),
        PackSource::HuggingFace { repo_id, revision, files } => files
            .iter()
            .find(|file| file.id == variant_id)
            .cloned()
            .map(|file| PackSource::HuggingFace {
                repo_id: repo_id.clone(),
                revision: revision.clone(),
                files: vec![file],
            })
            .unwrap_or_else(|| source.clone()),
        PackSource::LocalPath { .. } | PackSource::Cloud { .. } => source.clone(),
    }
}

fn ordered_source_candidates(candidates: Vec<PackSourceCandidate>) -> Vec<PackSourceCandidate> {
    let mut indexed = candidates.into_iter().enumerate().collect::<Vec<_>>();
    indexed.sort_by(|(left_index, left), (right_index, right)| {
        left.priority
            .unwrap_or(i32::MAX)
            .cmp(&right.priority.unwrap_or(i32::MAX))
            .then_with(|| left_index.cmp(right_index))
    });
    indexed.into_iter().map(|(_, candidate)| candidate).collect()
}

fn resolve_named_components(
    components: &BTreeMap<String, ResolvedComponent>,
    component_ids: &[String],
) -> Result<BTreeMap<String, ResolvedComponent>, ModelPackError> {
    let mut resolved = BTreeMap::new();
    for component_id in component_ids {
        let component = components.get(component_id).cloned().ok_or_else(|| {
            ModelPackError::MissingNamedDocument { kind: "component", id: component_id.clone() }
        })?;
        resolved.insert(component_id.clone(), component);
    }
    Ok(resolved)
}

fn resolve_preset_variant(
    pack: &ModelPack,
    components: &BTreeMap<String, ResolvedComponent>,
    variants: &BTreeMap<String, ResolvedVariant>,
    preset: &PresetDocument,
) -> Result<ResolvedVariant, ModelPackError> {
    let Some(variant_id) = preset.variant_id.as_deref() else {
        return Ok(resolve_manifest_default_variant(pack, components));
    };

    variants.get(variant_id).cloned().ok_or_else(|| ModelPackError::MissingNamedDocument {
        kind: "variant",
        id: variant_id.to_owned(),
    })
}

fn resolve_manifest_default_variant(
    pack: &ModelPack,
    components: &BTreeMap<String, ResolvedComponent>,
) -> ResolvedVariant {
    let (component_ids, resolved_components, effective_sources) = if !pack
        .manifest()
        .sources
        .is_empty()
    {
        (Vec::new(), BTreeMap::new(), ordered_source_candidates(pack.manifest().sources.clone()))
    } else {
        (components.keys().cloned().collect(), components.clone(), Vec::new())
    };

    ResolvedVariant {
        document: VariantDocument {
            id: String::new(),
            label: "Original Model".to_owned(),
            description: Some("Resolved from manifest without an explicit variant".to_owned()),
            sources: Vec::new(),
            component_ids,
            load_config: None,
            inference_config: None,
            metadata: BTreeMap::new(),
        },
        effective_sources,
        components: resolved_components,
        load_config: None,
        inference_config: None,
    }
}

fn resolve_effective_backend_config(
    pack: &ModelPack,
    override_ref: Option<&ConfigRef>,
    fallback: Option<&BackendConfigDocument>,
    scope: BackendConfigScope,
) -> Result<Option<BackendConfigDocument>, ModelPackError> {
    if let Some(config_ref) = override_ref {
        return pack.resolve_backend_config(config_ref, scope).cloned().map(Some);
    }

    Ok(fallback.cloned())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use serde_json::json;
    use zip::CompressionMethod;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    use crate::manifest::PackSource;
    use crate::pack::ModelPack;

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

    #[test]
    fn resolves_default_preset_and_effective_configs() {
        let bytes = build_pack(vec![
            (
                "manifest.json",
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
                    "components": [
                        {
                            "id": "model",
                            "label": "Model",
                            "$config": "ref://models/components/model.json"
                        }
                    ],
                    "variants": [
                        {
                            "id": "q4_k_m",
                            "label": "Q4_K_M",
                            "$config": "ref://models/variants/q4_k_m.json"
                        }
                    ],
                    "presets": [
                        {
                            "id": "default",
                            "label": "Default",
                            "$config": "ref://models/presets/default.json"
                        },
                        {
                            "id": "long-context",
                            "label": "Long Context",
                            "$config": "ref://models/presets/long-context.json"
                        }
                    ],
                    "default_preset": "default"
                })
                .to_string(),
            ),
            (
                "models/components/model.json",
                json!({
                    "kind": "component",
                    "id": "model",
                    "label": "Model",
                    "source": {
                        "kind": "local_path",
                        "path": "C:/models/qwen.gguf"
                    }
                })
                .to_string(),
            ),
            (
                "models/configs/load-default.json",
                json!({
                    "kind": "backend_config",
                    "id": "load-default",
                    "label": "Load Default",
                    "scope": "load",
                    "payload": {
                        "context_length": 8192
                    }
                })
                .to_string(),
            ),
            (
                "models/configs/load-long.json",
                json!({
                    "kind": "backend_config",
                    "id": "load-long",
                    "label": "Load Long",
                    "scope": "load",
                    "payload": {
                        "context_length": 32768
                    }
                })
                .to_string(),
            ),
            (
                "models/configs/inference-default.json",
                json!({
                    "kind": "backend_config",
                    "id": "inference-default",
                    "label": "Inference Default",
                    "scope": "inference",
                    "payload": {
                        "temperature": 0.7
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
                    "$load_config": "ref://models/configs/load-default.json",
                    "$inference_config": "ref://models/configs/inference-default.json"
                })
                .to_string(),
            ),
            (
                "models/presets/default.json",
                json!({
                    "kind": "preset",
                    "id": "default",
                    "label": "Default",
                    "variant_id": "q4_k_m"
                })
                .to_string(),
            ),
            (
                "models/presets/long-context.json",
                json!({
                    "kind": "preset",
                    "id": "long-context",
                    "label": "Long Context",
                    "variant_id": "q4_k_m",
                    "$load_config": "ref://models/configs/load-long.json"
                })
                .to_string(),
            ),
        ]);

        let pack = ModelPack::from_bytes(&bytes).expect("pack should load");
        let resolved = pack.resolve().expect("pack should resolve");

        assert_eq!(resolved.default_preset_id.as_deref(), Some("default"));
        assert_eq!(
            resolved
                .default_preset()
                .and_then(|preset| preset.effective_load_config.as_ref())
                .and_then(|config| config.payload.get("context_length"))
                .and_then(|value| value.as_u64()),
            Some(8192)
        );
        assert_eq!(
            resolved
                .presets
                .get("long-context")
                .and_then(|preset| preset.effective_load_config.as_ref())
                .and_then(|config| config.payload.get("context_length"))
                .and_then(|value| value.as_u64()),
            Some(32768)
        );
    }

    #[test]
    fn resolves_preset_without_variant_id_to_manifest_source() {
        let bytes = build_pack(vec![
            (
                "manifest.json",
                json!({
                    "version": 2,
                    "id": "demo",
                    "label": "Demo",
                    "family": "llama",
                    "capabilities": ["text_generation"],
                    "source": {
                        "kind": "local_path",
                        "path": "C:/models/base.gguf"
                    },
                    "presets": [
                        {
                            "id": "default",
                            "label": "Default",
                            "$config": "ref://models/presets/default.json"
                        }
                    ],
                    "default_preset": "default"
                })
                .to_string(),
            ),
            (
                "models/configs/load.json",
                json!({
                    "kind": "backend_config",
                    "id": "load-default",
                    "label": "Load Default",
                    "scope": "load",
                    "payload": {
                        "context_length": 4096
                    }
                })
                .to_string(),
            ),
            (
                "models/presets/default.json",
                json!({
                    "kind": "preset",
                    "id": "default",
                    "label": "Default",
                    "$load_config": "ref://models/configs/load.json"
                })
                .to_string(),
            ),
        ]);

        let pack = ModelPack::from_bytes(&bytes).expect("pack should load");
        let resolved = pack.resolve().expect("pack should resolve");
        let preset = resolved.default_preset().expect("default preset");

        assert_eq!(
            preset.variant.effective_sources.first().map(|candidate| &candidate.source),
            Some(&PackSource::LocalPath { path: "C:/models/base.gguf".to_owned() })
        );
        assert_eq!(
            preset
                .effective_load_config
                .as_ref()
                .and_then(|config| config.payload.get("context_length"))
                .and_then(|value| value.as_u64()),
            Some(4096)
        );
    }

    #[test]
    fn resolves_variant_id_to_matching_manifest_source_file() {
        let bytes = build_pack(vec![
            (
                "manifest.json",
                json!({
                    "version": 2,
                    "id": "qwen2.5-0.5b-instruct",
                    "label": "Qwen2.5 0.5B Instruct",
                    "family": "llama",
                    "capabilities": ["text_generation"],
                    "source": {
                        "kind": "hugging_face",
                        "repo_id": "bartowski/Qwen2.5-0.5B-Instruct-GGUF",
                        "files": [
                            { "id": "model", "path": "Qwen2.5-0.5B-Instruct-f16.gguf" },
                            { "id": "Q4_K_M", "path": "Qwen2.5-0.5B-Instruct-Q4_K_M.gguf" },
                            { "id": "Q8_0", "path": "Qwen2.5-0.5B-Instruct-Q8_0.gguf" }
                        ]
                    },
                    "variants": [
                        { "id": "Q8_0", "label": "Q8_0", "$config": "ref://models/variants/q8_0.json" }
                    ],
                    "presets": [
                        { "id": "default", "label": "Default", "$config": "ref://models/presets/default.json" }
                    ],
                    "default_preset": "default"
                })
                .to_string(),
            ),
            (
                "models/variants/q8_0.json",
                json!({
                    "kind": "variant",
                    "id": "Q8_0",
                    "label": "Q8_0"
                })
                .to_string(),
            ),
            (
                "models/presets/default.json",
                json!({
                    "kind": "preset",
                    "id": "default",
                    "label": "Default",
                    "variant_id": "Q8_0"
                })
                .to_string(),
            ),
        ]);

        let pack = ModelPack::from_bytes(&bytes).expect("pack should load");
        let resolved = pack.resolve().expect("pack should resolve");
        let preset = resolved.default_preset().expect("default preset");

        assert_eq!(
            preset.variant.effective_sources.first().map(|candidate| &candidate.source),
            Some(&PackSource::HuggingFace {
                repo_id: "bartowski/Qwen2.5-0.5B-Instruct-GGUF".to_owned(),
                revision: None,
                files: vec![crate::manifest::PackSourceFile {
                    id: "Q8_0".to_owned(),
                    label: None,
                    description: None,
                    path: "Qwen2.5-0.5B-Instruct-Q8_0.gguf".to_owned(),
                }],
            })
        );
    }

    #[test]
    fn resolves_manifest_preset_variant_override_to_matching_source_file() {
        let bytes = build_pack(vec![
            (
                "manifest.json",
                json!({
                    "version": 2,
                    "id": "qwen2.5-0.5b-instruct",
                    "label": "Qwen2.5 0.5B Instruct",
                    "family": "llama",
                    "capabilities": ["text_generation"],
                    "source": {
                        "kind": "hugging_face",
                        "repo_id": "bartowski/Qwen2.5-0.5B-Instruct-GGUF",
                        "files": [
                            { "id": "model", "path": "Qwen2.5-0.5B-Instruct-f16.gguf" },
                            { "id": "Q8_0", "path": "Qwen2.5-0.5B-Instruct-Q8_0.gguf" }
                        ]
                    },
                    "variants": [
                        { "id": "Q8_0", "label": "Q8_0", "$config": "ref://models/variants/q8_0.json" }
                    ],
                    "presets": [
                        {
                            "id": "default",
                            "label": "Default",
                            "variant_id": "Q8_0",
                            "$config": "ref://models/presets/default.json"
                        }
                    ],
                    "default_preset": "default"
                })
                .to_string(),
            ),
            (
                "models/variants/q8_0.json",
                json!({
                    "kind": "variant",
                    "id": "Q8_0",
                    "label": "Q8_0"
                })
                .to_string(),
            ),
            (
                "models/presets/default.json",
                json!({
                    "kind": "preset",
                    "id": "default",
                    "label": "Default"
                })
                .to_string(),
            ),
        ]);

        let pack = ModelPack::from_bytes(&bytes).expect("pack should load");
        let resolved = pack.resolve().expect("pack should resolve");
        let preset = resolved.default_preset().expect("default preset");

        assert_eq!(preset.document.variant_id.as_deref(), Some("Q8_0"));
        assert_eq!(
            preset.variant.effective_sources.first().map(|candidate| &candidate.source),
            Some(&PackSource::HuggingFace {
                repo_id: "bartowski/Qwen2.5-0.5B-Instruct-GGUF".to_owned(),
                revision: None,
                files: vec![crate::manifest::PackSourceFile {
                    id: "Q8_0".to_owned(),
                    label: None,
                    description: None,
                    path: "Qwen2.5-0.5B-Instruct-Q8_0.gguf".to_owned(),
                }],
            })
        );
    }

    #[test]
    fn rejects_multiple_presets_without_default_selection() {
        let bytes = build_pack(vec![
            (
                "manifest.json",
                json!({
                    "version": 2,
                    "id": "demo",
                    "label": "Demo",
                    "family": "llama",
                    "capabilities": ["text_generation"],
                    "presets": [
                        { "id": "a", "label": "A", "$config": "ref://models/presets/a.json" },
                        { "id": "b", "label": "B", "$config": "ref://models/presets/b.json" }
                    ],
                    "variants": [
                        { "id": "v", "label": "V", "$config": "ref://models/variants/v.json" }
                    ]
                })
                .to_string(),
            ),
            (
                "models/variants/v.json",
                json!({
                    "kind": "variant",
                    "id": "v",
                    "label": "V"
                })
                .to_string(),
            ),
            (
                "models/presets/a.json",
                json!({
                    "kind": "preset",
                    "id": "a",
                    "label": "A",
                    "variant_id": "v"
                })
                .to_string(),
            ),
            (
                "models/presets/b.json",
                json!({
                    "kind": "preset",
                    "id": "b",
                    "label": "B",
                    "variant_id": "v"
                })
                .to_string(),
            ),
        ]);

        let pack = ModelPack::from_bytes(&bytes).expect("pack should load");
        let error = pack.resolve().unwrap_err();

        assert!(error.to_string().contains("default_preset"));
    }
}
