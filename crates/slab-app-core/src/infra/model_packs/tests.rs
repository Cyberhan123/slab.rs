use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use serde_json::json;
use slab_types::Capability;
use zip::CompressionMethod;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use super::{
    attach_persisted_state_to_pack_bytes, build_generated_model_pack_bytes,
    build_model_command_from_pack_bytes, build_pack_bytes, collect_pack_entries,
    manifest_sha256_from_pack_bytes, read_persisted_model_config_from_pack_bytes,
};
use crate::domain::models::{
    CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION, CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
    ManagedModelBackendId, ModelPackSelection, ModelSpec, RuntimePresets, StoredModelConfig,
    UnifiedModelKind, UnifiedModelStatus,
};
use crate::error::AppCoreError;

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
fn builds_cloud_model_command_from_pack_manifest() {
    let bytes = build_pack(vec![(
        "manifest.json",
        json!({
            "schema_version": 3,
            "deployment": "cloud",
            "id": "gpt_4_1_mini",
            "label": "GPT-4.1 mini",
            "family": "llama",
            "capabilities": ["text_generation", "chat_generation"],
            "context_window": 128000,
            "pricing": {
                "input": 0.4,
                "output": 1.6
            },
            "cloud": {
                "provider_id": "openai-main",
                "remote_model_id": "gpt-4.1-mini"
            }
        })
        .to_string(),
    )]);

    let command = build_model_command_from_pack_bytes(Path::new("gpt-4.1-mini.slab"), &bytes)
        .expect("cloud command");

    assert_eq!(command.id.as_deref(), Some("gpt_4_1_mini"));
    assert_eq!(command.display_name, "GPT-4.1 mini");
    assert_eq!(command.kind, UnifiedModelKind::Cloud);
    assert_eq!(command.backend_id, None);
    assert_eq!(command.status, Some(UnifiedModelStatus::Ready));
    assert_eq!(command.spec.provider_id.as_deref(), Some("openai-main"));
    assert_eq!(command.spec.remote_model_id.as_deref(), Some("gpt-4.1-mini"));
    assert_eq!(command.spec.context_window, Some(128000));
    assert_eq!(command.spec.pricing.as_ref().map(|pricing| pricing.input), Some(0.4));
    assert_eq!(command.spec.pricing.as_ref().map(|pricing| pricing.output), Some(1.6));
    assert!(command.runtime_presets.is_none());
}

#[test]
fn builds_local_model_command_from_pack_manifest() {
    let bytes = build_pack(vec![
        (
            "manifest.json",
            json!({
                "schema_version": 3,
                "deployment": "local",
                "id": "qwen2.5-7b-instruct",
                "label": "Qwen2.5 7B Instruct",
                "family": "llama",
                "context_window": 32768,
                "capabilities": ["text_generation"],
                "engines": [{"id": "ggml.llama", "format": "gguf"}],
                "sources": [{
                    "kind": "hugging_face",
                    "repo_id": "bartowski/Qwen2.5-7B-Instruct-GGUF",
                    "files": [
                        {
                            "id": "model",
                            "path": "Qwen2.5-7B-Instruct-Q4_K_M.gguf"
                        }
                    ]
                }],
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
                "id": "load",
                "label": "Load",
                "scope": "load",
                "payload": {
                    "chat_template": {
                        "id": "chatml-template",
                        "name": "ChatML",
                        "$path": "ref://models/assets/chat_template.jinja"
                    },
                    "num_workers": 2
                }
            })
            .to_string(),
        ),
        (
            "models/assets/chat_template.jinja",
            "{% for message in messages %}<|im_start|>{{ message.role }}\n{{ message.content }}<|im_end|>\n{% endfor %}{% if add_generation_prompt %}<|im_start|>assistant\n{% endif %}".to_owned(),
        ),
        (
            "models/configs/inference.json",
            json!({
                "kind": "backend_config",
                "id": "inference",
                "label": "Inference",
                "scope": "inference",
                "payload": {
                    "temperature": 0.7,
                    "top_p": 0.95
                }
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
    ]);

    let command =
        build_model_command_from_pack_bytes(Path::new("qwen2.5-7b-instruct.slab"), &bytes)
            .expect("local command");

    assert_eq!(command.kind, UnifiedModelKind::Local);
    assert_eq!(command.backend_id, Some(ManagedModelBackendId::GgmlLlama));
    assert_eq!(command.status, Some(UnifiedModelStatus::NotDownloaded));
    assert_eq!(command.spec.repo_id.as_deref(), Some("bartowski/Qwen2.5-7B-Instruct-GGUF"));
    assert_eq!(command.spec.filename.as_deref(), Some("Qwen2.5-7B-Instruct-Q4_K_M.gguf"));
    assert_eq!(command.spec.local_path, None);
    assert_eq!(command.spec.context_window, Some(32768));
    assert_eq!(command.runtime_presets.as_ref().and_then(|presets| presets.temperature), Some(0.7));
    assert_eq!(command.runtime_presets.as_ref().and_then(|presets| presets.top_p), Some(0.95));
}

#[test]
fn builds_diffusion_model_command_from_hugging_face_pack_without_local_paths() {
    let bytes = build_pack(vec![
        (
            "manifest.json",
            json!({
                "schema_version": 3,
                "deployment": "local",
                "id": "sdxl-turbo",
                "label": "SDXL Turbo",
                "family": "diffusion",
                "capabilities": ["image_generation"],
                "engines": [{"id": "ggml.diffusion", "format": "safetensors"}],
                "sources": [{
                    "kind": "hugging_face",
                    "repo_id": "stabilityai/sdxl-turbo",
                    "files": [
                        {
                            "id": "model",
                            "path": "sdxl_turbo.safetensors"
                        }
                    ]
                }],
                "variants": [{"id": "model", "label": "Model", "$ref": "ref://models/variants/model.json"}],
                "presets": [{"id": "default", "label": "Default", "$ref": "ref://models/presets/default.json"}],
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
            "models/variants/model.json",
            json!({
                "kind": "variant",
                "id": "model",
                "label": "Model",
                "format": "safetensors",
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
                "variant_id": "model"
            })
            .to_string(),
        ),
    ]);

    let command = build_model_command_from_pack_bytes(Path::new("sdxl-turbo.slab"), &bytes)
        .expect("diffusion command");

    assert_eq!(command.kind, UnifiedModelKind::Local);
    assert_eq!(command.backend_id, Some(ManagedModelBackendId::GgmlDiffusion));
    assert_eq!(command.status, Some(UnifiedModelStatus::NotDownloaded));
    assert_eq!(command.spec.repo_id.as_deref(), Some("stabilityai/sdxl-turbo"));
    assert_eq!(command.spec.filename.as_deref(), Some("sdxl_turbo.safetensors"));
    assert_eq!(command.spec.local_path, None);
}

#[test]
fn builds_local_model_command_using_selected_variant_file_from_manifest_source() {
    let bytes = build_pack(vec![
        (
            "manifest.json",
            json!({
                "schema_version": 3,
                "deployment": "local",
                "id": "qwen2.5-0.5b-instruct",
                "label": "Qwen2.5 0.5B Instruct",
                "family": "llama",
                "capabilities": ["text_generation"],
                "engines": [{"id": "ggml.llama", "format": "gguf"}],
                "sources": [{
                    "kind": "hugging_face",
                    "repo_id": "bartowski/Qwen2.5-0.5B-Instruct-GGUF",
                    "files": [
                        {
                            "id": "model",
                            "path": "Qwen2.5-0.5B-Instruct-f16.gguf"
                        },
                        {
                            "id": "Q4_K_M",
                            "path": "Qwen2.5-0.5B-Instruct-Q4_K_M.gguf"
                        },
                        {
                            "id": "Q8_0",
                            "path": "Qwen2.5-0.5B-Instruct-Q8_0.gguf"
                        }
                    ]
                }],
                "variants": [{"id": "Q8_0", "label": "Q8_0", "$ref": "ref://models/variants/q8_0.json"}],
                "presets": [{"id": "default", "label": "Default", "$ref": "ref://models/presets/default.json"}],
                "default_preset": "default"
            })
            .to_string(),
        ),
        (
            "models/variants/q8_0.json",
            json!({
                "kind": "variant",
                "id": "Q8_0",
                "label": "Q8_0",
                "format": "gguf"
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

    let command =
        build_model_command_from_pack_bytes(Path::new("qwen2.5-0.5b-instruct.slab"), &bytes)
            .expect("local command");

    assert_eq!(command.kind, UnifiedModelKind::Local);
    assert_eq!(command.backend_id, Some(ManagedModelBackendId::GgmlLlama));
    assert_eq!(command.status, Some(UnifiedModelStatus::NotDownloaded));
    assert_eq!(command.spec.repo_id.as_deref(), Some("bartowski/Qwen2.5-0.5B-Instruct-GGUF"));
    assert_eq!(command.spec.filename.as_deref(), Some("Qwen2.5-0.5B-Instruct-Q8_0.gguf"));
    assert_eq!(command.spec.local_path, None);
}

#[test]
fn builds_local_model_command_using_preset_document_variant() {
    let bytes = build_pack(vec![
        (
            "manifest.json",
            json!({
                "schema_version": 3,
                "deployment": "local",
                "id": "qwen2.5-0.5b-instruct",
                "label": "Qwen2.5 0.5B Instruct",
                "family": "llama",
                "capabilities": ["text_generation"],
                "engines": [{"id": "ggml.llama", "format": "gguf"}],
                "sources": [{
                    "kind": "hugging_face",
                    "repo_id": "bartowski/Qwen2.5-0.5B-Instruct-GGUF",
                    "files": [
                        {
                            "id": "model",
                            "path": "Qwen2.5-0.5B-Instruct-f16.gguf"
                        },
                        {
                            "id": "Q8_0",
                            "path": "Qwen2.5-0.5B-Instruct-Q8_0.gguf"
                        }
                    ]
                }],
                "variants": [{"id": "Q8_0", "label": "Q8_0", "$ref": "ref://models/variants/q8_0.json"}],
                "presets": [{"id": "default", "label": "Default", "$ref": "ref://models/presets/default.json"}],
                "default_preset": "default"
            })
            .to_string(),
        ),
        (
            "models/variants/q8_0.json",
            json!({
                "kind": "variant",
                "id": "Q8_0",
                "label": "Q8_0",
                "format": "gguf"
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

    let command =
        build_model_command_from_pack_bytes(Path::new("qwen2.5-0.5b-instruct.slab"), &bytes)
            .expect("local command");

    assert_eq!(command.spec.filename.as_deref(), Some("Qwen2.5-0.5B-Instruct-Q8_0.gguf"));
}

#[test]
fn manifest_remains_the_source_of_truth_when_persisted_state_matches() {
    let base_bytes = build_pack(vec![(
        "manifest.json",
        json!({
            "schema_version": 3,
            "deployment": "cloud",
            "id": "openrouter-llama-3_1-8b-instruct",
            "label": "Manifest Label",
            "family": "llama",
            "capabilities": ["text_generation"],
            "cloud": {
                "provider_id": "openrouter-main",
                "remote_model_id": "meta-llama/llama-3.1-8b-instruct"
            }
        })
        .to_string(),
    )]);
    let config = StoredModelConfig {
        schema_version: CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
        policy_version: CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION,
        id: "openrouter-llama-3_1-8b-instruct".to_owned(),
        display_name: "Persisted Label".to_owned(),
        kind: UnifiedModelKind::Cloud,
        backend_id: None,
        capabilities: vec![Capability::TextGeneration, Capability::ChatGeneration],
        status: Some(UnifiedModelStatus::Ready),
        spec: ModelSpec {
            provider_id: Some("openrouter-main".to_owned()),
            remote_model_id: Some("meta-llama/llama-3.1-8b-instruct".to_owned()),
            ..Default::default()
        },
        runtime_presets: Some(RuntimePresets {
            temperature: Some(0.2),
            top_p: Some(0.8),
            ..Default::default()
        }),
        materialized_artifacts: BTreeMap::new(),
        pack_selection: None,
        selected_download_source: None,
    };

    let bytes =
        attach_persisted_state_to_pack_bytes(&base_bytes, &config).expect("attach persisted state");
    let command = build_model_command_from_pack_bytes(Path::new("openrouter.slab"), &bytes)
        .expect("command from pack");

    assert_eq!(command.display_name, "Manifest Label");
    assert!(command.runtime_presets.is_none());
}

#[test]
fn ignores_persisted_state_after_manifest_change() {
    let base_bytes = build_pack(vec![(
        "manifest.json",
        json!({
            "schema_version": 3,
            "deployment": "cloud",
            "id": "openrouter-llama-3_1-8b-instruct",
            "label": "Original Manifest Label",
            "family": "llama",
            "capabilities": ["text_generation"],
            "cloud": {
                "provider_id": "openrouter-main",
                "remote_model_id": "meta-llama/llama-3.1-8b-instruct"
            }
        })
        .to_string(),
    )]);
    let config = StoredModelConfig {
        schema_version: CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
        policy_version: CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION,
        id: "openrouter-llama-3_1-8b-instruct".to_owned(),
        display_name: "Persisted Label".to_owned(),
        kind: UnifiedModelKind::Cloud,
        backend_id: None,
        capabilities: vec![Capability::TextGeneration, Capability::ChatGeneration],
        status: Some(UnifiedModelStatus::Ready),
        spec: ModelSpec {
            provider_id: Some("openrouter-main".to_owned()),
            remote_model_id: Some("meta-llama/llama-3.1-8b-instruct".to_owned()),
            ..Default::default()
        },
        runtime_presets: None,
        materialized_artifacts: BTreeMap::new(),
        pack_selection: None,
        selected_download_source: None,
    };

    let bytes =
        attach_persisted_state_to_pack_bytes(&base_bytes, &config).expect("attach persisted state");
    let mut entries = collect_pack_entries(&bytes).expect("collect entries");
    for (path, payload) in &mut entries {
        if path == "manifest.json" {
            *payload = serde_json::to_vec_pretty(&json!({
                "schema_version": 3,
                "deployment": "cloud",
                "id": "openrouter-llama-3_1-8b-instruct",
                "label": "Changed Manifest Label",
                "family": "llama",
                "capabilities": ["text_generation"],
                "cloud": {
                    "provider_id": "openrouter-main",
                    "remote_model_id": "meta-llama/llama-3.1-8b-instruct"
                }
            }))
            .expect("serialize manifest");
        }
    }
    let bytes = build_pack_bytes(entries).expect("rebuild pack");
    let command = build_model_command_from_pack_bytes(Path::new("openrouter.slab"), &bytes)
        .expect("command from pack");

    assert_eq!(command.display_name, "Changed Manifest Label");
}

#[test]
fn generated_pack_carries_persisted_state() {
    let config = StoredModelConfig {
        schema_version: CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
        policy_version: CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION,
        id: "local-qwen".to_owned(),
        display_name: "Local Qwen".to_owned(),
        kind: UnifiedModelKind::Local,
        backend_id: Some(ManagedModelBackendId::GgmlLlama),
        capabilities: vec![Capability::TextGeneration, Capability::ChatGeneration],
        status: Some(UnifiedModelStatus::NotDownloaded),
        spec: ModelSpec {
            repo_id: Some("bartowski/Qwen2.5-7B-Instruct-GGUF".to_owned()),
            filename: Some("Qwen2.5-7B-Instruct-Q4_K_M.gguf".to_owned()),
            context_window: Some(8192),
            ..Default::default()
        },
        runtime_presets: Some(RuntimePresets {
            temperature: Some(0.6),
            top_p: Some(0.9),
            ..Default::default()
        }),
        materialized_artifacts: BTreeMap::new(),
        pack_selection: None,
        selected_download_source: None,
    };

    let bytes = build_generated_model_pack_bytes(&config).expect("generate pack");
    let restored = read_persisted_model_config_from_pack_bytes(&bytes)
        .expect("read state")
        .expect("state exists");
    let command = build_model_command_from_pack_bytes(Path::new("local-qwen.slab"), &bytes)
        .expect("command from pack");

    assert_eq!(restored.id, "local-qwen");
    assert_eq!(restored.schema_version, CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION);
    assert_eq!(restored.policy_version, CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION);
    assert_eq!(command.display_name, "Local Qwen");
    assert_eq!(command.spec.repo_id.as_deref(), Some("bartowski/Qwen2.5-7B-Instruct-GGUF"));
    assert_eq!(command.spec.filename.as_deref(), Some("Qwen2.5-7B-Instruct-Q4_K_M.gguf"));
}

#[test]
fn persisted_state_with_schema_version_one_is_rejected() {
    let base_bytes = build_pack(vec![(
        "manifest.json",
        json!({
            "schema_version": 3,
            "deployment": "cloud",
            "id": "gpt_4_1_mini",
            "label": "GPT-4.1 mini",
            "family": "llama",
            "cloud": {
                "provider_id": "openai-main",
                "remote_model_id": "gpt-4.1-mini"
            }
        })
        .to_string(),
    )]);
    let manifest_sha256 = manifest_sha256_from_pack_bytes(&base_bytes).expect("manifest hash");
    let mut entries = collect_pack_entries(&base_bytes).expect("collect entries");
    entries.push((
        "internal/stored-model-config".to_owned(),
        serde_json::to_vec_pretty(&json!({
            "manifest_sha256": manifest_sha256,
            "config": {
                "schema_version": 1,
                "policy_version": 1,
                "id": "gpt_4_1_mini",
                "display_name": "Persisted GPT-4.1 mini",
                "kind": "cloud",
                "status": "ready",
                "spec": {
                    "provider_id": "openai-main",
                    "remote_model_id": "gpt-4.1-mini",
                    "context_window": 128000
                },
                "runtime_presets": {
                    "temperature": 0.6
                }
            }
        }))
        .expect("serialize state"),
    ));
    let bytes = build_pack_bytes(entries).expect("build pack");

    let error = read_persisted_model_config_from_pack_bytes(&bytes)
        .expect_err("legacy persisted state should require migration");

    assert!(
        matches!(error, AppCoreError::BadRequest(message) if message.contains("unsupported stored model config schema_version"))
    );
}

#[test]
fn future_persisted_state_versions_are_rejected() {
    let base_bytes = build_pack(vec![(
        "manifest.json",
        json!({
            "schema_version": 3,
            "deployment": "cloud",
            "id": "gpt_4_1_mini",
            "label": "GPT-4.1 mini",
            "family": "llama",
            "cloud": {
                "provider_id": "openai-main",
                "remote_model_id": "gpt-4.1-mini"
            }
        })
        .to_string(),
    )]);
    let manifest_sha256 = manifest_sha256_from_pack_bytes(&base_bytes).expect("manifest hash");
    let mut entries = collect_pack_entries(&base_bytes).expect("collect entries");
    entries.push((
        "internal/stored-model-config".to_owned(),
        serde_json::to_vec_pretty(&json!({
            "manifest_sha256": manifest_sha256,
            "config": {
                "schema_version": CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION + 1,
                "policy_version": CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION,
                "id": "gpt_4_1_mini",
                "display_name": "Persisted GPT-4.1 mini",
                "kind": "cloud",
                "status": "ready",
                "spec": {
                    "provider_id": "openai-main",
                    "remote_model_id": "gpt-4.1-mini"
                }
            }
        }))
        .expect("serialize state"),
    ));
    let bytes = build_pack_bytes(entries).expect("build pack");

    let error = read_persisted_model_config_from_pack_bytes(&bytes)
        .expect_err("future version should be rejected");

    assert!(
        matches!(error, AppCoreError::BadRequest(message) if message.contains("unsupported stored model config schema_version"))
    );
}

#[test]
fn persisted_state_preserves_download_projection_without_overriding_pack_selection() {
    let bytes = build_pack(vec![
        (
            "manifest.json",
            json!({
                "schema_version": 3,
                "deployment": "local",
                "id": "local-qwen",
                "label": "Local Qwen",
                "family": "llama",
                "capabilities": ["text_generation", "chat_generation"],
                "engines": [{"id": "ggml.llama", "format": "gguf"}],
                "sources": [{
                    "kind": "hugging_face",
                    "repo_id": "bartowski/Qwen2.5-7B-Instruct-GGUF",
                    "files": [{ "id": "model", "path": "Qwen2.5-7B-Instruct-Q4_K_M.gguf" }]
                }],
                "variants": [{"id": "q4_k_m", "label": "Q4_K_M", "$ref": "ref://models/variants/q4.json"}],
                "presets": [{"id": "default", "label": "Default", "$ref": "ref://models/presets/default.json"}],
                "default_preset": "default"
            })
            .to_string(),
        ),
        (
            "models/variants/q4.json",
            json!({
                "kind": "variant",
                "id": "q4_k_m",
                "label": "Q4_K_M",
                "format": "gguf"
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
    ]);
    let persisted = StoredModelConfig {
        schema_version: CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
        policy_version: CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION,
        id: "local-qwen".to_owned(),
        display_name: "Old Local Qwen".to_owned(),
        kind: UnifiedModelKind::Local,
        backend_id: Some(ManagedModelBackendId::GgmlLlama),
        capabilities: vec![Capability::TextGeneration, Capability::ChatGeneration],
        status: Some(UnifiedModelStatus::Ready),
        spec: ModelSpec {
            repo_id: Some("bartowski/Qwen2.5-7B-Instruct-GGUF".to_owned()),
            filename: Some("Qwen2.5-7B-Instruct-Q4_K_M.gguf".to_owned()),
            local_path: Some("C:/models/Qwen2.5-7B-Instruct-Q4_K_M.gguf".to_owned()),
            ..Default::default()
        },
        runtime_presets: Some(RuntimePresets {
            temperature: Some(0.7),
            top_p: Some(0.95),
            ..Default::default()
        }),
        materialized_artifacts: BTreeMap::new(),
        pack_selection: Some(ModelPackSelection {
            preset_id: Some("default".to_owned()),
            variant_id: Some("q8_0".to_owned()),
        }),
        selected_download_source: None,
    };
    let bytes =
        attach_persisted_state_to_pack_bytes(&bytes, &persisted).expect("attach persisted state");

    let command =
        build_model_command_from_pack_bytes(Path::new("local-qwen.slab"), &bytes).expect("command");

    assert_eq!(command.display_name, "Local Qwen");
    assert_eq!(command.spec.filename.as_deref(), Some("Qwen2.5-7B-Instruct-Q4_K_M.gguf"));
    assert_eq!(
        command.spec.local_path.as_deref(),
        Some("C:/models/Qwen2.5-7B-Instruct-Q4_K_M.gguf")
    );
    assert_eq!(command.status, Some(UnifiedModelStatus::Ready));
}
