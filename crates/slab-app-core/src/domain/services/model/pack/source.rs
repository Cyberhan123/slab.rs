use std::path::PathBuf;

use crate::domain::models::{ModelSpec, SelectedModelDownloadSource, StoredModelConfig};

use super::super::catalog;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(in crate::domain::services::model) struct ModelSourcePreview {
    pub(in crate::domain::services::model) repo_id: Option<String>,
    pub(in crate::domain::services::model) filename: Option<String>,
    pub(in crate::domain::services::model) hub_provider: Option<String>,
    pub(in crate::domain::services::model) local_path: Option<String>,
}

impl ModelSourcePreview {
    fn into_model_spec(self) -> ModelSpec {
        ModelSpec {
            repo_id: self.repo_id,
            filename: self.filename,
            hub_provider: self.hub_provider,
            local_path: self.local_path,
            ..Default::default()
        }
    }

    fn is_empty(&self) -> bool {
        self.repo_id.is_none() && self.filename.is_none() && self.local_path.is_none()
    }
}

fn canonical_hub_provider(value: Option<&str>) -> Option<String> {
    catalog::normalized_hub_provider_preference(value).ok().and_then(|(_, canonical)| canonical)
}

fn comparable_hub_provider(spec: &ModelSpec) -> Option<String> {
    let has_remote_source =
        spec.repo_id.as_deref().map(str::trim).filter(|value| !value.is_empty()).is_some()
            && spec.filename.as_deref().map(str::trim).filter(|value| !value.is_empty()).is_some();

    canonical_hub_provider(spec.hub_provider.as_deref())
        .or_else(|| has_remote_source.then(|| "hf_hub".to_owned()))
}

fn pack_source_hub_provider(source: &slab_model_pack::PackSource) -> Option<String> {
    match source {
        slab_model_pack::PackSource::HuggingFace { .. } => Some("hf_hub".to_owned()),
        slab_model_pack::PackSource::ModelScope { .. } => Some("models_cat".to_owned()),
        slab_model_pack::PackSource::LocalPath { .. }
        | slab_model_pack::PackSource::LocalFiles { .. }
        | slab_model_pack::PackSource::Cloud { .. } => None,
    }
}

pub(in crate::domain::services::model) fn source_preview_from_pack_source(
    source: Option<&slab_model_pack::PackSourceCandidate>,
) -> ModelSourcePreview {
    match source.map(|candidate| &candidate.source) {
        Some(
            source @ (slab_model_pack::PackSource::HuggingFace { .. }
            | slab_model_pack::PackSource::ModelScope { .. }),
        ) => {
            let remote_source = source
                .remote_repository()
                .expect("remote source candidates expose repository info");
            ModelSourcePreview {
                repo_id: Some(remote_source.repo_id.to_owned()),
                filename: remote_source
                    .files
                    .iter()
                    .find(|file| file.id == "model")
                    .or_else(|| remote_source.files.first())
                    .map(|file| file.path.clone()),
                hub_provider: pack_source_hub_provider(source),
                local_path: None,
            }
        }
        Some(slab_model_pack::PackSource::LocalPath { path }) => {
            ModelSourcePreview { local_path: Some(path.clone()), ..Default::default() }
        }
        Some(slab_model_pack::PackSource::LocalFiles { files }) => ModelSourcePreview {
            local_path: files
                .iter()
                .find(|file| file.id == "model")
                .or_else(|| files.first())
                .map(|file| file.path.clone()),
            ..Default::default()
        },
        Some(slab_model_pack::PackSource::Cloud { .. }) | None => ModelSourcePreview::default(),
    }
}

fn source_preview_from_model_source(
    source: &slab_types::ModelSource,
    hub_provider: Option<&str>,
) -> ModelSourcePreview {
    match source {
        slab_types::ModelSource::HuggingFace { repo_id, files, .. } => ModelSourcePreview {
            repo_id: Some(repo_id.clone()),
            filename: files
                .get("model")
                .or_else(|| files.values().next())
                .map(|path| path.to_string_lossy().into_owned()),
            hub_provider: canonical_hub_provider(hub_provider),
            local_path: None,
        },
        slab_types::ModelSource::LocalPath { path } => ModelSourcePreview {
            local_path: Some(path.to_string_lossy().into_owned()),
            ..Default::default()
        },
        slab_types::ModelSource::LocalArtifacts { files } => ModelSourcePreview {
            local_path: files
                .get("model")
                .or_else(|| files.values().next())
                .map(|path| path.to_string_lossy().into_owned()),
            ..Default::default()
        },
        _ => ModelSourcePreview::default(),
    }
}

pub(super) fn preview_from_pack_candidate_or_model_source(
    source_hint: Option<&slab_model_pack::PackSourceCandidate>,
    source: &slab_types::ModelSource,
) -> ModelSourcePreview {
    let preview = source_preview_from_pack_source(source_hint);
    if preview.is_empty() { source_preview_from_model_source(source, None) } else { preview }
}

pub(in crate::domain::services::model) fn materialized_model_source(
    source: &slab_types::ModelSource,
    persisted: Option<&StoredModelConfig>,
    source_hint: Option<&slab_model_pack::PackSourceCandidate>,
) -> slab_types::ModelSource {
    let Some(persisted) = persisted else {
        return source.clone();
    };
    let mut persisted_spec = persisted.spec.clone();
    if let Some(selected_download_source) = persisted.selected_download_source.as_ref() {
        apply_selected_download_source_to_spec(&mut persisted_spec, selected_download_source);
    }
    let projected_spec =
        preview_from_pack_candidate_or_model_source(source_hint, source).into_model_spec();
    if !same_model_download_source(&persisted_spec, &projected_spec) {
        return source.clone();
    }

    if !persisted.materialized_artifacts.is_empty() {
        return slab_types::ModelSource::LocalArtifacts {
            files: persisted
                .materialized_artifacts
                .iter()
                .map(|(artifact_id, path)| (artifact_id.clone(), PathBuf::from(path)))
                .collect(),
        };
    }

    let Some(local_path) =
        persisted.spec.local_path.as_deref().map(str::trim).filter(|value| !value.is_empty())
    else {
        return source.clone();
    };

    match source {
        slab_types::ModelSource::HuggingFace { .. }
        | slab_types::ModelSource::LocalPath { .. }
        | slab_types::ModelSource::LocalArtifacts { .. } => {
            slab_types::ModelSource::LocalPath { path: PathBuf::from(local_path) }
        }
        _ => source.clone(),
    }
}

pub(in crate::domain::services::model) fn apply_materialized_source_to_bridge(
    bridge: &mut slab_model_pack::ModelPackRuntimeBridge,
    persisted: Option<&StoredModelConfig>,
    source_hint: Option<&slab_model_pack::PackSourceCandidate>,
) {
    bridge.model_spec.source =
        materialized_model_source(&bridge.model_spec.source, persisted, source_hint);
}

pub(in crate::domain::services::model) fn apply_selected_download_source_to_spec(
    spec: &mut ModelSpec,
    selected_download_source: &SelectedModelDownloadSource,
) {
    spec.repo_id = Some(selected_download_source.repo_id.clone());
    spec.filename = Some(selected_download_source.filename.clone());
    spec.hub_provider = selected_download_source.hub_provider.clone();
}

pub(in crate::domain::services::model) fn same_model_download_source(
    current: &ModelSpec,
    next: &ModelSpec,
) -> bool {
    match (current.repo_id.as_deref(), next.repo_id.as_deref()) {
        (Some(_), Some(_)) => {
            current.repo_id == next.repo_id
                && current.filename == next.filename
                && comparable_hub_provider(current) == comparable_hub_provider(next)
        }
        (None, None) => current.local_path == next.local_path,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use slab_model_pack::{PackSource, PackSourceCandidate, PackSourceFile};

    use super::{materialized_model_source, same_model_download_source};
    use crate::domain::models::{
        CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION, CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
        ModelSpec, StoredModelConfig, UnifiedModelKind, UnifiedModelStatus,
    };

    #[test]
    fn same_model_download_source_treats_legacy_blank_provider_as_hf_hub() {
        let persisted = ModelSpec {
            repo_id: Some("ggml-org/whisper-vad".into()),
            filename: Some("ggml-silero-v6.2.0.bin".into()),
            hub_provider: Some("hf_hub".into()),
            ..ModelSpec::default()
        };
        let projected = ModelSpec {
            repo_id: Some("ggml-org/whisper-vad".into()),
            filename: Some("ggml-silero-v6.2.0.bin".into()),
            hub_provider: None,
            ..ModelSpec::default()
        };

        assert!(same_model_download_source(&persisted, &projected));
    }

    #[test]
    fn materialized_model_source_uses_pack_source_provider_hint_for_modelscope() {
        let mut files = BTreeMap::new();
        files.insert("model".to_owned(), PathBuf::from("Qwen2.5-7B-Instruct-Q4_K_M.gguf"));

        let source = slab_types::ModelSource::HuggingFace {
            repo_id: "Qwen/Qwen2.5-7B-Instruct".into(),
            revision: None,
            files,
        };
        let persisted = StoredModelConfig {
            schema_version: CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
            policy_version: CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION,
            id: "local-qwen".into(),
            display_name: "Local Qwen".into(),
            kind: UnifiedModelKind::Local,
            backend_id: None,
            capabilities: Vec::new(),
            status: Some(UnifiedModelStatus::Ready),
            spec: ModelSpec {
                repo_id: Some("Qwen/Qwen2.5-7B-Instruct".into()),
                filename: Some("Qwen2.5-7B-Instruct-Q4_K_M.gguf".into()),
                hub_provider: Some("models_cat".into()),
                local_path: Some("C:/models/Qwen2.5-7B-Instruct-Q4_K_M.gguf".into()),
                ..ModelSpec::default()
            },
            runtime_presets: None,
            materialized_artifacts: BTreeMap::new(),
            selected_download_source: None,
            pack_selection: None,
        };
        let source_hint = PackSourceCandidate::new(PackSource::ModelScope {
            repo_id: "Qwen/Qwen2.5-7B-Instruct".into(),
            revision: None,
            files: vec![PackSourceFile {
                id: "model".into(),
                label: None,
                description: None,
                path: "Qwen2.5-7B-Instruct-Q4_K_M.gguf".into(),
            }],
        });

        let materialized = materialized_model_source(&source, Some(&persisted), Some(&source_hint));

        assert_eq!(
            materialized,
            slab_types::ModelSource::LocalPath {
                path: PathBuf::from("C:/models/Qwen2.5-7B-Instruct-Q4_K_M.gguf")
            }
        );
    }
}
