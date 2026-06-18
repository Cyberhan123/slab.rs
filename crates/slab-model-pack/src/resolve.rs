use std::collections::BTreeMap;

use crate::error::ModelPackError;
use crate::manifest::{
    AdapterDocument, BackendConfigDocument, BackendConfigScope, ComponentDocument, EngineTarget,
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
    pub text_assets: BTreeMap<String, String>,
}

impl ResolvedModelPack {
    pub fn default_preset(&self) -> Option<&ResolvedPreset> {
        self.default_preset_id.as_ref().and_then(|id| self.presets.get(id))
    }

    pub fn text_asset(&self, config_ref: &ConfigRef) -> Result<&str, ModelPackError> {
        self.text_assets.get(config_ref.path()).map(String::as_str).ok_or_else(|| {
            ModelPackError::MissingReferencedDocument {
                from: "resolved_model_pack".into(),
                path: config_ref.path().into(),
            }
        })
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
}

#[derive(Debug, Clone)]
pub struct ResolvedPreset {
    pub document: PresetDocument,
    pub variant: ResolvedVariant,
    pub adapters: BTreeMap<String, ResolvedAdapter>,
    pub effective_inference_config: Option<BackendConfigDocument>,
    pub engine_candidates: Vec<EngineTarget>,
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
            text_assets: self.text_assets().clone(),
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

            resolved.insert(
                entry.id.clone(),
                ResolvedVariant {
                    document,
                    effective_sources,
                    components: resolved_components,
                    load_config,
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
            let document = self.resolve_preset(&entry.config_ref)?.clone();
            let variant = resolve_preset_variant(self, components, variants, &document)?;

            let mut resolved_adapters = BTreeMap::new();
            for adapter_id in &document.adapter_ids {
                let adapter = adapters.get(adapter_id).cloned().ok_or_else(|| {
                    ModelPackError::MissingNamedDocument { kind: "adapter", id: adapter_id.clone() }
                })?;
                resolved_adapters.insert(adapter_id.clone(), adapter);
            }

            let effective_inference_config = document
                .inference_config
                .as_ref()
                .map(|config_ref| {
                    self.resolve_backend_config(config_ref, BackendConfigScope::Inference).cloned()
                })
                .transpose()?;
            let engine_candidates = self
                .manifest()
                .engines
                .iter()
                .copied()
                .filter(|engine| engine.format == variant.document.format)
                .collect::<Vec<_>>();
            if engine_candidates.is_empty() {
                return Err(ModelPackError::MissingCompatibleEngines {
                    preset_id: document.id.clone(),
                    variant_id: variant.document.id.clone(),
                });
            }

            resolved.insert(
                entry.id.clone(),
                ResolvedPreset {
                    document,
                    variant,
                    adapters: resolved_adapters,
                    effective_inference_config,
                    engine_candidates,
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
            1 => Err(ModelPackError::MissingDefaultPresetDeclaration),
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
            .map(|file| source.with_files(vec![file]))
            .unwrap_or_else(|| source.clone()),
        PackSource::HuggingFace { files, .. } | PackSource::ModelScope { files, .. } => files
            .iter()
            .find(|file| file.id == variant_id)
            .cloned()
            .map(|file| source.with_files(vec![file]))
            .unwrap_or_else(|| source.clone()),
        PackSource::LocalPath { .. } => source.clone(),
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
    _pack: &ModelPack,
    _components: &BTreeMap<String, ResolvedComponent>,
    variants: &BTreeMap<String, ResolvedVariant>,
    preset: &PresetDocument,
) -> Result<ResolvedVariant, ModelPackError> {
    variants.get(&preset.variant_id).cloned().ok_or_else(|| ModelPackError::MissingNamedDocument {
        kind: "variant",
        id: preset.variant_id.clone(),
    })
}

#[cfg(test)]
mod v3_tests {
    use std::io::Write;

    use serde_json::json;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    use crate::manifest::PackSource;
    use crate::pack::ModelPack;

    #[test]
    fn resolves_default_preset_and_engine_candidates() {
        let pack = ModelPack::from_bytes(&build_pack(valid_pack_entries(true))).expect("load pack");
        let resolved = pack.resolve().expect("resolve pack");
        let preset = resolved.default_preset().expect("default preset");

        assert_eq!(resolved.default_preset_id.as_deref(), Some("default"));
        assert_eq!(preset.document.variant_id, "Q8_0");
        assert_eq!(
            preset
                .variant
                .load_config
                .as_ref()
                .and_then(|config| config.payload.get("num_workers"))
                .and_then(|value| value.as_u64()),
            Some(2)
        );
        assert_eq!(
            preset
                .effective_inference_config
                .as_ref()
                .and_then(|config| config.payload.get("temperature"))
                .and_then(|value| value.as_f64()),
            Some(0.7)
        );
        assert_eq!(preset.engine_candidates.len(), 1);
        assert_eq!(preset.engine_candidates[0].id.canonical_id(), "ggml.llama");
        assert_eq!(
            preset.variant.effective_sources.first().map(|candidate| &candidate.source),
            Some(&PackSource::LocalPath {
                path: "C:/models/Qwen2.5-0.5B-Instruct-Q8_0.gguf".to_owned(),
            })
        );
    }

    #[test]
    fn rejects_single_preset_without_default_selection() {
        let pack =
            ModelPack::from_bytes(&build_pack(valid_pack_entries(false))).expect("load pack");
        let error = pack.resolve().expect_err("default_preset is required");

        assert!(error.to_string().contains("default_preset"));
    }

    fn valid_pack_entries(include_default: bool) -> Vec<(&'static str, String)> {
        let mut manifest = json!({
            "schema_version": 3,
            "deployment": "local",
            "id": "qwen2.5-0.5b-instruct",
            "label": "Qwen2.5 0.5B Instruct",
            "family": "llama",
            "capabilities": ["text_generation"],
            "engines": [
                {"id": "ggml.llama", "format": "gguf"},
                {"id": "candle.llama", "format": "safetensors"}
            ],
            "sources": [{
                "kind": "local_path",
                "path": "C:/models/Qwen2.5-0.5B-Instruct-Q8_0.gguf"
            }],
            "variants": [
                {"id": "Q8_0", "label": "Q8_0", "$ref": "ref://models/variants/q8_0.json"}
            ],
            "presets": [
                {"id": "default", "label": "Default", "$ref": "ref://models/presets/default.json"}
            ]
        });
        if include_default {
            manifest["default_preset"] = json!("default");
        }

        vec![
            ("manifest.json", manifest.to_string()),
            (
                "models/configs/load-default.json",
                json!({
                    "kind": "backend_config",
                    "label": "Load Default",
                    "scope": "load",
                    "payload": {"num_workers": 2}
                })
                .to_string(),
            ),
            (
                "models/configs/inference-default.json",
                json!({
                    "kind": "backend_config",
                    "label": "Inference Default",
                    "scope": "inference",
                    "payload": {"temperature": 0.7}
                })
                .to_string(),
            ),
            (
                "models/variants/q8_0.json",
                json!({
                    "kind": "variant",
                    "id": "Q8_0",
                    "label": "Q8_0",
                    "format": "gguf",
                    "$load_config": "ref://models/configs/load-default.json"
                })
                .to_string(),
            ),
            (
                "models/presets/default.json",
                json!({
                    "kind": "preset",
                    "id": "default",
                    "label": "Default",
                    "variant_id": "Q8_0",
                    "$inference_config": "ref://models/configs/inference-default.json"
                })
                .to_string(),
            ),
        ]
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
