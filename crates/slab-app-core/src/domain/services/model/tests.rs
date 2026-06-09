use std::collections::BTreeMap;

use chrono::Utc;
use slab_hub::HubErrorKind;
use slab_types::{Capability, DriverHints, ModelFamily, RuntimeBackendId};

use crate::domain::models::{
    ChatModelSource, ModelSpec, RuntimePresets, UnifiedModel, UnifiedModelKind, UnifiedModelStatus,
    default_model_capabilities,
};
use crate::error::AppCoreError;

use super::catalog::{
    build_cloud_chat_model_option, build_local_chat_model_option, canonicalize_model_spec,
    canonicalize_runtime_presets, map_hub_client_error, normalize_required_text,
};
use super::pack::build_local_model_command_from_pack_preset;
use super::runtime::validate_and_normalize_model_workers;

#[test]
fn cloud_models_require_provider_reference() {
    let error = canonicalize_model_spec(UnifiedModelKind::Cloud, None, ModelSpec::default())
        .expect_err("missing cloud fields");

    assert!(
        error.to_string().contains(
            "cloud models must set spec.provider_id to a configured providers.registry entry"
        ),
        "unexpected error: {error}"
    );
}

#[test]
fn cloud_models_require_remote_model_id() {
    let error = canonicalize_model_spec(
        UnifiedModelKind::Cloud,
        None,
        ModelSpec { provider_id: Some("openai-main".into()), ..ModelSpec::default() },
    )
    .expect_err("missing remote_model_id");

    assert!(
        error.to_string().contains("cloud models must set spec.remote_model_id"),
        "unexpected error: {error}"
    );
}

#[test]
fn cloud_models_trim_provider_and_remote_model() {
    let (_, spec) = canonicalize_model_spec(
        UnifiedModelKind::Cloud,
        None,
        ModelSpec {
            provider_id: Some(" openai-main ".into()),
            remote_model_id: Some(" gpt-4.1-mini ".into()),
            ..ModelSpec::default()
        },
    )
    .expect("cloud spec");

    assert_eq!(spec.provider_id.as_deref(), Some("openai-main"));
    assert_eq!(spec.remote_model_id.as_deref(), Some("gpt-4.1-mini"));
}

#[test]
fn cloud_models_clear_local_only_fields() {
    let (_, spec) = canonicalize_model_spec(
        UnifiedModelKind::Cloud,
        None,
        ModelSpec {
            provider_id: Some("openai-main".into()),
            remote_model_id: Some("gpt-4.1-mini".into()),
            repo_id: Some("Qwen/Qwen3-8B-GGUF".into()),
            hub_provider: Some("hf".into()),
            filename: Some("qwen3-8b.gguf".into()),
            local_path: Some("C:/models/qwen3-8b.gguf".into()),
            ..ModelSpec::default()
        },
    )
    .expect("cloud spec");

    assert!(spec.repo_id.is_none());
    assert!(spec.hub_provider.is_none());
    assert!(spec.filename.is_none());
    assert!(spec.local_path.is_none());
}

#[test]
fn local_models_require_backend_id() {
    let error = canonicalize_model_spec(UnifiedModelKind::Local, None, ModelSpec::default())
        .expect_err("missing backend_id");

    assert!(
        error.to_string().contains("local models must set backend_id"),
        "unexpected error: {error}"
    );
}

#[test]
fn local_models_clear_cloud_only_fields_and_canonicalize_backend_id() {
    let (backend_id, spec) = canonicalize_model_spec(
        UnifiedModelKind::Local,
        Some(crate::domain::models::ManagedModelBackendId::GgmlLlama),
        ModelSpec {
            provider_id: Some("openai-main".into()),
            remote_model_id: Some("gpt-4.1-mini".into()),
            ..ModelSpec::default()
        },
    )
    .expect("local spec");

    assert_eq!(backend_id, Some(crate::domain::models::ManagedModelBackendId::GgmlLlama));
    assert!(spec.provider_id.is_none());
    assert!(spec.remote_model_id.is_none());
}

#[test]
fn local_models_canonicalize_explicit_hub_provider() {
    let (_, spec) = canonicalize_model_spec(
        UnifiedModelKind::Local,
        Some(crate::domain::models::ManagedModelBackendId::GgmlLlama),
        ModelSpec { hub_provider: Some(" hf ".into()), ..ModelSpec::default() },
    )
    .expect("local spec");

    assert_eq!(spec.hub_provider.as_deref(), Some("hf_hub"));
}

#[test]
fn local_models_reject_unknown_hub_provider() {
    let error = canonicalize_model_spec(
        UnifiedModelKind::Local,
        Some(crate::domain::models::ManagedModelBackendId::GgmlLlama),
        ModelSpec { hub_provider: Some("unknown".into()), ..ModelSpec::default() },
    )
    .expect_err("invalid hub provider");

    assert!(error.to_string().contains("unsupported hub provider"), "unexpected error: {error}");
}

#[test]
fn hub_invalid_repo_errors_map_to_bad_request() {
    let error = map_hub_client_error(
        "hub file listing failed",
        HubErrorKind::InvalidRepoId,
        "repo_id is invalid".to_owned(),
    );

    assert!(
        matches!(error, AppCoreError::BadRequest(message) if message.contains("repo_id is invalid"))
    );
}

#[test]
fn hub_network_errors_map_to_backend_not_ready() {
    let error = map_hub_client_error(
        "hub file listing failed",
        HubErrorKind::NetworkUnavailable,
        "network unreachable".to_owned(),
    );

    assert!(
        matches!(error, AppCoreError::BackendNotReady(message) if message.contains("network unreachable"))
    );
}

#[test]
fn local_chat_picker_only_includes_llama_models() {
    let whisper = make_model(
        UnifiedModelKind::Local,
        Some("ggml.whisper"),
        None,
        None,
        UnifiedModelStatus::Ready,
        Some("C:/models/whisper.bin"),
    );
    assert!(build_local_chat_model_option(&whisper).is_none());

    let llama = make_model(
        UnifiedModelKind::Local,
        Some("ggml.llama"),
        None,
        None,
        UnifiedModelStatus::Downloading,
        None,
    );
    let option = build_local_chat_model_option(&llama).expect("llama option");

    assert_eq!(option.source, ChatModelSource::Local);
    assert_eq!(option.backend_id, Some(crate::domain::models::ManagedModelBackendId::GgmlLlama));
    assert!(option.pending);
    assert!(!option.downloaded);
}

#[test]
fn cloud_chat_picker_requires_known_provider() {
    let model = make_model(
        UnifiedModelKind::Cloud,
        None,
        Some("openai-main"),
        Some("gpt-4.1-mini"),
        UnifiedModelStatus::Ready,
        None,
    );

    assert!(build_cloud_chat_model_option(&BTreeMap::new(), &model).is_none());

    let mut providers = BTreeMap::new();
    providers.insert(
        "openai-main".to_owned(),
        slab_config::CloudProviderConfig {
            id: "openai-main".to_owned(),
            name: "OpenAI".to_owned(),
            api_base: "https://api.openai.com/v1".to_owned(),
            api_key: None,
            api_key_env: None,
        },
    );

    let option = build_cloud_chat_model_option(&providers, &model).expect("cloud option");
    assert_eq!(option.source, ChatModelSource::Cloud);
    assert_eq!(option.provider_id.as_deref(), Some("openai-main"));
    assert_eq!(option.provider_name.as_deref(), Some("OpenAI"));
}

#[test]
fn empty_runtime_presets_are_dropped() {
    let presets = canonicalize_runtime_presets(Some(RuntimePresets::default()));

    assert!(presets.is_none());
}

#[test]
fn required_text_fields_are_trimmed() {
    let value = normalize_required_text("  model-id  ".into(), "id").expect("trimmed value");

    assert_eq!(value, "model-id");
}

#[test]
fn diffusion_workers_are_clamped_to_one() {
    let (workers, source) =
        validate_and_normalize_model_workers(RuntimeBackendId::GgmlDiffusion, 4, "settings")
            .expect("diffusion worker count should normalize");

    assert_eq!(workers, 1);
    assert_eq!(source, "settings");
}

#[test]
fn non_diffusion_workers_keep_requested_count() {
    let (workers, source) =
        validate_and_normalize_model_workers(RuntimeBackendId::GgmlWhisper, 3, "request")
            .expect("whisper worker count should normalize");

    assert_eq!(workers, 3);
    assert_eq!(source, "request");
}

#[test]
fn product_only_vad_pack_projects_into_local_catalog_model() {
    let manifest = slab_model_pack::ModelPackManifest {
        version: 2,
        id: "whisper-vad".into(),
        label: "whisper-vad".into(),
        status: None,
        family: ModelFamily::Whisper,
        capabilities: vec![Capability::AudioVad],
        backend_hints: DriverHints {
            prefer_drivers: vec!["ggml.whisper".into()],
            avoid_drivers: Vec::new(),
            require_streaming: false,
        },
        context_window: None,
        pricing: None,
        runtime_presets: None,
        metadata: BTreeMap::new(),
        sources: vec![slab_model_pack::PackSourceCandidate::new(
            slab_model_pack::PackSource::HuggingFace {
                repo_id: "ggml-org/whisper-vad".into(),
                revision: None,
                files: vec![slab_model_pack::PackSourceFile {
                    id: "model".into(),
                    label: None,
                    description: None,
                    path: "ggml-silero-v6.2.0.bin".into(),
                }],
            },
        )],
        components: Vec::new(),
        variants: Vec::new(),
        adapters: Vec::new(),
        presets: Vec::new(),
        default_preset: Some("default".into()),
        footprint: Default::default(),
    };
    let preset = slab_model_pack::ResolvedPreset {
        document: slab_model_pack::PresetDocument {
            id: "default".into(),
            label: "Default".into(),
            variant_id: None,
            description: None,
            adapter_ids: Vec::new(),
            load_config: None,
            inference_config: None,
            footprint: Default::default(),
            metadata: BTreeMap::new(),
        },
        variant: slab_model_pack::ResolvedVariant {
            document: slab_model_pack::VariantDocument {
                id: String::new(),
                label: "Original Model".into(),
                description: None,
                sources: Vec::new(),
                component_ids: Vec::new(),
                load_config: None,
                inference_config: None,
                metadata: BTreeMap::new(),
            },
            effective_sources: manifest.sources.clone(),
            components: BTreeMap::new(),
            load_config: None,
            inference_config: None,
        },
        adapters: BTreeMap::new(),
        effective_load_config: None,
        effective_inference_config: None,
    };
    let mut presets = BTreeMap::new();
    presets.insert("default".into(), preset.clone());
    let resolved = slab_model_pack::ResolvedModelPack {
        manifest: manifest.clone(),
        components: BTreeMap::new(),
        text_assets: BTreeMap::new(),
        adapters: BTreeMap::new(),
        variants: BTreeMap::new(),
        presets,
        default_preset_id: Some("default".into()),
    };

    assert!(
        matches!(
            resolved.compile_runtime_bridge(&preset),
            Err(slab_model_pack::ModelPackError::MissingRuntimeCapability)
        ),
        "pure VAD packs should still skip runtime bridge compilation"
    );

    let command = build_local_model_command_from_pack_preset(&manifest, &resolved, &preset)
        .expect("project vad pack into local catalog model");

    assert_eq!(command.backend_id, Some(crate::domain::models::ManagedModelBackendId::GgmlWhisper));
    assert_eq!(command.capabilities, Some(vec![Capability::AudioVad]));
    assert_eq!(command.status, Some(UnifiedModelStatus::NotDownloaded));
    assert_eq!(command.spec.repo_id.as_deref(), Some("ggml-org/whisper-vad"));
    assert_eq!(command.spec.hub_provider.as_deref(), Some("hf_hub"));
    assert_eq!(command.spec.filename.as_deref(), Some("ggml-silero-v6.2.0.bin"));
    assert!(command.spec.local_path.is_none());
}

#[test]
fn zero_workers_are_rejected() {
    let error = validate_and_normalize_model_workers(RuntimeBackendId::GgmlDiffusion, 0, "request")
        .expect_err("zero workers should fail validation");

    assert!(matches!(error, AppCoreError::BadRequest(message) if message.contains("at least 1")));
}

fn make_model(
    kind: UnifiedModelKind,
    backend_id: Option<&str>,
    provider_id: Option<&str>,
    remote_model_id: Option<&str>,
    status: UnifiedModelStatus,
    local_path: Option<&str>,
) -> UnifiedModel {
    let backend_id = backend_id.map(|value| value.parse().expect("managed model backend id"));

    UnifiedModel {
        id: "model-1".to_owned(),
        display_name: "Model 1".to_owned(),
        kind,
        backend_id,
        capabilities: default_model_capabilities(
            kind,
            backend_id,
            "Model 1",
            &ModelSpec {
                provider_id: provider_id.map(str::to_owned),
                remote_model_id: remote_model_id.map(str::to_owned),
                local_path: local_path.map(str::to_owned),
                ..ModelSpec::default()
            },
        ),
        status,
        spec: ModelSpec {
            provider_id: provider_id.map(str::to_owned),
            remote_model_id: remote_model_id.map(str::to_owned),
            local_path: local_path.map(str::to_owned),
            ..ModelSpec::default()
        },
        runtime_presets: None,
        materialized_artifacts: BTreeMap::new(),
        selected_download_source: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}
