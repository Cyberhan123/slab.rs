use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::Utc;
use slab_hub::{HubClient, HubErrorKind, HubProviderPreference};
use slab_types::Capability;

use crate::context::ModelState;
use crate::domain::models::{
    AvailableModelsQuery, AvailableModelsView, CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION,
    CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION, ChatModelCapabilities, ChatModelOption,
    ChatModelSource, CreateModelCommand, DeletedModelView, ListModelsFilter, ManagedModelBackendId,
    ModelPackSelection, ModelSpec, UnifiedModel, UnifiedModelKind, UnifiedModelStatus,
    UpdateModelCommand, UpdateModelConfigSelectionCommand, normalize_model_capabilities,
};
use crate::error::AppCoreError;
use crate::infra::db::{ModelConfigStateStore, ModelDownloadStore, ModelStore, UnifiedModelRecord};
use crate::infra::model_packs;

use super::{ModelService, download, pack, runtime};

pub(super) type CloudProviderConfig = slab_config::CloudProviderConfig;

impl ModelService {
    pub async fn create_model(
        &self,
        req: CreateModelCommand,
    ) -> Result<UnifiedModel, AppCoreError> {
        self.persist_model_definition(req).await
    }

    pub async fn get_model(&self, id: &str) -> Result<UnifiedModel, AppCoreError> {
        let record = self
            .model_state
            .store()
            .get_model(id)
            .await?
            .ok_or_else(|| AppCoreError::NotFound(format!("model {id} not found")))?;

        record.try_into().map_err(|error: String| AppCoreError::Internal(error))
    }

    pub async fn update_model(
        &self,
        id: &str,
        req: UpdateModelCommand,
    ) -> Result<UnifiedModel, AppCoreError> {
        let existing_record = self
            .model_state
            .store()
            .get_model(id)
            .await?
            .ok_or_else(|| AppCoreError::NotFound(format!("model {id} not found")))?;

        let existing_model: UnifiedModel =
            existing_record.try_into().map_err(|error: String| AppCoreError::Internal(error))?;

        let next = CreateModelCommand {
            id: Some(existing_model.id),
            display_name: req.display_name.unwrap_or(existing_model.display_name),
            kind: req.kind.unwrap_or(existing_model.kind),
            backend_id: req.backend_id.or(existing_model.backend_id),
            capabilities: req.capabilities.or(Some(existing_model.capabilities)),
            status: Some(req.status.unwrap_or(existing_model.status)),
            spec: req.spec.unwrap_or(existing_model.spec),
            runtime_presets: req.runtime_presets.or(existing_model.runtime_presets),
        };

        self.persist_model_definition(next).await
    }

    pub async fn update_model_config_selection(
        &self,
        id: &str,
        req: UpdateModelConfigSelectionCommand,
    ) -> Result<UnifiedModel, AppCoreError> {
        let current_model = self.get_model(id).await?;
        runtime::resolve_local_backend_from_model(&current_model)?;

        let context = self.load_model_pack_context(id).await?;
        let explicit_selection = pack::normalize_model_pack_selection(ModelPackSelection {
            preset_id: req.selected_preset_id,
            variant_id: req.selected_variant_id,
        });
        let selected_preset =
            pack::resolve_selected_model_pack_preset(&context.resolved, &explicit_selection)?;
        let effective_selection = pack::effective_model_pack_selection(
            &context.resolved,
            &explicit_selection,
            &selected_preset,
        );
        let mut command = pack::build_model_command_from_pack_context(&context, &selected_preset)?;
        command.id = Some(current_model.id.clone());

        if pack::same_model_download_source(&current_model.spec, &command.spec) {
            command.spec.local_path = current_model.spec.local_path.clone();
            command.status = Some(current_model.status.clone());
        } else if command.spec.repo_id.is_some() {
            command.spec.local_path = None;
            command.status = Some(UnifiedModelStatus::NotDownloaded);
        }

        let next_model = self.build_model_definition(command).await?;
        let stored_selection = pack::selection_state_record_for_storage(
            id,
            &context.resolved,
            &explicit_selection,
            &effective_selection,
        );
        let stored_model = self.store_model_definition(next_model).await?;

        match stored_selection {
            Some(record) => self.model_state.store().upsert_model_config_state(record).await?,
            None => self.model_state.store().delete_model_config_state(id).await?,
        }

        Ok(stored_model)
    }

    pub async fn delete_model(&self, id: &str) -> Result<DeletedModelView, AppCoreError> {
        let record = self
            .model_state
            .store()
            .get_model(id)
            .await?
            .ok_or_else(|| AppCoreError::NotFound(format!("model {id} not found")))?;
        let model: UnifiedModel =
            record.try_into().map_err(|error: String| AppCoreError::Internal(error))?;

        let _ = model_packs::delete_model_pack(self.model_config_dir(), id)?;
        if let Some(local_path) = model.spec.local_path.as_deref()
            && model_packs::is_model_pack_path(local_path)
        {
            let pack_path = std::path::Path::new(local_path);
            if pack_path.starts_with(self.model_config_dir()) {
                let _ = model_packs::delete_model_pack_at_path(pack_path)?;
            }
        }

        let _ = self.model_state.store().delete_model_config_state(id).await;
        self.model_state.store().delete_model(id).await?;
        self.model_state
            .auto_unload()
            .invalidate_model_replay(id, "model deleted from catalog")
            .await;
        Ok(DeletedModelView { id: id.to_owned(), status: "deleted".to_owned() })
    }

    pub async fn list_models(
        &self,
        query: ListModelsFilter,
    ) -> Result<Vec<UnifiedModel>, AppCoreError> {
        load_models_from_state(&self.model_state, query).await
    }

    pub async fn list_chat_models(&self) -> Result<Vec<ChatModelOption>, AppCoreError> {
        list_chat_models_from_state(&self.model_state).await
    }

    pub async fn list_available_models(
        &self,
        query: AvailableModelsQuery,
    ) -> Result<AvailableModelsView, AppCoreError> {
        let files = HubClient::new().list_repo_files(&query.repo_id).await.map_err(|error| {
            map_hub_client_error("hub file listing failed", error.kind(), error.to_string())
        })?;

        Ok(AvailableModelsView { repo_id: query.repo_id, files })
    }

    pub(super) async fn persist_model_definition(
        &self,
        req: CreateModelCommand,
    ) -> Result<UnifiedModel, AppCoreError> {
        self.persist_model_definition_with_options(req, true).await
    }

    pub(super) async fn persist_model_definition_with_options(
        &self,
        req: CreateModelCommand,
        sync_model_pack: bool,
    ) -> Result<UnifiedModel, AppCoreError> {
        let model = self.build_model_definition(req).await?;
        if sync_model_pack {
            self.write_model_pack(&model)?;
        }

        self.store_model_definition(model).await
    }

    pub(super) async fn store_model_definition(
        &self,
        model: UnifiedModel,
    ) -> Result<UnifiedModel, AppCoreError> {
        let record = model_to_record(&model)?;
        self.model_state.store().upsert_model(record).await?;
        self.model_state
            .auto_unload()
            .invalidate_model_replay(&model.id, "model definition upserted")
            .await;
        Ok(model)
    }

    pub(super) async fn build_model_definition(
        &self,
        req: CreateModelCommand,
    ) -> Result<UnifiedModel, AppCoreError> {
        let id = normalize_required_text(
            req.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            "id",
        )?;
        let display_name = normalize_required_text(req.display_name, "display_name")?;
        let (backend_id, spec) = canonicalize_model_spec(req.kind, req.backend_id, req.spec)?;
        let capabilities = normalize_model_capabilities(
            req.kind,
            backend_id,
            &display_name,
            &spec,
            req.capabilities,
        );
        let runtime_presets = canonicalize_runtime_presets(req.runtime_presets);
        let status = req.status.unwrap_or_else(|| default_status_for_kind(req.kind));

        let existing_record = self.model_state.store().get_model(&id).await?;
        let now = Utc::now();
        let created_at = existing_record.as_ref().map(|record| record.created_at).unwrap_or(now);
        let (materialized_artifacts, selected_download_source) =
            preserved_download_state(existing_record, &spec)?;

        Ok(UnifiedModel {
            id,
            display_name,
            kind: req.kind,
            backend_id,
            capabilities,
            status,
            spec,
            runtime_presets,
            materialized_artifacts,
            selected_download_source,
            created_at,
            updated_at: now,
        })
    }

    pub(super) fn write_model_pack(&self, model: &UnifiedModel) -> Result<(), AppCoreError> {
        model_packs::write_persisted_model_pack(self.model_config_dir(), model)?;
        Ok(())
    }
}

pub(super) async fn load_models_from_state(
    state: &ModelState,
    query: ListModelsFilter,
) -> Result<Vec<UnifiedModel>, AppCoreError> {
    state.store().reconcile_model_downloads().await?;

    let records = state.store().list_models().await?;
    let download_status = download::load_model_download_status_index(state).await?;
    let requested_capability = query.capability;
    let models = records
        .into_iter()
        .filter_map(|record| {
            record
                .try_into()
                .map(|mut model: UnifiedModel| {
                    model.status = download::effective_model_status(&model, &download_status);
                    model
                })
                .map_err(|error: String| {
                    tracing::warn!(error = %error, "failed to deserialize model record; skipping");
                })
                .ok()
        })
        .filter(|model: &UnifiedModel| {
            requested_capability.is_none_or(|capability| model.capabilities.contains(&capability))
        })
        .collect();
    Ok(models)
}

pub(crate) async fn list_chat_models_from_state(
    state: &ModelState,
) -> Result<Vec<ChatModelOption>, AppCoreError> {
    let providers = load_cloud_provider_map_for_chat(state).await?;
    let records = load_models_from_state(
        state,
        ListModelsFilter { capability: Some(Capability::ChatGeneration) },
    )
    .await?;
    let mut items = Vec::new();

    for model in records {
        if let Some(item) = build_local_chat_model_option(&model) {
            items.push(item);
            continue;
        }

        if let Some(item) = build_cloud_chat_model_option(&providers, &model) {
            items.push(item);
        }
    }

    items.sort_by(|left, right| {
        left.display_name
            .to_ascii_lowercase()
            .cmp(&right.display_name.to_ascii_lowercase())
            .then_with(|| left.id.cmp(&right.id))
    });

    Ok(items)
}

async fn load_cloud_provider_map_for_chat(
    state: &ModelState,
) -> Result<BTreeMap<String, CloudProviderConfig>, AppCoreError> {
    Ok(state
        .pmid()
        .config()
        .chat
        .providers
        .into_iter()
        .map(|provider| (provider.id.clone(), provider))
        .collect())
}

fn is_cloud_catalog_model_for_chat(model: &UnifiedModel) -> bool {
    model.kind == UnifiedModelKind::Cloud
        && model.capabilities.contains(&Capability::ChatGeneration)
}

fn is_local_chat_model(model: &UnifiedModel) -> bool {
    model.kind == UnifiedModelKind::Local
        && model.capabilities.contains(&Capability::ChatGeneration)
}

fn referenced_cloud_provider_id(model: &UnifiedModel) -> Option<String> {
    model
        .spec
        .provider_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn local_chat_model_downloaded(model: &UnifiedModel) -> bool {
    matches!(model.status, UnifiedModelStatus::Ready) && model.spec.local_path.is_some()
}

fn local_chat_model_pending(model: &UnifiedModel) -> bool {
    matches!(model.status, UnifiedModelStatus::Downloading)
}

pub(super) fn build_local_chat_model_option(model: &UnifiedModel) -> Option<ChatModelOption> {
    if !is_local_chat_model(model) {
        return None;
    }

    Some(ChatModelOption {
        id: model.id.clone(),
        display_name: model.display_name.clone(),
        source: ChatModelSource::Local,
        downloaded: local_chat_model_downloaded(model),
        pending: local_chat_model_pending(model),
        capabilities: ChatModelCapabilities::local(),
        backend_id: model.backend_id,
        provider_id: None,
        provider_name: None,
    })
}

pub(super) fn build_cloud_chat_model_option(
    providers: &BTreeMap<String, CloudProviderConfig>,
    model: &UnifiedModel,
) -> Option<ChatModelOption> {
    if !is_cloud_catalog_model_for_chat(model) {
        return None;
    }

    let provider_id = referenced_cloud_provider_id(model)?;
    let remote_model_id =
        model.spec.remote_model_id.as_deref().map(str::trim).filter(|value| !value.is_empty());
    if remote_model_id.is_none() {
        tracing::warn!(
            model_id = %model.id,
            provider_id = %provider_id,
            "cloud model is missing remote_model_id; hiding from chat picker"
        );
        return None;
    }
    let Some(provider) = providers.get(&provider_id) else {
        tracing::warn!(
            model_id = %model.id,
            provider_id = %provider_id,
            "cloud model references unknown provider; hiding from chat picker"
        );
        return None;
    };

    Some(ChatModelOption {
        id: model.id.clone(),
        display_name: model.display_name.clone(),
        source: ChatModelSource::Cloud,
        downloaded: true,
        pending: false,
        capabilities: ChatModelCapabilities::cloud(),
        backend_id: None,
        provider_id: Some(provider_id),
        provider_name: Some(provider.name.clone()),
    })
}

pub(super) fn primary_artifact_key<T>(files: &BTreeMap<String, T>) -> Option<String> {
    if files.contains_key("model") {
        return Some("model".to_owned());
    }
    if files.contains_key("diffusion_model") {
        return Some("diffusion_model".to_owned());
    }

    files.keys().next().cloned()
}

fn preserved_download_state(
    existing_record: Option<UnifiedModelRecord>,
    next_spec: &ModelSpec,
) -> Result<
    (BTreeMap<String, String>, Option<crate::domain::models::SelectedModelDownloadSource>),
    AppCoreError,
> {
    let Some(existing_record) = existing_record else {
        return Ok((BTreeMap::new(), None));
    };
    let existing: UnifiedModel =
        existing_record.try_into().map_err(|error: String| AppCoreError::Internal(error))?;
    let mut existing_spec = existing.spec.clone();
    if let Some(selected_download_source) = existing.selected_download_source.as_ref() {
        pack::apply_selected_download_source_to_spec(&mut existing_spec, selected_download_source);
    }

    if pack::same_model_download_source(&existing_spec, next_spec) {
        return Ok((existing.materialized_artifacts, existing.selected_download_source));
    }

    Ok((BTreeMap::new(), None))
}

pub(super) fn canonicalize_model_spec(
    kind: UnifiedModelKind,
    backend_id: Option<ManagedModelBackendId>,
    mut spec: ModelSpec,
) -> Result<(Option<ManagedModelBackendId>, ModelSpec), AppCoreError> {
    spec.provider_id = normalize_optional_text(spec.provider_id);
    spec.remote_model_id = normalize_optional_text(spec.remote_model_id);
    spec.repo_id = normalize_optional_text(spec.repo_id);
    let (_, canonical_hub_provider) =
        normalized_hub_provider_preference(spec.hub_provider.as_deref())?;
    spec.hub_provider = canonical_hub_provider;
    spec.filename = normalize_optional_text(spec.filename);
    spec.local_path = normalize_optional_text(spec.local_path);

    match kind {
        UnifiedModelKind::Cloud => {
            spec.repo_id = None;
            spec.hub_provider = None;
            spec.filename = None;
            spec.local_path = None;

            if spec.provider_id.is_none() {
                return Err(AppCoreError::BadRequest(
                    "cloud models must set spec.provider_id to a configured providers.registry entry"
                        .into(),
                ));
            }
            if spec.remote_model_id.is_none() {
                return Err(AppCoreError::BadRequest(
                    "cloud models must set spec.remote_model_id".into(),
                ));
            }

            Ok((None, spec))
        }
        UnifiedModelKind::Local => {
            spec.provider_id = None;
            spec.remote_model_id = None;

            let backend_id = backend_id.ok_or_else(|| {
                AppCoreError::BadRequest("local models must set backend_id".into())
            })?;

            Ok((Some(backend_id), spec))
        }
    }
}

pub(super) fn canonicalize_runtime_presets(
    runtime_presets: Option<crate::domain::models::RuntimePresets>,
) -> Option<crate::domain::models::RuntimePresets> {
    runtime_presets.filter(|presets| {
        presets.max_tokens.is_some()
            || presets.temperature.is_some()
            || presets.top_p.is_some()
            || presets.top_k.is_some()
            || presets.min_p.is_some()
            || presets.presence_penalty.is_some()
            || presets.repetition_penalty.is_some()
    })
}

fn default_status_for_kind(kind: UnifiedModelKind) -> UnifiedModelStatus {
    match kind {
        UnifiedModelKind::Cloud => UnifiedModelStatus::Ready,
        UnifiedModelKind::Local => UnifiedModelStatus::NotDownloaded,
    }
}

pub(super) fn normalize_required_text(value: String, label: &str) -> Result<String, AppCoreError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppCoreError::BadRequest(format!("{label} must not be empty")));
    }
    Ok(trimmed.to_owned())
}

pub(super) fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
    })
}

pub(super) fn normalized_hub_provider_preference(
    value: Option<&str>,
) -> Result<(HubProviderPreference, Option<String>), AppCoreError> {
    let preference =
        HubProviderPreference::from_optional_str(value).map_err(AppCoreError::BadRequest)?;
    let canonical = match preference {
        HubProviderPreference::Auto => None,
        HubProviderPreference::Provider(provider) => Some(provider.as_config_value().to_owned()),
    };
    Ok((preference, canonical))
}

pub(super) fn build_hub_client(
    model_cache_dir: Option<&str>,
    hub_provider: Option<&str>,
) -> Result<HubClient, AppCoreError> {
    let (provider_preference, _) = normalized_hub_provider_preference(hub_provider)?;
    let mut client = HubClient::new().with_provider_preference(provider_preference);
    if let Some(dir) = model_cache_dir.map(str::trim).filter(|value| !value.is_empty()) {
        client = client.with_cache_dir(PathBuf::from(dir));
    }
    Ok(client)
}

pub(super) fn map_hub_client_error(
    context: &str,
    kind: HubErrorKind,
    message: String,
) -> AppCoreError {
    match kind {
        HubErrorKind::InvalidRepoId | HubErrorKind::UnsupportedProvider => {
            AppCoreError::BadRequest(format!("{context}: {message}"))
        }
        HubErrorKind::NetworkUnavailable => {
            AppCoreError::BackendNotReady(format!("{context}: {message}"))
        }
        HubErrorKind::OperationFailed => AppCoreError::Internal(format!("{context}: {message}")),
    }
}

pub(super) fn validate_path(label: &str, path: &str) -> Result<(), AppCoreError> {
    if path.is_empty() {
        return Err(AppCoreError::BadRequest(format!("{label} must not be empty")));
    }
    if !std::path::Path::new(path).is_absolute() {
        return Err(AppCoreError::BadRequest(format!(
            "{label} must be an absolute path (got: {path})"
        )));
    }
    let has_traversal = std::path::Path::new(path)
        .components()
        .any(|component| component == std::path::Component::ParentDir);
    if has_traversal {
        return Err(AppCoreError::BadRequest(format!("{label} must not contain '..' components")));
    }
    Ok(())
}

pub(super) fn validate_existing_model_file(path: &str) -> Result<(), AppCoreError> {
    let model_path = std::path::Path::new(path);
    if !model_path.exists() {
        return Err(AppCoreError::BadRequest(format!("model_path does not exist: {path}")));
    }
    if !model_path.is_file() {
        return Err(AppCoreError::BadRequest(format!("model_path is not a file: {path}")));
    }
    Ok(())
}

pub(super) fn model_to_record(model: &UnifiedModel) -> Result<UnifiedModelRecord, AppCoreError> {
    let spec_json = serde_json::to_string(&model.spec)
        .map_err(|error| AppCoreError::Internal(format!("failed to serialize spec: {error}")))?;
    let capabilities_json = serde_json::to_string(&model.capabilities).map_err(|error| {
        AppCoreError::Internal(format!("failed to serialize capabilities: {error}"))
    })?;
    let runtime_presets_json =
        model.runtime_presets.as_ref().map(serde_json::to_string).transpose().map_err(|error| {
            AppCoreError::Internal(format!("failed to serialize runtime_presets: {error}"))
        })?;
    let materialized_artifacts_json = serde_json::to_string(&model.materialized_artifacts)
        .map_err(|error| {
            AppCoreError::Internal(format!("failed to serialize materialized_artifacts: {error}"))
        })?;
    let selected_download_source_json = model
        .selected_download_source
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| {
            AppCoreError::Internal(format!("failed to serialize selected_download_source: {error}"))
        })?;

    Ok(UnifiedModelRecord {
        id: model.id.clone(),
        display_name: model.display_name.clone(),
        kind: model.kind.as_str().to_owned(),
        backend_id: model.backend_id.map(|backend_id| backend_id.to_string()),
        capabilities: capabilities_json,
        status: model.status.as_str().to_owned(),
        spec: spec_json,
        runtime_presets: runtime_presets_json,
        materialized_artifacts: materialized_artifacts_json,
        selected_download_source: selected_download_source_json,
        config_schema_version: CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION as i64,
        config_policy_version: CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION as i64,
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
}

#[cfg(test)]
mod model_catalog_tests {
    use crate::domain::models::{
        ChatModelSource, ListModelsFilter, ModelSpec, UnifiedModelStatus, UpdateModelCommand,
    };
    use crate::infra::db::ModelStore;
    use crate::test_support::{
        TEST_FILENAME, TEST_HUB_PROVIDER, TEST_PROVIDER_ID, TEST_REPO_ID, TestAppCore,
        cloud_chat_model_command, downloadable_llama_command, ready_local_llama_command,
    };

    #[tokio::test]
    async fn model_catalog_create_get_list_and_persists_pack() {
        let app = TestAppCore::new().await;
        let mut command = downloadable_llama_command("catalog-local");
        command.display_name = " Catalog Local ".to_owned();

        let created = app.model.create_model(command).await.expect("create model");

        assert_eq!(created.id, "catalog-local");
        assert_eq!(created.display_name, "Catalog Local");
        assert_eq!(created.status, UnifiedModelStatus::NotDownloaded);
        assert_eq!(created.spec.hub_provider.as_deref(), Some(TEST_HUB_PROVIDER));
        assert!(app.model_pack_path(&created.id).is_file());

        let fetched = app.model.get_model(&created.id).await.expect("get model");
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.spec.repo_id.as_deref(), Some(TEST_REPO_ID));

        let listed = app.model.list_models(ListModelsFilter::default()).await.expect("list models");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);
        assert!(app.runtime.loads().is_empty());
        assert!(app.runtime.unloads().is_empty());
    }

    #[tokio::test]
    async fn model_catalog_update_preserves_same_source_download_state_and_clears_changed_source() {
        let app = TestAppCore::new().await;
        let created =
            app.model.create_model(downloadable_llama_command("catalog-update")).await.unwrap();
        let local_path = app.model_cache_dir.join(TEST_FILENAME).to_string_lossy().into_owned();
        app.seed_downloaded_model_state(&created.id, &local_path).await;

        let same_source = app
            .model
            .update_model(
                &created.id,
                UpdateModelCommand {
                    display_name: Some("Renamed Local".to_owned()),
                    kind: None,
                    backend_id: None,
                    capabilities: None,
                    status: None,
                    spec: None,
                    runtime_presets: None,
                },
            )
            .await
            .expect("update same source");

        assert_eq!(same_source.display_name, "Renamed Local");
        assert_eq!(same_source.spec.local_path.as_deref(), Some(local_path.as_str()));
        assert!(!same_source.materialized_artifacts.is_empty());
        assert_eq!(
            same_source.selected_download_source.as_ref().map(|source| source.repo_id.as_str()),
            Some(TEST_REPO_ID)
        );

        let changed_source = app
            .model
            .update_model(
                &created.id,
                UpdateModelCommand {
                    display_name: None,
                    kind: None,
                    backend_id: None,
                    capabilities: None,
                    status: Some(UnifiedModelStatus::NotDownloaded),
                    spec: Some(ModelSpec {
                        repo_id: Some("slab/other-llama".to_owned()),
                        hub_provider: Some(TEST_HUB_PROVIDER.to_owned()),
                        filename: Some("other-model.gguf".to_owned()),
                        ..ModelSpec::default()
                    }),
                    runtime_presets: None,
                },
            )
            .await
            .expect("update changed source");

        assert!(changed_source.spec.local_path.is_none());
        assert!(changed_source.materialized_artifacts.is_empty());
        assert!(changed_source.selected_download_source.is_none());
        assert_eq!(changed_source.spec.repo_id.as_deref(), Some("slab/other-llama"));
    }

    #[tokio::test]
    async fn model_catalog_delete_removes_db_row_and_persisted_pack() {
        let app = TestAppCore::new().await;
        let created =
            app.model.create_model(downloadable_llama_command("catalog-delete")).await.unwrap();
        let pack_path = app.model_pack_path(&created.id);
        assert!(pack_path.is_file());

        let deleted = app.model.delete_model(&created.id).await.expect("delete model");

        assert_eq!(deleted.id, created.id);
        assert!(!pack_path.exists());
        assert!(app.store.get_model(&created.id).await.expect("get deleted model").is_none());
    }

    #[tokio::test]
    async fn model_catalog_list_chat_models_filters_by_capability_and_provider() {
        let app = TestAppCore::new().await;
        let model_path = app.model_cache_dir.join("ready-chat.gguf");
        std::fs::write(&model_path, []).expect("write local model fixture");
        app.model
            .create_model(ready_local_llama_command("local-chat", &model_path))
            .await
            .expect("create local chat model");
        app.model
            .create_model(cloud_chat_model_command("cloud-chat", TEST_PROVIDER_ID))
            .await
            .expect("create cloud chat model");
        app.model
            .create_model(cloud_chat_model_command("cloud-missing-provider", "missing-provider"))
            .await
            .expect("create unknown-provider cloud model");

        let options = app.model.list_chat_models().await.expect("list chat models");
        let ids = options.iter().map(|option| option.id.as_str()).collect::<Vec<_>>();

        assert!(ids.contains(&"local-chat"));
        assert!(ids.contains(&"cloud-chat"));
        assert!(!ids.contains(&"cloud-missing-provider"));

        let local = options.iter().find(|option| option.id == "local-chat").unwrap();
        assert!(local.downloaded);
        assert!(!local.pending);
        assert_eq!(local.source, ChatModelSource::Local);

        let cloud = options.iter().find(|option| option.id == "cloud-chat").unwrap();
        assert!(cloud.downloaded);
        assert_eq!(cloud.source, ChatModelSource::Cloud);
        assert_eq!(cloud.provider_id.as_deref(), Some(TEST_PROVIDER_ID));
    }
}
