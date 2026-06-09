use std::collections::HashMap;

use crate::context::ModelState;
use crate::domain::models::{TaskStatus, UnifiedModel, UnifiedModelKind, UnifiedModelStatus};
use crate::error::AppCoreError;
use crate::infra::db::{ModelDownloadRecord, ModelDownloadStore};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ModelDownloadSourceKey {
    pub(super) model_id: String,
    pub(super) source_key: String,
    pub(super) repo_id: String,
    pub(super) filename: String,
    pub(super) hub_provider: Option<String>,
}

#[derive(Default)]
pub(in crate::domain::services::model) struct ModelDownloadStatusIndex {
    latest_by_source: HashMap<ModelDownloadSourceKey, ModelDownloadRecord>,
    latest_by_model: HashMap<String, ModelDownloadRecord>,
}

pub(in crate::domain::services::model) async fn load_model_download_status_index(
    state: &ModelState,
) -> Result<ModelDownloadStatusIndex, AppCoreError> {
    let mut index = ModelDownloadStatusIndex::default();

    for download in state.store().list_model_downloads().await? {
        let Some(key) = model_download_source_key_from_parts(
            &download.model_id,
            download.hub_provider.as_deref(),
            &download.repo_id,
            &download.filename,
        ) else {
            continue;
        };

        index.latest_by_source.entry(key).or_insert_with(|| download.clone());
        index.latest_by_model.entry(download.model_id.clone()).or_insert(download);
    }

    Ok(index)
}

pub(in crate::domain::services::model) fn effective_model_status(
    model: &UnifiedModel,
    download_status: &ModelDownloadStatusIndex,
) -> UnifiedModelStatus {
    if model.kind != UnifiedModelKind::Local {
        return model.status.clone();
    }

    let base_status = normalized_local_model_status(model);

    if let Some(download) = download_status.latest_by_model.get(&model.id)
        && matches!(download.status, TaskStatus::Pending | TaskStatus::Running)
    {
        return UnifiedModelStatus::Downloading;
    }

    if base_status == UnifiedModelStatus::Ready {
        return UnifiedModelStatus::Ready;
    }

    if let Some(source_key) = model_download_source_key(model)
        && let Some(download) = download_status.latest_by_source.get(&source_key)
    {
        return match download.status {
            TaskStatus::Pending | TaskStatus::Running => UnifiedModelStatus::Downloading,
            TaskStatus::Failed | TaskStatus::Cancelled | TaskStatus::Interrupted => {
                UnifiedModelStatus::Error
            }
            TaskStatus::Succeeded => base_status,
        };
    }

    match download_status.latest_by_model.get(&model.id).map(|download| download.status) {
        Some(TaskStatus::Failed | TaskStatus::Cancelled | TaskStatus::Interrupted) => {
            UnifiedModelStatus::Error
        }
        _ => base_status,
    }
}

pub(super) fn model_download_source_key_from_parts(
    model_id: &str,
    hub_provider: Option<&str>,
    repo_id: &str,
    filename: &str,
) -> Option<ModelDownloadSourceKey> {
    let model_id = model_id.trim();
    let repo_id = repo_id.trim();
    let filename = filename.trim();
    if model_id.is_empty() || repo_id.is_empty() || filename.is_empty() {
        return None;
    }

    Some(ModelDownloadSourceKey {
        model_id: model_id.to_owned(),
        source_key: format!(
            "{}::{}::{}",
            source_key_hub_provider_segment(hub_provider),
            repo_id,
            filename
        ),
        repo_id: repo_id.to_owned(),
        filename: filename.to_owned(),
        hub_provider: normalized_source_key_hub_provider(hub_provider),
    })
}

fn normalized_local_model_status(model: &UnifiedModel) -> UnifiedModelStatus {
    if model.spec.local_path.as_deref().map(str::trim).is_some_and(|value| !value.is_empty()) {
        return UnifiedModelStatus::Ready;
    }

    match &model.status {
        UnifiedModelStatus::Error => UnifiedModelStatus::Error,
        _ => UnifiedModelStatus::NotDownloaded,
    }
}

fn model_download_source_key(model: &UnifiedModel) -> Option<ModelDownloadSourceKey> {
    model_download_source_key_from_parts(
        &model.id,
        model.spec.hub_provider.as_deref(),
        model.spec.repo_id.as_deref().unwrap_or_default(),
        model.spec.filename.as_deref().unwrap_or_default(),
    )
}

fn normalized_source_key_hub_provider(hub_provider: Option<&str>) -> Option<String> {
    hub_provider.map(str::trim).filter(|value| !value.is_empty()).map(|value| {
        match value.to_ascii_lowercase().replace('-', "_").as_str() {
            "hf" | "hf_hub" | "huggingface" | "hugging_face" => "hf_hub".to_owned(),
            "models_cat" | "modelscope" | "model_scope" => "models_cat".to_owned(),
            other => other.to_owned(),
        }
    })
}

fn source_key_hub_provider_segment(hub_provider: Option<&str>) -> String {
    match normalized_source_key_hub_provider(hub_provider).as_deref() {
        Some("hf_hub") => "hugging_face".to_owned(),
        Some("models_cat") => "model_scope".to_owned(),
        Some(other) => other.to_owned(),
        None => "auto".to_owned(),
    }
}
