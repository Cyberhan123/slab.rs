use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use futures::StreamExt;
use futures::stream::{self, BoxStream};
use slab_config::{
    LaunchProfile, PluginJsRuntimeTransport, PluginPythonRuntimeTransport, ProviderDefaultsConfig,
    ProviderFamily, ProviderRegistryEntry, ResolvedLaunchSpec, ResolvedRuntimeEndpoints,
    RuntimeTransportMode, SettingsDocument,
};
use slab_types::{Capability, RuntimeBackendId, RuntimeBackendLoadSpec, sqlite_url_for_path};
use tempfile::TempDir;

use crate::config::Config;
use crate::context::{ModelState, OperationManager, WorkerState};
use crate::domain::models::{
    CreateModelCommand, ManagedModelBackendId, ModelSpec, TaskStatus, UnifiedModelKind,
    UnifiedModelStatus,
};
use crate::domain::ports::{
    RuntimeBackendStatus, RuntimeDiffusionImageRequest, RuntimeDiffusionImageResult,
    RuntimeDiffusionVideoRequest, RuntimeDiffusionVideoResult, RuntimeInferenceGateway,
    RuntimeTextGenerationChunk, RuntimeTextGenerationRequest, RuntimeTextGenerationResponse,
    RuntimeTranscriptionRequest, RuntimeTranscriptionResult,
};
use crate::domain::services::{ModelService, PmidService};
use crate::error::AppCoreError;
use crate::infra::db::{AnyStore, ModelDownloadRecord, ModelDownloadStore, ModelStore, TaskRecord};
use crate::infra::rpc::gateway::GrpcGateway;
use crate::model_auto_unload::ModelAutoUnloadManager;
use crate::runtime_supervisor::RuntimeSupervisorStatus;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

pub(crate) const TEST_PROVIDER_ID: &str = "openai-main";
pub(crate) const TEST_REPO_ID: &str = "slab/test-llama";
pub(crate) const TEST_FILENAME: &str = "test-model.gguf";
pub(crate) const TEST_HUB_PROVIDER: &str = "hf_hub";

pub(crate) async fn migrated_test_store() -> AnyStore {
    let options = sqlx::sqlite::SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("sqlite test url")
        .foreign_keys(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connect migrated in-memory db");
    sqlx::migrate!("./migrations").run(&pool).await.expect("run migrations");
    AnyStore { pool }
}

pub(crate) async fn migrated_test_pool() -> sqlx::Pool<sqlx::Sqlite> {
    migrated_test_store().await.pool
}

#[derive(Debug, Default)]
pub(crate) struct RecordingRuntimeGateway {
    available_backends: Mutex<HashSet<RuntimeBackendId>>,
    loads: Mutex<Vec<RuntimeBackendLoadSpec>>,
    unloads: Mutex<Vec<RuntimeBackendId>>,
}

impl RecordingRuntimeGateway {
    pub(crate) fn allow_backend(&self, backend_id: RuntimeBackendId) {
        self.available_backends
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(backend_id);
    }

    pub(crate) fn loads(&self) -> Vec<RuntimeBackendLoadSpec> {
        self.loads.lock().unwrap_or_else(|error| error.into_inner()).clone()
    }

    pub(crate) fn unloads(&self) -> Vec<RuntimeBackendId> {
        self.unloads.lock().unwrap_or_else(|error| error.into_inner()).clone()
    }

    fn unavailable() -> AppCoreError {
        AppCoreError::BackendNotReady("test runtime gateway is unavailable".to_owned())
    }
}

#[async_trait]
impl RuntimeInferenceGateway for RecordingRuntimeGateway {
    fn backend_available(&self, backend_id: RuntimeBackendId) -> bool {
        self.available_backends
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .contains(&backend_id)
    }

    async fn chat(
        &self,
        _request: RuntimeTextGenerationRequest,
    ) -> Result<RuntimeTextGenerationResponse, AppCoreError> {
        Err(Self::unavailable())
    }

    async fn chat_stream(
        &self,
        _request: RuntimeTextGenerationRequest,
    ) -> Result<BoxStream<'static, Result<RuntimeTextGenerationChunk, AppCoreError>>, AppCoreError>
    {
        Ok(stream::empty().boxed())
    }

    async fn transcribe(
        &self,
        _request: RuntimeTranscriptionRequest,
    ) -> Result<RuntimeTranscriptionResult, AppCoreError> {
        Err(Self::unavailable())
    }

    async fn generate_image(
        &self,
        _request: RuntimeDiffusionImageRequest,
    ) -> Result<RuntimeDiffusionImageResult, AppCoreError> {
        Err(Self::unavailable())
    }

    async fn generate_video(
        &self,
        _request: RuntimeDiffusionVideoRequest,
    ) -> Result<RuntimeDiffusionVideoResult, AppCoreError> {
        Err(Self::unavailable())
    }

    async fn load_model(
        &self,
        spec: &RuntimeBackendLoadSpec,
    ) -> Result<RuntimeBackendStatus, AppCoreError> {
        self.loads.lock().unwrap_or_else(|error| error.into_inner()).push(spec.clone());
        Ok(RuntimeBackendStatus {
            backend: spec.backend(),
            status: "ready".to_owned(),
            context_length: None,
            training_context_length: None,
        })
    }

    async fn unload_model(
        &self,
        backend_id: RuntimeBackendId,
    ) -> Result<RuntimeBackendStatus, AppCoreError> {
        self.unloads.lock().unwrap_or_else(|error| error.into_inner()).push(backend_id);
        Ok(RuntimeBackendStatus {
            backend: backend_id,
            status: "unloaded".to_owned(),
            context_length: None,
            training_context_length: None,
        })
    }
}

pub(crate) struct TestAppCore {
    _temp_dir: TempDir,
    _config: Arc<Config>,
    _pmid: Arc<PmidService>,
    pub(crate) store: Arc<AnyStore>,
    pub(crate) runtime: Arc<RecordingRuntimeGateway>,
    pub(crate) auto_unload: Arc<ModelAutoUnloadManager>,
    pub(crate) model: ModelService,
    pub(crate) model_config_dir: PathBuf,
    pub(crate) model_cache_dir: PathBuf,
}

impl TestAppCore {
    pub(crate) async fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("test app-core temp dir");
        let root = temp_dir.path();
        let settings_dir = root.join("config");
        let model_config_dir = settings_dir.join("models");
        let model_cache_dir = root.join("model-cache");
        let plugins_dir = root.join("plugins");
        let session_state_dir = root.join("sessions");
        let exec_rules_dir = root.join("rules");
        let runtime_log_dir = root.join("runtime-logs");

        for dir in [
            &settings_dir,
            &model_config_dir,
            &model_cache_dir,
            &plugins_dir,
            &session_state_dir,
            &exec_rules_dir,
            &runtime_log_dir,
        ] {
            std::fs::create_dir_all(dir).expect("create test support directory");
        }

        let settings_path = settings_dir.join("settings.json");
        let database_url = sqlite_url_for_path(&root.join("slab.db"));
        write_test_settings(&settings_path, &model_cache_dir, &model_config_dir, &plugins_dir);

        let pmid = Arc::new(
            PmidService::load_from_path(settings_path.clone()).await.expect("load test settings"),
        );
        let store =
            Arc::new(AnyStore::connect(&database_url).await.expect("connect migrated test DB"));
        let config = Arc::new(Config {
            bind_address: "127.0.0.1:0".to_owned(),
            database_url,
            log_level: "warn".to_owned(),
            log_json: false,
            log_file: None,
            cloud_http_trace: false,
            queue_capacity: 64,
            backend_capacity: 4,
            enable_swagger: false,
            cors_allowed_origins: None,
            admin_api_token: None,
            transport_mode: "http".to_owned(),
            llama_grpc_endpoint: None,
            whisper_grpc_endpoint: None,
            diffusion_grpc_endpoint: None,
            candle_llama_grpc_endpoint: None,
            candle_whisper_grpc_endpoint: None,
            candle_diffusion_grpc_endpoint: None,
            lib_dir: None,
            session_state_dir: session_state_dir.to_string_lossy().into_owned(),
            settings_path,
            settings_overlay_path: None,
            workspace_root: None,
            model_config_dir: model_config_dir.clone(),
            plugins_dir,
            exec_rules_dir,
            plugin_js_runtime_transport: PluginJsRuntimeTransport::default(),
            plugin_python_runtime_transport: PluginPythonRuntimeTransport::default(),
        });

        let grpc = Arc::new(GrpcGateway::default());
        let runtime = Arc::new(RecordingRuntimeGateway::default());
        let runtime_port: Arc<dyn RuntimeInferenceGateway> = runtime.clone();
        let launch_spec = disabled_launch_spec(&runtime_log_dir);
        let runtime_status = Arc::new(RuntimeSupervisorStatus::from_launch_spec(&launch_spec));
        let auto_unload = Arc::new(ModelAutoUnloadManager::new(
            Arc::clone(&pmid),
            Arc::clone(&runtime_port),
            Arc::clone(&runtime_status),
        ));
        let model_state = ModelState::new(
            Arc::clone(&config),
            Arc::clone(&pmid),
            Arc::clone(&store),
            Arc::clone(&grpc),
            Arc::clone(&runtime_port),
            Arc::clone(&runtime_status),
            Arc::clone(&auto_unload),
        );
        let worker_state = WorkerState::new(
            Arc::clone(&config),
            Arc::clone(&store),
            grpc,
            runtime_port,
            runtime_status,
            Arc::clone(&auto_unload),
            Arc::new(OperationManager::new()),
        );
        let model = ModelService::new(model_state, worker_state);

        Self {
            _temp_dir: temp_dir,
            _config: config,
            _pmid: pmid,
            store,
            runtime,
            auto_unload,
            model,
            model_config_dir,
            model_cache_dir,
        }
    }

    pub(crate) fn model_pack_path(&self, id: &str) -> PathBuf {
        crate::infra::model_packs::model_pack_file_path(&self.model_config_dir, id)
    }

    pub(crate) fn write_model_file(&self, filename: &str) -> PathBuf {
        let path = self.model_cache_dir.join(filename);
        std::fs::write(&path, b"test model").expect("write test model file");
        path
    }

    pub(crate) async fn seed_model_download(
        &self,
        task_id: &str,
        model_id: &str,
        repo_id: &str,
        filename: &str,
        hub_provider: Option<&str>,
        status: TaskStatus,
    ) -> String {
        let source_key = model_download_source_key(hub_provider, repo_id, filename);
        let now = Utc::now();
        self.store
            .insert_model_download_operation(
                TaskRecord {
                    id: task_id.to_owned(),
                    task_type: "model_download".to_owned(),
                    status,
                    model_id: Some(model_id.to_owned()),
                    input_data: None,
                    result_data: None,
                    error_msg: if status == TaskStatus::Failed {
                        Some("download failed".to_owned())
                    } else {
                        None
                    },
                    core_task_id: None,
                    created_at: now,
                    updated_at: now,
                },
                ModelDownloadRecord {
                    task_id: task_id.to_owned(),
                    model_id: model_id.to_owned(),
                    source_key: source_key.clone(),
                    repo_id: repo_id.to_owned(),
                    filename: filename.to_owned(),
                    hub_provider: hub_provider.map(str::to_owned),
                    status,
                    error_msg: if status == TaskStatus::Failed {
                        Some("download failed".to_owned())
                    } else {
                        None
                    },
                    created_at: now,
                    updated_at: now,
                },
            )
            .await
            .expect("seed model download");
        source_key
    }

    pub(crate) async fn seed_downloaded_model_state(&self, model_id: &str, local_path: &str) {
        let artifacts =
            serde_json::to_string(&BTreeMap::from([("model".to_owned(), local_path.to_owned())]))
                .expect("serialize materialized artifacts");
        let selected_source = serde_json::json!({
            "source_key": model_download_source_key(Some(TEST_HUB_PROVIDER), TEST_REPO_ID, TEST_FILENAME),
            "repo_id": TEST_REPO_ID,
            "filename": TEST_FILENAME,
            "hub_provider": TEST_HUB_PROVIDER
        })
        .to_string();

        self.store
            .update_model_download_state(
                model_id,
                local_path,
                UnifiedModelStatus::Ready.as_str(),
                &artifacts,
                Some(&selected_source),
            )
            .await
            .expect("seed downloaded model state");
    }
}

pub(crate) fn local_llama_command(id: &str) -> CreateModelCommand {
    CreateModelCommand {
        id: Some(id.to_owned()),
        display_name: id.to_owned(),
        kind: UnifiedModelKind::Local,
        backend_id: Some(ManagedModelBackendId::GgmlLlama),
        capabilities: None,
        status: None,
        spec: ModelSpec::default(),
        runtime_presets: None,
    }
}

pub(crate) fn downloadable_llama_command(id: &str) -> CreateModelCommand {
    let mut command = local_llama_command(id);
    command.spec = ModelSpec {
        repo_id: Some(TEST_REPO_ID.to_owned()),
        hub_provider: Some("hf".to_owned()),
        filename: Some(TEST_FILENAME.to_owned()),
        ..ModelSpec::default()
    };
    command
}

pub(crate) fn ready_local_llama_command(id: &str, path: &Path) -> CreateModelCommand {
    let mut command = local_llama_command(id);
    command.status = Some(UnifiedModelStatus::Ready);
    command.spec.local_path = Some(path.to_string_lossy().into_owned());
    command
}

pub(crate) fn cloud_chat_model_command(id: &str, provider_id: &str) -> CreateModelCommand {
    CreateModelCommand {
        id: Some(id.to_owned()),
        display_name: id.to_owned(),
        kind: UnifiedModelKind::Cloud,
        backend_id: None,
        capabilities: Some(vec![Capability::TextGeneration, Capability::ChatGeneration]),
        status: None,
        spec: ModelSpec {
            provider_id: Some(provider_id.to_owned()),
            remote_model_id: Some("gpt-4.1-mini".to_owned()),
            ..ModelSpec::default()
        },
        runtime_presets: None,
    }
}

pub(crate) fn cloud_model_pack_bytes(id: &str) -> Vec<u8> {
    build_pack_bytes(vec![(
        "manifest.json",
        serde_json::json!({
            "schema_version": 3,
            "deployment": "cloud",
            "id": id,
            "label": id,
            "family": "llama",
            "capabilities": ["text_generation", "chat_generation"],
            "cloud": {
                "provider_id": TEST_PROVIDER_ID,
                "remote_model_id": "gpt-4.1-mini"
            }
        })
        .to_string(),
    )])
}

pub(crate) fn build_pack_bytes(entries: Vec<(&str, String)>) -> Vec<u8> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(&mut cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    for (path, content) in entries {
        writer.start_file(path, options).expect("start pack entry");
        writer.write_all(content.as_bytes()).expect("write pack entry");
    }

    writer.finish().expect("finish pack archive");
    cursor.into_inner()
}

pub(crate) fn model_download_source_key(
    hub_provider: Option<&str>,
    repo_id: &str,
    filename: &str,
) -> String {
    format!(
        "{}::{}::{}",
        source_key_hub_provider_segment(hub_provider),
        repo_id.trim(),
        filename.trim()
    )
}

fn write_test_settings(
    settings_path: &Path,
    model_cache_dir: &Path,
    model_config_dir: &Path,
    plugins_dir: &Path,
) {
    let mut document = SettingsDocument::default();
    document.models.cache_dir = Some(model_cache_dir.to_string_lossy().into_owned());
    document.models.config_dir = Some(model_config_dir.to_string_lossy().into_owned());
    document.plugin.install_dir = Some(plugins_dir.to_string_lossy().into_owned());
    document.providers.registry.push(ProviderRegistryEntry {
        id: TEST_PROVIDER_ID.to_owned(),
        family: ProviderFamily::OpenaiCompatible,
        display_name: "OpenAI Test".to_owned(),
        api_base: "https://api.openai.test/v1".to_owned(),
        auth: Default::default(),
        defaults: ProviderDefaultsConfig::default(),
    });

    let raw = serde_json::to_string_pretty(&document).expect("serialize test settings");
    std::fs::write(settings_path, format!("{raw}\n")).expect("write test settings");
}

fn disabled_launch_spec(runtime_log_dir: &Path) -> ResolvedLaunchSpec {
    ResolvedLaunchSpec {
        profile: LaunchProfile::Server,
        transport: RuntimeTransportMode::Http,
        runtime_log_dir: runtime_log_dir.to_owned(),
        runtime_ipc_dir: None,
        extra_dirs: Vec::new(),
        children: Vec::new(),
        endpoints: ResolvedRuntimeEndpoints::default(),
        gateway: None,
    }
}

fn source_key_hub_provider_segment(hub_provider: Option<&str>) -> String {
    match normalized_source_key_hub_provider(hub_provider).as_deref() {
        Some("hf_hub") => "hugging_face".to_owned(),
        Some("models_cat") => "model_scope".to_owned(),
        Some(other) => other.to_owned(),
        None => "auto".to_owned(),
    }
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
