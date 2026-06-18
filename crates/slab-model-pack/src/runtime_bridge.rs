use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;
use slab_types::{
    CandleDiffusionLoadConfig, CandleLlamaLoadConfig, CandleWhisperLoadConfig, Capability,
    DiffusionLoadOptions, DriverHints, GbnfAssetRef, GgmlDiffusionLoadConfig, GgmlLlamaLoadConfig,
    GgmlWhisperLoadConfig, JsonOptions, ModelSource, ModelSpec, OnnxLoadConfig, RuntimeBackendId,
    RuntimeBackendLoadSpec, RuntimeModelLoadCommand, TemplateAssetRef,
};

use crate::error::ModelPackError;
use crate::manifest::{BackendConfigDocument, PackDeployment, PackSource, PackSourceFile};
use crate::refs::ConfigRef;
use crate::resolve::{ResolvedModelPack, ResolvedPreset};

#[derive(Debug, Clone, Default)]
pub struct ModelPackLoadDefaults {
    pub num_workers: Option<u32>,
    pub context_length: Option<u32>,
    pub chat_template: Option<TemplateAssetRef>,
    pub chat_template_source: Option<String>,
    pub gbnf: Option<GbnfAssetRef>,
    pub gbnf_source: Option<String>,
    pub diffusion: Option<DiffusionLoadOptions>,
}

#[derive(Debug, Clone)]
pub struct ModelPackRuntimeBridge {
    pub backend: RuntimeBackendId,
    pub capability: Capability,
    pub model_spec: ModelSpec,
    pub load_defaults: ModelPackLoadDefaults,
    pub inference_defaults: JsonOptions,
    pub engine_load_specs: Vec<ModelPackEngineLoadSpec>,
}

#[derive(Debug, Clone)]
pub struct ModelPackEngineLoadSpec {
    pub backend: RuntimeBackendId,
    pub model_spec: ModelSpec,
    pub load_defaults: ModelPackLoadDefaults,
}

impl ResolvedModelPack {
    pub fn compile_default_runtime_bridge(&self) -> Result<ModelPackRuntimeBridge, ModelPackError> {
        let preset =
            self.default_preset().ok_or(ModelPackError::MissingDefaultPresetDeclaration)?;
        self.compile_runtime_bridge(preset)
    }

    pub fn compile_model_source(
        &self,
        preset: &ResolvedPreset,
    ) -> Result<ModelSource, ModelPackError> {
        build_model_source(preset)
    }

    pub fn compile_runtime_bridge(
        &self,
        preset: &ResolvedPreset,
    ) -> Result<ModelPackRuntimeBridge, ModelPackError> {
        if self.manifest.deployment == PackDeployment::Cloud {
            return Err(ModelPackError::UnsupportedRuntimeBridgeSource {
                source_kind: "cloud".into(),
            });
        }

        let capability = self
            .manifest
            .capabilities
            .iter()
            .copied()
            .find(|capability| capability.is_runtime_execution())
            .ok_or(ModelPackError::MissingRuntimeCapability)?;
        let source = build_model_source(preset)?;
        let load_config = preset.variant.load_config.as_ref();
        let load_options = config_payload_as_options(load_config)?;
        let inference_defaults =
            config_payload_as_options(preset.effective_inference_config.as_ref())?;
        let metadata = merged_metadata(self, preset);
        let mut engine_load_specs = Vec::new();

        for engine in &preset.engine_candidates {
            let backend = engine.id;
            let load_defaults =
                build_load_defaults(self, &preset.document.id, backend, &source, load_config)?;
            let mut model_spec = ModelSpec::new(self.manifest.family, capability, source.clone())
                .named(self.manifest.id.clone())
                .with_driver_hints(DriverHints {
                    prefer_drivers: vec![backend.canonical_id().to_owned()],
                    avoid_drivers: Vec::new(),
                    require_streaming: false,
                });
            model_spec.load_options = load_options.clone();
            model_spec.metadata = metadata.clone();
            engine_load_specs.push(ModelPackEngineLoadSpec { backend, model_spec, load_defaults });
        }

        let primary = engine_load_specs.first().cloned().ok_or_else(|| {
            ModelPackError::MissingCompatibleEngines {
                preset_id: preset.document.id.clone(),
                variant_id: preset.variant.document.id.clone(),
            }
        })?;

        Ok(ModelPackRuntimeBridge {
            backend: primary.backend,
            capability,
            model_spec: primary.model_spec,
            load_defaults: primary.load_defaults,
            inference_defaults,
            engine_load_specs,
        })
    }
}

impl ModelPackRuntimeBridge {
    pub fn runtime_load_command(
        &self,
        preset_id: &str,
    ) -> Result<RuntimeModelLoadCommand, ModelPackError> {
        self.engine_load_specs
            .first()
            .ok_or_else(|| ModelPackError::MissingRuntimeBackend { preset_id: preset_id.into() })?
            .runtime_load_command(preset_id)
    }

    pub fn runtime_load_spec(
        &self,
        preset_id: &str,
    ) -> Result<RuntimeBackendLoadSpec, ModelPackError> {
        self.engine_load_specs
            .first()
            .ok_or_else(|| ModelPackError::MissingRuntimeBackend { preset_id: preset_id.into() })?
            .runtime_load_spec(preset_id)
    }
}

impl ModelPackEngineLoadSpec {
    pub fn runtime_load_command(
        &self,
        preset_id: &str,
    ) -> Result<RuntimeModelLoadCommand, ModelPackError> {
        let spec = self.runtime_load_spec(preset_id)?;
        Ok(RuntimeModelLoadCommand { backend: self.backend, spec })
    }

    pub fn runtime_load_spec(
        &self,
        preset_id: &str,
    ) -> Result<RuntimeBackendLoadSpec, ModelPackError> {
        let model_path = match &self.model_spec.source {
            ModelSource::LocalPath { path } => path.clone(),
            ModelSource::LocalArtifacts { files } => files
                .get("model")
                .or_else(|| files.get("diffusion_model"))
                .or_else(|| files.values().next())
                .cloned()
                .ok_or_else(|| ModelPackError::MissingPrimaryArtifact {
                    preset_id: preset_id.to_owned(),
                })?,
            ModelSource::HuggingFace { .. } => {
                return Err(ModelPackError::NonMaterializedSource {
                    preset_id: preset_id.to_owned(),
                    source_kind: "hugging_face".into(),
                });
            }
            _ => {
                return Err(ModelPackError::NonMaterializedSource {
                    preset_id: preset_id.to_owned(),
                    source_kind: "unknown".into(),
                });
            }
        };

        Ok(match self.backend {
            RuntimeBackendId::GgmlLlama => RuntimeBackendLoadSpec::GgmlLlama(GgmlLlamaLoadConfig {
                model_path,
                num_workers: usize::try_from(self.load_defaults.num_workers.unwrap_or(1)).map_err(
                    |error| ModelPackError::InvalidRuntimeLoadCommand {
                        preset_id: preset_id.to_owned(),
                        message: error.to_string(),
                    },
                )?,
                context_length: self.load_defaults.context_length,
                flash_attn: true,
                chat_template: self.load_defaults.chat_template_source.clone(),
                gbnf: self.load_defaults.gbnf_source.clone(),
            }),
            RuntimeBackendId::GgmlWhisper => {
                RuntimeBackendLoadSpec::GgmlWhisper(GgmlWhisperLoadConfig {
                    model_path,
                    flash_attn: true,
                })
            }
            RuntimeBackendId::GgmlDiffusion => {
                let diffusion = self.load_defaults.diffusion.clone().unwrap_or_default();
                RuntimeBackendLoadSpec::GgmlDiffusion(Box::new(GgmlDiffusionLoadConfig {
                    model_path,
                    diffusion_model_path: diffusion.diffusion_model_path,
                    vae_path: diffusion.vae_path,
                    taesd_path: diffusion.taesd_path,
                    clip_l_path: diffusion.clip_l_path,
                    clip_g_path: diffusion.clip_g_path,
                    t5xxl_path: diffusion.t5xxl_path,
                    clip_vision_path: None,
                    control_net_path: None,
                    flash_attn: diffusion.flash_attn,
                    vae_device: (!diffusion.vae_device.is_empty()).then_some(diffusion.vae_device),
                    clip_device: (!diffusion.clip_device.is_empty())
                        .then_some(diffusion.clip_device),
                    offload_params_to_cpu: diffusion.offload_params_to_cpu,
                    enable_mmap: false,
                    n_threads: None,
                }))
            }
            RuntimeBackendId::CandleLlama => {
                RuntimeBackendLoadSpec::CandleLlama(CandleLlamaLoadConfig {
                    model_path,
                    tokenizer_path: None,
                    device: None,
                    seed: 0,
                })
            }
            RuntimeBackendId::CandleWhisper => {
                RuntimeBackendLoadSpec::CandleWhisper(CandleWhisperLoadConfig {
                    model_path,
                    tokenizer_path: None,
                    device: None,
                })
            }
            RuntimeBackendId::CandleDiffusion => {
                let diffusion = self.load_defaults.diffusion.clone().unwrap_or_default();
                RuntimeBackendLoadSpec::CandleDiffusion(CandleDiffusionLoadConfig {
                    model_path,
                    vae_path: diffusion.vae_path,
                    device: None,
                    sd_version: "v2-1".to_owned(),
                })
            }
            RuntimeBackendId::Onnx => RuntimeBackendLoadSpec::Onnx(OnnxLoadConfig {
                model_path,
                execution_providers: vec!["CPU".to_owned()],
                intra_op_num_threads: None,
                inter_op_num_threads: None,
            }),
            backend => {
                return Err(ModelPackError::InvalidRuntimeLoadCommand {
                    preset_id: preset_id.to_owned(),
                    message: format!(
                        "runtime backend '{}' is not supported by model pack runtime bridge",
                        backend.canonical_id()
                    ),
                });
            }
        })
    }
}

fn build_model_source(preset: &ResolvedPreset) -> Result<ModelSource, ModelPackError> {
    if !preset.variant.components.is_empty() {
        return build_model_source_from_components(preset);
    }

    let Some(source) = preset.variant.effective_sources.first() else {
        return Err(ModelPackError::MissingPrimaryArtifact {
            preset_id: preset.document.id.clone(),
        });
    };

    pack_source_to_model_source(&source.source)
}

fn build_model_source_from_components(
    preset: &ResolvedPreset,
) -> Result<ModelSource, ModelPackError> {
    let mut local_files = BTreeMap::new();
    let mut hf_repo_id: Option<String> = None;
    let mut hf_revision: Option<String> = None;
    let mut saw_hf = false;
    let mut saw_local = false;

    for (component_id, component) in &preset.variant.components {
        match &component.document.source {
            PackSource::LocalPath { path } => {
                saw_local = true;
                local_files.insert(component_id.clone(), PathBuf::from(path));
            }
            PackSource::LocalFiles { files } => {
                saw_local = true;
                for (key, path) in component_files_to_entries(component_id, files) {
                    local_files.insert(key, path);
                }
            }
            PackSource::HuggingFace { repo_id, revision, files }
            | PackSource::ModelScope { repo_id, revision, files } => {
                saw_hf = true;
                if let Some(existing) = &hf_repo_id {
                    if existing != repo_id || hf_revision.as_ref() != revision.as_ref() {
                        return Err(ModelPackError::ConflictingRuntimeBackend {
                            preset_id: preset.document.id.clone(),
                        });
                    }
                } else {
                    hf_repo_id = Some(repo_id.clone());
                    hf_revision = revision.clone();
                }
                for (key, path) in component_files_to_entries(component_id, files) {
                    local_files.insert(key, path);
                }
            }
        }
    }

    if saw_hf && saw_local {
        return Err(ModelPackError::NonMaterializedSource {
            preset_id: preset.document.id.clone(),
            source_kind: "mixed".into(),
        });
    }

    if saw_hf {
        return Ok(ModelSource::HuggingFace {
            repo_id: hf_repo_id.expect("repo id set when saw_hf"),
            revision: hf_revision,
            files: local_files,
        });
    }

    if local_files.len() == 1 {
        let (_, path) = local_files.into_iter().next().expect("one local file exists");
        return Ok(ModelSource::LocalPath { path });
    }

    Ok(ModelSource::LocalArtifacts { files: local_files })
}

fn pack_source_to_model_source(source: &PackSource) -> Result<ModelSource, ModelPackError> {
    Ok(match source {
        PackSource::LocalPath { path } => ModelSource::LocalPath { path: PathBuf::from(path) },
        PackSource::LocalFiles { files } => {
            ModelSource::LocalArtifacts { files: source_file_map(None, files) }
        }
        PackSource::HuggingFace { repo_id, revision, files }
        | PackSource::ModelScope { repo_id, revision, files } => ModelSource::HuggingFace {
            repo_id: repo_id.clone(),
            revision: revision.clone(),
            files: source_file_map(None, files),
        },
    })
}

fn source_file_map(prefix: Option<&str>, files: &[PackSourceFile]) -> BTreeMap<String, PathBuf> {
    component_files_to_entries(prefix.unwrap_or("model"), files)
}

fn component_files_to_entries(
    component_id: &str,
    files: &[PackSourceFile],
) -> BTreeMap<String, PathBuf> {
    let use_component_id = files.len() == 1;
    files
        .iter()
        .map(|file| {
            let key = if use_component_id {
                component_id.to_owned()
            } else {
                format!("{component_id}/{}", file.id)
            };
            (key, PathBuf::from(&file.path))
        })
        .collect()
}

fn config_payload_as_options(
    config: Option<&BackendConfigDocument>,
) -> Result<JsonOptions, ModelPackError> {
    let Some(config) = config else {
        return Ok(JsonOptions::default());
    };
    let Value::Object(object) = &config.payload else {
        return Err(ModelPackError::InvalidBackendConfigPayloadShape { id: config_id(config) });
    };

    Ok(object.iter().map(|(key, value)| (key.clone(), value.clone())).collect())
}

fn config_id(config: &BackendConfigDocument) -> String {
    config.id.clone().unwrap_or_else(|| config.label.clone())
}

fn build_load_defaults(
    resolved: &ResolvedModelPack,
    preset_id: &str,
    backend: RuntimeBackendId,
    source: &ModelSource,
    config: Option<&BackendConfigDocument>,
) -> Result<ModelPackLoadDefaults, ModelPackError> {
    let options = config_payload_as_options(config)?;
    let config_id = config.map(config_id).unwrap_or_else(|| preset_id.to_owned());
    reject_legacy_llama_load_fields(backend, &config_id, &options)?;
    let chat_template = parse_optional_asset_ref(&options, "chat_template", &config_id)?;
    let gbnf = parse_optional_asset_ref(&options, "gbnf", &config_id)?;

    Ok(ModelPackLoadDefaults {
        num_workers: options.get("num_workers").and_then(as_u32),
        context_length: resolved
            .manifest
            .context_window
            .or_else(|| options.get("context_length").and_then(as_u32)),
        chat_template_source: resolve_text_asset_ref(
            resolved,
            &config_id,
            "chat_template",
            chat_template.as_ref(),
        )?,
        chat_template,
        gbnf_source: resolve_text_asset_ref(resolved, &config_id, "gbnf", gbnf.as_ref())?,
        gbnf,
        diffusion: matches!(
            backend,
            RuntimeBackendId::GgmlDiffusion | RuntimeBackendId::CandleDiffusion
        )
        .then(|| build_diffusion_load_defaults(preset_id, source, &options))
        .transpose()?,
    })
}

fn build_diffusion_load_defaults(
    _preset_id: &str,
    source: &ModelSource,
    options: &JsonOptions,
) -> Result<DiffusionLoadOptions, ModelPackError> {
    let materialized_source = match source {
        ModelSource::HuggingFace { .. } => None,
        other => Some(other),
    };

    Ok(DiffusionLoadOptions {
        diffusion_model_path: materialized_source.and_then(|source| {
            artifact_path(source, "diffusion_model").or_else(|| artifact_path(source, "model"))
        }),
        vae_path: materialized_source.and_then(|source| artifact_path(source, "vae")),
        taesd_path: materialized_source.and_then(|source| artifact_path(source, "taesd")),
        lora_model_dir: options.get("lora_model_dir").and_then(as_string).map(PathBuf::from),
        clip_l_path: materialized_source.and_then(|source| artifact_path(source, "clip_l")),
        clip_g_path: materialized_source.and_then(|source| artifact_path(source, "clip_g")),
        t5xxl_path: materialized_source.and_then(|source| artifact_path(source, "t5xxl")),
        flash_attn: options.get("flash_attn").and_then(Value::as_bool).unwrap_or(true),
        vae_device: options.get("vae_device").and_then(as_string).unwrap_or_default(),
        clip_device: options.get("clip_device").and_then(as_string).unwrap_or_default(),
        offload_params_to_cpu: options
            .get("offload_params_to_cpu")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn artifact_path(source: &ModelSource, key: &str) -> Option<PathBuf> {
    source.artifact(key).map(PathBuf::from)
}

fn merged_metadata(pack: &ResolvedModelPack, preset: &ResolvedPreset) -> BTreeMap<String, String> {
    let mut metadata = pack.manifest.metadata.clone();
    for (key, value) in &preset.variant.document.metadata {
        metadata.insert(key.clone(), value.clone());
    }
    for (key, value) in &preset.document.metadata {
        metadata.insert(key.clone(), value.clone());
    }
    metadata.insert("default_preset".into(), preset.document.id.clone());
    metadata
}

fn as_u32(value: &Value) -> Option<u32> {
    value.as_u64().and_then(|value| u32::try_from(value).ok())
}

fn as_string(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned).filter(|value| !value.trim().is_empty())
}

fn parse_optional_asset_ref(
    options: &JsonOptions,
    field: &str,
    config_id: &str,
) -> Result<Option<slab_types::AssetRef>, ModelPackError> {
    let Some(value) = options.get(field) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let asset_ref: slab_types::AssetRef =
        serde_json::from_value(value.clone()).map_err(|error| {
            ModelPackError::InvalidBackendConfigAssetRef {
                id: config_id.to_owned(),
                field: field.to_owned(),
                message: error.to_string(),
            }
        })?;
    asset_ref.validate_configured(field).map_err(|error| {
        ModelPackError::InvalidBackendConfigAssetRef {
            id: config_id.to_owned(),
            field: field.to_owned(),
            message: error.to_string(),
        }
    })
}

fn resolve_text_asset_ref(
    resolved: &ResolvedModelPack,
    config_id: &str,
    field: &str,
    asset_ref: Option<&slab_types::AssetRef>,
) -> Result<Option<String>, ModelPackError> {
    let Some(asset_ref) = asset_ref else {
        return Ok(None);
    };
    let path = asset_ref.path.as_deref().expect("validated asset ref always has a path");
    let config_ref = ConfigRef::parse(path.to_owned()).map_err(|error| {
        ModelPackError::InvalidBackendConfigAssetRef {
            id: config_id.to_owned(),
            field: field.to_owned(),
            message: error.to_string(),
        }
    })?;
    resolved.text_asset(&config_ref).map(str::to_owned).map(Some).map_err(|_| {
        ModelPackError::MissingBackendConfigAsset {
            id: config_id.to_owned(),
            field: field.to_owned(),
            path: config_ref.path().to_owned(),
        }
    })
}

fn reject_legacy_llama_load_fields(
    backend: RuntimeBackendId,
    config_id: &str,
    options: &JsonOptions,
) -> Result<(), ModelPackError> {
    if backend != RuntimeBackendId::GgmlLlama {
        return Ok(());
    }

    for (field, message) in [
        ("grammar", "legacy load payload field removed; use 'gbnf' instead"),
        (
            "grammar_json",
            "legacy boolean grammar flags were removed; compile structured output to GBNF instead",
        ),
        (
            "grammar_tool_call",
            "legacy boolean grammar flags were removed; compile structured output to GBNF instead",
        ),
        (
            "apply_chat_template",
            "legacy runtime chat-template application was removed; configure 'chat_template' asset refs instead",
        ),
    ] {
        if options.contains_key(field) {
            return Err(ModelPackError::UnsupportedBackendConfigField {
                id: config_id.to_owned(),
                field: field.to_owned(),
                message: message.to_owned(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod v3_tests {
    use std::io::Write;

    use serde_json::{Value, json};
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    use crate::pack::ModelPack;

    #[test]
    fn compiles_default_runtime_bridge_for_llama_v3() {
        let pack = ModelPack::from_bytes(&build_pack(local_pack_entries())).expect("load pack");
        let resolved = pack.resolve().expect("resolve pack");
        let bridge = resolved.compile_default_runtime_bridge().expect("compile bridge");

        assert_eq!(bridge.backend.canonical_id(), "ggml.llama");
        assert_eq!(bridge.engine_load_specs.len(), 1);
        assert_eq!(
            bridge
                .model_spec
                .source
                .primary_path()
                .map(|path| path.to_string_lossy().to_string())
                .as_deref(),
            Some("C:/models/qwen.gguf")
        );
        assert_eq!(bridge.load_defaults.context_length, Some(8192));
        assert_eq!(bridge.inference_defaults.get("temperature").and_then(Value::as_f64), Some(0.7));
    }

    #[test]
    fn cloud_pack_does_not_compile_runtime_bridge() {
        let pack = ModelPack::from_bytes(&build_pack(vec![(
            "manifest.json",
            json!({
                "schema_version": 3,
                "deployment": "cloud",
                "id": "gpt-4.1-mini",
                "label": "GPT-4.1 mini",
                "family": "llama",
                "capabilities": ["text_generation"],
                "cloud": {
                    "provider_id": "openai-main",
                    "remote_model_id": "gpt-4.1-mini"
                }
            })
            .to_string(),
        )]))
        .expect("load pack");
        let resolved = pack.resolve().expect("resolve pack");
        let error = resolved
            .compile_default_runtime_bridge()
            .expect_err("cloud pack must not compile runtime bridge");

        assert!(error.to_string().contains("default_preset"));
    }

    #[test]
    fn rejects_legacy_grammar_field_in_llama_load_payload() {
        let mut entries = local_pack_entries();
        entries[1].1 = json!({
            "kind": "backend_config",
            "label": "Load",
            "scope": "load",
            "payload": {
                "grammar": "root ::= \"ok\""
            }
        })
        .to_string();

        let pack = ModelPack::from_bytes(&build_pack(entries)).expect("load pack");
        let resolved = pack.resolve().expect("resolve pack");
        let error =
            resolved.compile_default_runtime_bridge().expect_err("legacy grammar field must fail");

        assert!(error.to_string().contains("use 'gbnf' instead"));
    }

    fn local_pack_entries() -> Vec<(&'static str, String)> {
        vec![
            (
                "manifest.json",
                json!({
                    "schema_version": 3,
                    "deployment": "local",
                    "id": "qwen2.5-7b-instruct",
                    "label": "Qwen2.5 7B Instruct",
                    "family": "llama",
                    "capabilities": ["text_generation"],
                    "context_window": 8192,
                    "engines": [{"id": "ggml.llama", "format": "gguf"}],
                    "variants": [{"id": "q4_k_m", "label": "Q4_K_M", "$ref": "ref://models/variants/q4.json"}],
                    "presets": [{"id": "default", "label": "Default", "$ref": "ref://models/presets/default.json"}],
                    "default_preset": "default"
                })
                .to_string(),
            ),
            (
                "models/configs/load.json",
                json!({
                    "kind": "backend_config",
                    "label": "Load",
                    "scope": "load",
                    "payload": {"num_workers": 2}
                })
                .to_string(),
            ),
            (
                "models/configs/inference.json",
                json!({
                    "kind": "backend_config",
                    "label": "Inference",
                    "scope": "inference",
                    "payload": {"temperature": 0.7}
                })
                .to_string(),
            ),
            (
                "models/variants/q4.json",
                json!({
                    "kind": "variant",
                    "id": "q4_k_m",
                    "label": "Q4",
                    "format": "gguf",
                    "sources": [{"kind": "local_path", "path": "C:/models/qwen.gguf"}],
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
