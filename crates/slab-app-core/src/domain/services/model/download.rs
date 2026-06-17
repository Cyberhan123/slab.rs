use super::download_progress::ModelDownloadProgressReporter;
pub(super) use super::download_status::{effective_model_status, load_model_download_status_index};

use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use slab_config::ModelDownloadSourcePreference;
use slab_hub::DownloadProgress;
use tracing::{info, warn};

use crate::domain::models::{
    AcceptedOperation, DownloadModelCommand, SelectedModelDownloadSource, TaskStatus, UnifiedModel,
};
use crate::error::{AppCoreError, AppCoreErrorData};
use crate::infra::db::{ModelDownloadRecord, ModelDownloadStore, ModelStore, TaskRecord};
use crate::infra::model_packs;

use super::download_status::{ModelDownloadSourceKey, model_download_source_key_from_parts};
use super::{ModelService, catalog, pack, runtime};

pub(crate) const MODEL_DOWNLOAD_TASK_TYPE: &str = "model_download";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelDownloadTaskInput {
    model_id: String,
    backend_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model_cache_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    candidates: Vec<ModelDownloadTaskCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelDownloadTaskCandidate {
    source_key: String,
    repo_id: String,
    filename: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hub_provider: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    artifacts: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    primary_artifact_id: Option<String>,
}

#[derive(Serialize)]
struct ModelDownloadResultPayload {
    local_path: String,
}

#[derive(Debug, Clone)]
struct ResolvedModelDownloadPlan {
    task_key: ModelDownloadSourceKey,
    candidates: Vec<ModelDownloadTaskCandidate>,
}

impl ModelService {
    pub async fn download_model(
        &self,
        req: DownloadModelCommand,
    ) -> Result<AcceptedOperation, AppCoreError> {
        let model_id = req.model_id.trim().to_owned();

        let configured_model_cache_dir = self.model_state.pmid().config().runtime.model_cache_dir;
        if let Some(dir) = &configured_model_cache_dir {
            catalog::validate_path("model_cache_dir", dir)?;
        }

        let record = self
            .model_state
            .store()
            .get_model(&model_id)
            .await?
            .ok_or_else(|| AppCoreError::NotFound(format!("model {model_id} not found")))?;

        let model: UnifiedModel =
            record.try_into().map_err(|error: String| AppCoreError::Internal(error))?;

        let backend_id = runtime::resolve_local_backend_from_model(&model)?;

        let canonical_backend_id = backend_id.to_string();
        let download_preference =
            self.model_state.pmid().model_download_source_preference().await?;
        let download_plan = self.resolve_model_download_plan(&model, download_preference).await?;
        let download_key = download_plan.task_key.clone();

        self.model_state.store().reconcile_model_downloads().await?;
        if let Some(existing) = self
            .model_state
            .store()
            .get_active_model_download_for_source(&download_key.model_id, &download_key.source_key)
            .await?
        {
            info!(
                task_id = %existing.task_id,
                backend_id = %backend_id,
                model_id = %model_id,
                "reusing existing model download task"
            );
            return Ok(AcceptedOperation { operation_id: existing.task_id });
        }

        let input_data = serde_json::to_string(&ModelDownloadTaskInput {
            model_id: model.id,
            backend_id: canonical_backend_id,
            model_cache_dir: configured_model_cache_dir,
            candidates: download_plan.candidates.clone(),
        })
        .map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to serialize model download task input: {error}"
            ))
        })?;

        let operation_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        let insert_result = self
            .model_state
            .store()
            .insert_model_download_operation(
                TaskRecord {
                    id: operation_id.clone(),
                    task_type: MODEL_DOWNLOAD_TASK_TYPE.to_owned(),
                    status: TaskStatus::Pending,
                    model_id: Some(model_id.clone()),
                    input_data: Some(input_data.clone()),
                    result_data: None,
                    error_msg: None,
                    core_task_id: None,
                    created_at: now,
                    updated_at: now,
                },
                ModelDownloadRecord {
                    task_id: operation_id.clone(),
                    model_id: model_id.clone(),
                    source_key: download_key.source_key.clone(),
                    repo_id: download_key.repo_id.clone(),
                    filename: download_key.filename.clone(),
                    hub_provider: download_key.hub_provider.clone(),
                    status: TaskStatus::Pending,
                    error_msg: None,
                    created_at: now,
                    updated_at: now,
                },
            )
            .await;

        match insert_result {
            Ok(()) => {}
            Err(error) if is_model_download_conflict(&error) => {
                if let Some(existing) = self
                    .model_state
                    .store()
                    .get_active_model_download_for_source(
                        &download_key.model_id,
                        &download_key.source_key,
                    )
                    .await?
                {
                    info!(
                        task_id = %existing.task_id,
                        backend_id = %backend_id,
                        model_id = %model_id,
                        "reusing concurrently created model download task"
                    );
                    return Ok(AcceptedOperation { operation_id: existing.task_id });
                }

                return Err(error.into());
            }
            Err(error) => return Err(error.into()),
        }

        self.spawn_model_download_operation(operation_id.clone(), input_data);

        info!(
            task_id = %operation_id,
            backend_id = %backend_id,
            model_id = %model_id,
            "model download task accepted"
        );

        Ok(AcceptedOperation { operation_id })
    }
}

impl ModelService {
    pub(crate) async fn restart_model_download_task(
        &self,
        task: TaskRecord,
    ) -> Result<(), AppCoreError> {
        match task.status {
            TaskStatus::Pending | TaskStatus::Running => {
                return Err(AppCoreError::Conflict(format!(
                    "task {} is already active (status: {})",
                    task.id, task.status
                )));
            }
            TaskStatus::Succeeded => {
                return Err(AppCoreError::BadRequest(format!(
                    "task {} cannot be restarted (status: {})",
                    task.id, task.status
                )));
            }
            TaskStatus::Failed | TaskStatus::Cancelled | TaskStatus::Interrupted => {}
        }

        let input_data = task.input_data.clone().ok_or_else(|| {
            AppCoreError::BadRequest(format!(
                "model download task {} is missing input_data",
                task.id
            ))
        })?;
        serde_json::from_str::<ModelDownloadTaskInput>(&input_data).map_err(|error| {
            AppCoreError::BadRequest(format!(
                "model download task {} has invalid input_data: {error}",
                task.id
            ))
        })?;

        self.model_state.store().reconcile_model_downloads().await?;
        let download =
            self.model_state.store().get_model_download(&task.id).await?.ok_or_else(|| {
                AppCoreError::BadRequest(format!(
                    "model download task {} is missing model_downloads row",
                    task.id
                ))
            })?;

        if let Some(active) = self
            .model_state
            .store()
            .get_active_model_download_for_source(&download.model_id, &download.source_key)
            .await?
            && active.task_id != task.id
        {
            return Err(AppCoreError::Conflict(format!(
                "model download source is already active in task {}",
                active.task_id
            )));
        }

        match self.model_state.store().restart_model_download_task(&task.id).await {
            Ok(()) => {}
            Err(error) if is_model_download_conflict(&error) => {
                return Err(AppCoreError::Conflict(format!(
                    "model download source is already active for task {}",
                    task.id
                )));
            }
            Err(error) => return Err(error.into()),
        }

        self.spawn_model_download_operation(task.id.clone(), input_data);
        info!(task_id = %task.id, "model download task restarted");
        Ok(())
    }

    fn spawn_model_download_operation(&self, operation_id: String, input_data: String) {
        let store = Arc::clone(self.model_state.store());
        let auto_unload = Arc::clone(self.model_state.auto_unload());

        self.worker_state.spawn_existing_operation(operation_id, move |operation| async move {
            run_model_download_operation(operation, store, auto_unload, input_data).await;
        });
    }
}

async fn run_model_download_operation(
    operation: crate::context::worker_state::OperationContext,
    store: Arc<crate::infra::db::AnyStore>,
    auto_unload: Arc<crate::model_auto_unload::ModelAutoUnloadManager>,
    input_data: String,
) {
    let operation_id = operation.id().to_owned();
    if let Err(error) = operation.mark_running().await {
        warn!(task_id = %operation_id, error = %error, "failed to mark model download running");
        let _ = persist_model_download_status(
            &store,
            &operation_id,
            TaskStatus::Failed,
            Some(&error.to_string()),
        )
        .await;
        return;
    }

    if let Err(error) =
        persist_model_download_status(&store, &operation_id, TaskStatus::Running, None).await
    {
        warn!(task_id = %operation_id, error = %error, "failed to persist model download running status");
    }

    let input: ModelDownloadTaskInput = match serde_json::from_str(&input_data) {
        Ok(value) => value,
        Err(error) => {
            warn!(task_id = %operation_id, error = %error, "invalid stored input_data for model_download task");
            let message = format!("invalid stored input_data: {error}");
            mark_model_download_failed(&operation, &store, &operation_id, &message).await;
            return;
        }
    };

    let model_id = input.model_id.trim().to_owned();
    let model_cache_dir = input
        .model_cache_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let download_candidates = input.candidates;

    if model_id.is_empty() || download_candidates.is_empty() {
        warn!(task_id = %operation_id, "model_download task is missing model_id or candidates");
        mark_model_download_failed(
            &operation,
            &store,
            &operation_id,
            "missing model_id or candidates in stored input_data",
        )
        .await;
        return;
    }

    let mut attempt_errors = Vec::new();
    let mut successful_download: Option<(
        String,
        BTreeMap<String, String>,
        SelectedModelDownloadSource,
    )> = None;

    for candidate in download_candidates {
        let repo_id = candidate.repo_id.trim().to_owned();
        let filename = candidate.filename.trim().to_owned();
        let hub_provider = candidate
            .hub_provider
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let download_artifacts = if candidate.artifacts.is_empty() {
            let mut artifacts = BTreeMap::new();
            artifacts.insert("model".to_owned(), filename.clone());
            artifacts
        } else {
            candidate.artifacts
        };
        let primary_artifact_id = candidate
            .primary_artifact_id
            .or_else(|| catalog::primary_artifact_key(&download_artifacts));

        if repo_id.is_empty() || filename.is_empty() {
            attempt_errors.push(format!(
                "{}: candidate is missing repo_id or filename",
                candidate.source_key
            ));
            continue;
        }

        let attempt_result = async {
            let client =
                catalog::build_hub_client(model_cache_dir.as_deref(), hub_provider.as_deref())
                    .map_err(|error| error.to_string())?;
            let mut materialized_artifacts = BTreeMap::new();
            let progress: Arc<dyn DownloadProgress> = Arc::new(ModelDownloadProgressReporter::new(
                operation_id.clone(),
                Arc::clone(&store),
                &download_artifacts,
            ));

            for (artifact_id, artifact_file) in &download_artifacts {
                let path = client
                    .download_file(&repo_id, artifact_file, Some(Arc::clone(&progress)))
                    .await
                    .map_err(|error| format!("hub download failed for {artifact_file}: {error}"))?;
                materialized_artifacts
                    .insert(artifact_id.clone(), path.to_string_lossy().into_owned());
            }

            let local_path = primary_artifact_id
                .as_ref()
                .and_then(|artifact_id| materialized_artifacts.get(artifact_id))
                .cloned()
                .or_else(|| materialized_artifacts.values().next().cloned())
                .ok_or_else(|| "hub download produced no local artifacts".to_owned())?;

            Ok::<(String, BTreeMap<String, String>), String>((local_path, materialized_artifacts))
        }
        .await;

        match attempt_result {
            Ok((local_path, materialized_artifacts)) => {
                successful_download = Some((
                    local_path,
                    materialized_artifacts,
                    SelectedModelDownloadSource {
                        source_key: candidate.source_key,
                        repo_id,
                        filename,
                        hub_provider,
                    },
                ));
                break;
            }
            Err(error) => {
                attempt_errors.push(format!("{}: {error}", candidate.source_key));
            }
        }
    }

    let Some((local_path, materialized_artifacts, selected_source)) = successful_download else {
        let error = if attempt_errors.is_empty() {
            "model download failed without a candidate attempt".to_owned()
        } else {
            attempt_errors.join(" | ")
        };
        warn!(task_id = %operation_id, error = %error, "model download failed");
        mark_model_download_failed(&operation, &store, &operation_id, &error).await;
        return;
    };

    let materialized_artifacts_json = match serde_json::to_string(&materialized_artifacts) {
        Ok(value) => value,
        Err(error) => {
            let message = format!("downloaded file but failed to serialize artifacts: {error}");
            mark_model_download_failed(&operation, &store, &operation_id, &message).await;
            return;
        }
    };
    let selected_source_json = match serde_json::to_string(&selected_source) {
        Ok(value) => value,
        Err(error) => {
            let message = format!("downloaded file but failed to serialize source: {error}");
            mark_model_download_failed(&operation, &store, &operation_id, &message).await;
            return;
        }
    };

    if let Err(error) = store
        .update_model_download_state(
            &model_id,
            &local_path,
            "ready",
            &materialized_artifacts_json,
            Some(&selected_source_json),
        )
        .await
    {
        warn!(task_id = %operation_id, error = %error, "failed to persist downloaded model path");
        let message = format!("downloaded file but failed to persist path: {error}");
        mark_model_download_failed(&operation, &store, &operation_id, &message).await;
        return;
    }

    auto_unload
        .invalidate_model_replay(&model_id, "model download updated catalog local_path")
        .await;

    let result_json =
        serde_json::to_string(&ModelDownloadResultPayload { local_path: local_path.clone() })
            .unwrap_or_default();
    if let Err(db_error) = operation.mark_succeeded(&result_json).await {
        warn!(task_id = %operation_id, error = %db_error, "failed to persist model download success");
    }
    if let Err(error) =
        persist_model_download_status(&store, &operation_id, TaskStatus::Succeeded, None).await
    {
        warn!(task_id = %operation_id, error = %error, "failed to persist model download success status");
    }
    info!(task_id = %operation_id, local_path = %local_path, "model download succeeded");
}

async fn persist_model_download_status(
    store: &Arc<crate::infra::db::AnyStore>,
    task_id: &str,
    status: TaskStatus,
    error_msg: Option<&str>,
) -> Result<(), AppCoreError> {
    store.update_model_download_status(task_id, status, error_msg).await?;
    Ok(())
}

async fn mark_model_download_failed(
    operation: &crate::context::worker_state::OperationContext,
    store: &Arc<crate::infra::db::AnyStore>,
    task_id: &str,
    message: &str,
) {
    if let Err(db_error) = operation.mark_failed(message).await {
        warn!(task_id = %task_id, error = %db_error, "failed to persist model download failure");
    }
    if let Err(error) =
        persist_model_download_status(store, task_id, TaskStatus::Failed, Some(message)).await
    {
        warn!(task_id = %task_id, error = %error, "failed to persist model download error status");
    }
}

impl ModelService {
    async fn resolve_model_download_plan(
        &self,
        model: &UnifiedModel,
        preference: ModelDownloadSourcePreference,
    ) -> Result<ResolvedModelDownloadPlan, AppCoreError> {
        let mut candidates = self
            .resolve_model_download_plan_from_pack(model, preference)
            .await?
            .unwrap_or_default();

        if candidates.is_empty() {
            candidates.push(build_download_task_candidate(
                &model.id,
                model.spec.repo_id.clone().ok_or_else(|| {
                    model_download_unavailable_error(
                        &model.id,
                        "missing repo_id required for download",
                        "Add a repo_id to the model source or import a model pack with a downloadable source.",
                    )
                })?,
                model.spec.filename.clone().ok_or_else(|| {
                    model_download_unavailable_error(
                        &model.id,
                        "missing filename required for download",
                        "Add a filename to the model source or select a model pack variant with a downloadable artifact.",
                    )
                })?,
                effective_hub_provider_for_model_spec(
                    model.spec.hub_provider.as_deref(),
                    preference,
                )?,
                BTreeMap::from([(
                    "model".to_owned(),
                    model.spec.filename.clone().expect("filename checked above"),
                )]),
                Some("model".to_owned()),
            )?);
        }

        let task_candidate = candidates.first().cloned().ok_or_else(|| {
            model_download_unavailable_error(
                &model.id,
                "no downloadable source candidates",
                "Select a model pack variant with a remote source or add repo_id and filename to the model.",
            )
        })?;
        let task_key = model_download_source_key_from_parts(
            &model.id,
            task_candidate.hub_provider.as_deref(),
            &task_candidate.repo_id,
            &task_candidate.filename,
        )
        .ok_or_else(|| {
            model_download_unavailable_error(
                &model.id,
                "download candidate is missing repo_id or filename",
                "Select a model pack variant with a complete remote source.",
            )
        })?;

        Ok(ResolvedModelDownloadPlan { task_key, candidates })
    }

    async fn resolve_model_download_plan_from_pack(
        &self,
        model: &UnifiedModel,
        preference: ModelDownloadSourcePreference,
    ) -> Result<Option<Vec<ModelDownloadTaskCandidate>>, AppCoreError> {
        let pack_path = model_packs::model_pack_file_path(self.model_config_dir(), &model.id);
        if !pack_path.exists() {
            return Ok(None);
        }

        let context = self.load_model_pack_context(&model.id).await?;
        let selection = self.resolve_model_pack_selection(&model.id, &context.resolved).await?;

        let candidates = selection
            .selected_preset
            .variant
            .effective_sources
            .iter()
            .filter_map(|candidate| {
                pack_source_candidate_to_download_task_candidate(&model.id, candidate, preference)
                    .transpose()
            })
            .collect::<Result<Vec<_>, _>>()?;

        if !candidates.is_empty() {
            return Ok(Some(candidates));
        }

        let source = match context.resolved.compile_runtime_bridge(&selection.selected_preset) {
            Ok(bridge) => bridge.model_spec.source,
            Err(slab_model_pack::ModelPackError::MissingRuntimeCapability)
                if !pack::pack_has_runtime_execution_capability(&context.resolved.manifest) =>
            {
                pack::resolve_pack_model_source(
                    &context.resolved,
                    &selection.selected_preset,
                    "failed to resolve selected pack preset source for download plan",
                )?
            }
            Err(error) => {
                return Err(AppCoreError::BadRequest(format!(
                    "failed to compile selected pack preset for download plan: {error}"
                )));
            }
        };

        let source = match source {
            slab_types::ModelSource::HuggingFace { repo_id, files, .. } => {
                let artifacts = files
                    .into_iter()
                    .map(|(artifact_id, path)| (artifact_id, path.to_string_lossy().into_owned()))
                    .collect::<BTreeMap<_, _>>();
                let primary_artifact_id = catalog::primary_artifact_key(&artifacts);
                let filename = primary_artifact_id
                    .as_ref()
                    .and_then(|artifact_id| artifacts.get(artifact_id))
                    .cloned()
                    .or_else(|| artifacts.values().next().cloned())
                    .ok_or_else(|| {
                        AppCoreError::BadRequest(format!(
                            "model {} resolved download plan is missing a primary artifact",
                            model.id
                        ))
                    })?;

                Some(build_download_task_candidate(
                    &model.id,
                    repo_id,
                    filename,
                    effective_hub_provider_for_model_spec(None, preference)?,
                    artifacts,
                    primary_artifact_id,
                )?)
            }
            _ => None,
        };

        Ok(source.map(|candidate| vec![candidate]))
    }
}

fn pack_source_candidate_to_download_task_candidate(
    model_id: &str,
    candidate: &slab_model_pack::PackSourceCandidate,
    preference: ModelDownloadSourcePreference,
) -> Result<Option<ModelDownloadTaskCandidate>, AppCoreError> {
    let Some(remote_source) = candidate.source.remote_repository() else {
        return Ok(None);
    };

    let Some(hub_provider) = effective_hub_provider_for_pack_source(&candidate.source, preference)
    else {
        return Ok(None);
    };

    let artifacts = remote_source
        .files
        .iter()
        .map(|file| (file.id.clone(), file.path.clone()))
        .collect::<BTreeMap<_, _>>();
    let primary_artifact_id = catalog::primary_artifact_key(&artifacts);
    let filename = primary_artifact_id
        .as_ref()
        .and_then(|artifact_id| artifacts.get(artifact_id))
        .cloned()
        .or_else(|| artifacts.values().next().cloned())
        .ok_or_else(|| {
            model_download_unavailable_error(
                model_id,
                "pack source candidate is missing downloadable files",
                "Select a model pack variant with at least one remote artifact.",
            )
        })?;

    Ok(Some(build_download_task_candidate(
        model_id,
        remote_source.repo_id.to_owned(),
        filename,
        hub_provider,
        artifacts,
        primary_artifact_id,
    )?))
}

fn build_download_task_candidate(
    model_id: &str,
    repo_id: String,
    filename: String,
    hub_provider: Option<String>,
    artifacts: BTreeMap<String, String>,
    primary_artifact_id: Option<String>,
) -> Result<ModelDownloadTaskCandidate, AppCoreError> {
    let source_key = model_download_source_key_from_parts(
        model_id,
        hub_provider.as_deref(),
        &repo_id,
        &filename,
    )
    .ok_or_else(|| {
        model_download_unavailable_error(
            model_id,
            "download candidate is missing repo_id or filename",
            "Select a model pack variant with a complete remote source.",
        )
    })?;

    Ok(ModelDownloadTaskCandidate {
        source_key: source_key.source_key,
        repo_id,
        filename,
        hub_provider,
        artifacts,
        primary_artifact_id,
    })
}

fn effective_hub_provider_for_model_spec(
    hub_provider: Option<&str>,
    preference: ModelDownloadSourcePreference,
) -> Result<Option<String>, AppCoreError> {
    match preference {
        ModelDownloadSourcePreference::Auto => {
            let (_, canonical_hub_provider) =
                catalog::normalized_hub_provider_preference(hub_provider)?;
            Ok(canonical_hub_provider)
        }
        ModelDownloadSourcePreference::HuggingFace => Ok(Some("hf_hub".to_owned())),
        ModelDownloadSourcePreference::ModelScope => Ok(Some("models_cat".to_owned())),
    }
}

fn effective_hub_provider_for_pack_source(
    source: &slab_model_pack::PackSource,
    preference: ModelDownloadSourcePreference,
) -> Option<Option<String>> {
    let remote_source = source.remote_repository()?;

    match preference {
        ModelDownloadSourcePreference::Auto => Some(Some(remote_source.hub_provider.to_owned())),
        ModelDownloadSourcePreference::HuggingFace => {
            (remote_source.hub_provider != "models_cat").then_some(Some("hf_hub".to_owned()))
        }
        ModelDownloadSourcePreference::ModelScope => {
            (remote_source.hub_provider != "hf_hub").then_some(Some("models_cat".to_owned()))
        }
    }
}

fn is_model_download_conflict(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(db_error) if db_error.message().contains("UNIQUE constraint failed"))
}

fn model_download_unavailable_error(
    model_id: &str,
    reason: &str,
    suggestion: &str,
) -> AppCoreError {
    AppCoreError::BadRequestData {
        message: format!("model {model_id} cannot be downloaded: {reason}. {suggestion}"),
        data: Box::new(AppCoreErrorData::model_download_unavailable(model_id, reason, suggestion)),
    }
}
