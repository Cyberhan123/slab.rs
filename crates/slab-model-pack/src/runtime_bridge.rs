use std::collections::BTreeMap;
use std::path::PathBuf;
use std::str::FromStr;

use serde_json::Value;
use slab_types::{
    Capability, DiffusionLoadOptions, DriverHints, JsonOptions, ModelSource, ModelSpec,
    RuntimeBackendId, RuntimeBackendLoadSpec, RuntimeModelLoadCommand, RuntimeModelLoadSpec,
};

use crate::error::ModelPackError;
use crate::manifest::{BackendConfigDocument, PackSource, PackSourceFile};
use crate::resolve::{ResolvedModelPack, ResolvedPreset};

#[derive(Debug, Clone, Default)]
pub struct ModelPackLoadDefaults {
    pub num_workers: Option<u32>,
    pub context_length: Option<u32>,
    pub chat_template: Option<String>,
    pub diffusion: Option<DiffusionLoadOptions>,
}

#[derive(Debug, Clone)]
pub struct ModelPackRuntimeBridge {
    pub backend: RuntimeBackendId,
    pub capability: Capability,
    pub model_spec: ModelSpec,
    pub load_defaults: ModelPackLoadDefaults,
    pub inference_defaults: JsonOptions,
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
        let capability = self
            .manifest
            .capabilities
            .iter()
            .copied()
            .find(|capability| capability.is_runtime_execution())
            .ok_or(ModelPackError::MissingRuntimeCapability)?;
        let backend = resolve_runtime_backend(&self.manifest.backend_hints, &preset.document.id)?;
        let source = build_model_source(preset)?;
        let load_options = config_payload_as_options(preset.effective_load_config.as_ref())?;
        let inference_defaults =
            config_payload_as_options(preset.effective_inference_config.as_ref())?;
        let load_defaults = build_load_defaults(
            &preset.document.id,
            backend,
            &source,
            preset.effective_load_config.as_ref(),
        )?;
        let metadata = merged_metadata(self, preset);

        let mut model_spec = ModelSpec::new(self.manifest.family, capability, source)
            .named(self.manifest.id.clone())
            .with_driver_hints(self.manifest.backend_hints.clone());
        model_spec.load_options = load_options;
        model_spec.metadata = metadata;

        Ok(ModelPackRuntimeBridge {
            backend,
            capability,
            model_spec,
            load_defaults,
            inference_defaults,
        })
    }
}

impl ModelPackRuntimeBridge {
    pub fn runtime_load_command(
        &self,
        preset_id: &str,
    ) -> Result<RuntimeModelLoadCommand, ModelPackError> {
        let legacy = self.runtime_load_spec(preset_id)?;
        let spec = RuntimeBackendLoadSpec::from_legacy(self.backend, legacy).map_err(|error| {
            ModelPackError::InvalidRuntimeLoadCommand {
                preset_id: preset_id.to_owned(),
                message: error.to_string(),
            }
        })?;

        Ok(RuntimeModelLoadCommand { backend: self.backend, spec })
    }

    pub fn runtime_load_spec(
        &self,
        preset_id: &str,
    ) -> Result<RuntimeModelLoadSpec, ModelPackError> {
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

        Ok(RuntimeModelLoadSpec {
            model_path,
            num_workers: self.load_defaults.num_workers.unwrap_or(1),
            context_length: self.load_defaults.context_length,
            chat_template: self.load_defaults.chat_template.clone(),
            diffusion: self.load_defaults.diffusion.clone(),
        })
    }
}

pub(crate) fn preferred_runtime_backends_from_hints(hints: &DriverHints) -> Vec<RuntimeBackendId> {
    let mut backends = Vec::new();
    for driver in &hints.prefer_drivers {
        let Ok(backend) = RuntimeBackendId::from_str(driver) else {
            continue;
        };
        if !backends.contains(&backend) {
            backends.push(backend);
        }
    }
    backends
}

fn resolve_runtime_backend(
    hints: &DriverHints,
    preset_id: &str,
) -> Result<RuntimeBackendId, ModelPackError> {
    preferred_runtime_backends_from_hints(hints)
        .into_iter()
        .next()
        .ok_or_else(|| ModelPackError::MissingRuntimeBackend { preset_id: preset_id.to_owned() })
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
            PackSource::Cloud { .. } => {
                return Err(ModelPackError::UnsupportedRuntimeBridgeSource {
                    source_kind: "cloud".into(),
                });
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
        PackSource::Cloud { .. } => {
            return Err(ModelPackError::UnsupportedRuntimeBridgeSource {
                source_kind: "cloud".into(),
            });
        }
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
        return Err(ModelPackError::InvalidBackendConfigPayloadShape { id: config.id.clone() });
    };

    Ok(object.iter().map(|(key, value)| (key.clone(), value.clone())).collect())
}

fn build_load_defaults(
    preset_id: &str,
    backend: RuntimeBackendId,
    source: &ModelSource,
    config: Option<&BackendConfigDocument>,
) -> Result<ModelPackLoadDefaults, ModelPackError> {
    let options = config_payload_as_options(config)?;

    Ok(ModelPackLoadDefaults {
        num_workers: options.get("num_workers").and_then(as_u32),
        context_length: options.get("context_length").and_then(as_u32),
        chat_template: options.get("chat_template").and_then(as_string),
        diffusion: (backend == RuntimeBackendId::GgmlDiffusion)
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
        flash_attn: options.get("flash_attn").and_then(Value::as_bool).unwrap_or(false),
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

#[cfg(test)]
mod tests {
    use std::io::Write;

    use serde_json::{Value, json};
    use zip::CompressionMethod;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

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
    fn compiles_default_runtime_bridge_for_llama() {
        let bytes = build_pack(vec![
            ("manifest.json", json!({
                "version": 2,
                "id": "qwen2.5-7b-instruct",
                "label": "Qwen2.5 7B Instruct",
                "family": "llama",
                "capabilities": ["text_generation"],
                "backend_hints": {"prefer_drivers": ["ggml.llama"], "avoid_drivers": [], "require_streaming": true},
                "components": [{"id": "model", "label": "Model", "$config": "ref://models/components/model.json"}],
                "variants": [{"id": "q4_k_m", "label": "Q4_K_M", "$config": "ref://models/variants/q4.json"}],
                "presets": [{"id": "default", "label": "Default", "$config": "ref://models/presets/default.json"}],
                "default_preset": "default"
            }).to_string()),
            ("models/components/model.json", json!({
                "kind": "component", "id": "model", "label": "Model",
                "source": {"kind": "local_path", "path": "C:/models/qwen.gguf"}
            }).to_string()),
            ("models/configs/load.json", json!({
                "kind": "backend_config", "id": "load", "label": "Load", "scope": "load",
                "payload": {"context_length": 8192, "chat_template": "chatml", "num_workers": 2}
            }).to_string()),
            ("models/configs/inference.json", json!({
                "kind": "backend_config", "id": "inference", "label": "Inference", "scope": "inference",
                "payload": {"temperature": 0.7, "top_p": 0.95}
            }).to_string()),
            ("models/variants/q4.json", json!({
                "kind": "variant", "id": "q4_k_m", "label": "Q4", "component_ids": ["model"]
            }).to_string()),
            ("models/presets/default.json", json!({
                "kind": "preset", "id": "default", "label": "Default",
                "$load_config": "ref://models/configs/load.json", "$inference_config": "ref://models/configs/inference.json"
            }).to_string()),
        ]);

        let pack = ModelPack::from_bytes(&bytes).expect("load pack");
        let resolved = pack.resolve().expect("resolve pack");
        let bridge = resolved.compile_default_runtime_bridge().expect("compile bridge");
        let load_spec = bridge.runtime_load_spec("default").expect("load spec");

        assert_eq!(bridge.backend.canonical_id(), "ggml.llama");
        assert_eq!(
            bridge
                .model_spec
                .source
                .primary_path()
                .map(|path| path.to_string_lossy().to_string())
                .as_deref(),
            Some("C:/models/qwen.gguf")
        );
        assert_eq!(load_spec.context_length, Some(8192));
        assert_eq!(bridge.inference_defaults.get("temperature").and_then(Value::as_f64), Some(0.7));
    }

    #[test]
    fn rejects_cloud_source_when_building_runtime_bridge() {
        let bytes = build_pack(vec![
            (
                "manifest.json",
                json!({
                    "version": 2,
                    "id": "gpt-4.1-mini",
                    "label": "GPT-4.1 mini",
                    "family": "llama",
                    "capabilities": ["text_generation"],
                    "backend_hints": {"prefer_drivers": ["ggml.llama"], "avoid_drivers": [], "require_streaming": true},
                    "source": {
                        "kind": "cloud",
                        "provider_id": "openai-main",
                        "remote_model_id": "gpt-4.1-mini"
                    },
                    "variants": [{"id": "default-variant", "label": "Default Variant", "$config": "ref://models/variants/default.json"}],
                    "presets": [{"id": "default", "label": "Default", "$config": "ref://models/presets/default.json"}],
                    "default_preset": "default"
                })
                .to_string(),
            ),
            (
                "models/variants/default.json",
                json!({
                    "kind": "variant",
                    "id": "default-variant",
                    "label": "Default Variant"
                })
                .to_string(),
            ),
            (
                "models/presets/default.json",
                json!({
                    "kind": "preset",
                    "id": "default",
                    "label": "Default",
                    "variant_id": "default-variant"
                })
                .to_string(),
            ),
        ]);

        let pack = ModelPack::from_bytes(&bytes).expect("load pack");
        let resolved = pack.resolve().expect("resolve pack");
        let error = resolved
            .compile_default_runtime_bridge()
            .expect_err("cloud source must not compile runtime bridge");

        assert!(error.to_string().contains("source kind 'cloud'"));
    }

    #[test]
    fn compiles_diffusion_bridge_for_hugging_face_source_without_materialized_paths() {
        let bytes = build_pack(vec![
            (
                "manifest.json",
                json!({
                    "version": 2,
                    "id": "sdxl-turbo",
                    "label": "SDXL Turbo",
                    "family": "diffusion",
                    "capabilities": ["image_generation"],
                    "backend_hints": {"prefer_drivers": ["ggml.diffusion"], "avoid_drivers": [], "require_streaming": false},
                    "source": {
                        "kind": "hugging_face",
                        "repo_id": "stabilityai/sdxl-turbo",
                        "files": [
                            {"id": "diffusion_model", "path": "sdxl_turbo.safetensors"},
                            {"id": "vae", "path": "vae.safetensors"}
                        ]
                    },
                    "presets": [{"id": "default", "label": "Default", "$config": "ref://models/presets/default.json"}],
                    "default_preset": "default"
                })
                .to_string(),
            ),
            (
                "models/configs/load.json",
                json!({
                    "kind": "backend_config",
                    "id": "load",
                    "label": "Load",
                    "scope": "load",
                    "payload": {
                        "flash_attn": true,
                        "vae_device": "cpu"
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

        let pack = ModelPack::from_bytes(&bytes).expect("load pack");
        let resolved = pack.resolve().expect("resolve pack");
        let bridge = resolved.compile_default_runtime_bridge().expect("compile bridge");
        let diffusion =
            bridge.load_defaults.diffusion.as_ref().expect("diffusion defaults should exist");
        let load_error =
            bridge.runtime_load_spec("default").expect_err("load spec should require download");

        assert_eq!(bridge.backend.canonical_id(), "ggml.diffusion");
        assert!(diffusion.diffusion_model_path.is_none());
        assert!(diffusion.vae_path.is_none());
        assert!(diffusion.clip_l_path.is_none());
        assert!(diffusion.flash_attn);
        assert_eq!(diffusion.vae_device, "cpu");
        assert!(load_error.to_string().contains("hugging_face"));
    }

    #[test]
    fn compiles_hugging_face_bridge_using_selected_variant_file() {
        let bytes = build_pack(vec![
            (
                "manifest.json",
                json!({
                    "version": 2,
                    "id": "qwen2.5-0.5b-instruct",
                    "label": "Qwen2.5 0.5B Instruct",
                    "family": "llama",
                    "capabilities": ["text_generation"],
                    "backend_hints": {"prefer_drivers": ["ggml.llama"], "avoid_drivers": [], "require_streaming": false},
                    "source": {
                        "kind": "hugging_face",
                        "repo_id": "bartowski/Qwen2.5-0.5B-Instruct-GGUF",
                        "files": [
                            {"id": "model", "path": "Qwen2.5-0.5B-Instruct-f16.gguf"},
                            {"id": "Q8_0", "path": "Qwen2.5-0.5B-Instruct-Q8_0.gguf"}
                        ]
                    },
                    "variants": [{"id": "Q8_0", "label": "Q8_0", "$config": "ref://models/variants/q8_0.json"}],
                    "presets": [{"id": "default", "label": "Default", "$config": "ref://models/presets/default.json"}],
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

        let pack = ModelPack::from_bytes(&bytes).expect("load pack");
        let resolved = pack.resolve().expect("resolve pack");
        let bridge = resolved.compile_default_runtime_bridge().expect("compile bridge");

        assert_eq!(
            bridge
                .model_spec
                .source
                .artifact("model")
                .map(|path| path.to_string_lossy().to_string())
                .as_deref(),
            Some("Qwen2.5-0.5B-Instruct-Q8_0.gguf")
        );
        assert_eq!(
            bridge
                .model_spec
                .source
                .primary_path()
                .map(|path| path.to_string_lossy().to_string())
                .as_deref(),
            Some("Qwen2.5-0.5B-Instruct-Q8_0.gguf")
        );
    }
}
