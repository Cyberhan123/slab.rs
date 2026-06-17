use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use crate::{
    ChatConfig, CloudProviderConfig, DesktopLaunchProfileConfig, DiffusionConfig,
    DiffusionPerformanceConfig, LaunchBackendConfig, LaunchBackendsConfig, LaunchConfig,
    LaunchProfilesConfig, ModelDownloadSourcePreference, PMID, PmidConfig, ProviderRegistryEntry,
    RuntimeConfig, RuntimeLlamaConfig, RuntimeModelAutoUnloadConfig, RuntimeWhisperConfig,
    RuntimeWorkerConfig, ServerLaunchProfileConfig, SettingsDocument, SetupBackendsConfig,
    SetupConfig, SetupFfmpegConfig, mcp_servers_json_schema, provider_registry_json_schema,
    string_list_json_schema, websearch_providers_json_schema,
};
use serde_json::{Value, json};
use slab_types::{I18nMessageRef, I18nPayload, ServerI18nKey};
use tracing::warn;

use crate::descriptor::setting_value;
use crate::{
    ConfigError, LaunchHostPaths, LaunchProfile, ResolvedLaunchSpec, SettingsDocumentProvider,
};
use crate::{
    SettingPropertySchema, SettingPropertyView, SettingValue, SettingValueType,
    SettingsDocumentView, SettingsSectionView, SettingsSubsectionView, UpdateSettingCommand,
    UpdateSettingOperation,
};

const DEFAULT_SERVER_RUNTIME_BIND_HOST: &str = "127.0.0.1";
const DEFAULT_SERVER_RUNTIME_BASE_PORT: u32 = 3001;
const DEFAULT_DESKTOP_RUNTIME_BIND_HOST: &str = "127.0.0.1";
const DEFAULT_DESKTOP_RUNTIME_BASE_PORT: u32 = 50051;
const SECRET_PLACEHOLDER: &str = "[REDACTED_SECRET]";

#[derive(Debug, Clone)]
pub struct PmidService {
    settings: Arc<SettingsDocumentProvider>,
    config: Arc<RwLock<PmidConfig>>,
}

impl PmidService {
    pub async fn load_from_path(path: PathBuf) -> Result<Self, ConfigError> {
        Self::load_from_paths(path, None).await
    }

    pub async fn load_from_paths(
        path: PathBuf,
        overlay_path: Option<PathBuf>,
    ) -> Result<Self, ConfigError> {
        let settings =
            Arc::new(SettingsDocumentProvider::load_with_overlay(path, overlay_path).await?);
        let config = load_config(&settings.document().await);
        Ok(Self { settings, config: Arc::new(RwLock::new(config)) })
    }

    pub fn config(&self) -> PmidConfig {
        self.config.read().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
    }

    pub fn spawn_periodic_refresh(self: &Arc<Self>, interval: Duration) {
        let service = Arc::clone(self);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                if let Err(error) = service.refresh().await {
                    warn!(%error, "failed to refresh settings from disk");
                }
            }
        });
    }

    pub async fn resolve_launch_spec(
        &self,
        profile: LaunchProfile,
        host_paths: &LaunchHostPaths,
    ) -> Result<ResolvedLaunchSpec, ConfigError> {
        crate::launch::resolve_launch_spec(&self.settings.document().await, profile, host_paths)
    }

    pub async fn document(&self) -> SettingsDocumentView {
        build_document_view(&self.settings).await.unwrap_or_else(|error| SettingsDocumentView {
            schema_version: SettingsDocument::default().schema_version,
            settings_path: self.settings.path().display().to_string(),
            warnings: vec![format!("Failed to build settings view: {error}")],
            sections: Vec::new(),
        })
    }

    pub async fn property(&self, pmid: &str) -> Result<SettingPropertyView, ConfigError> {
        build_property_view(&self.settings, pmid).await
    }

    pub async fn refresh(&self) -> Result<PmidConfig, ConfigError> {
        let next = load_config(&self.settings.document().await);
        *self.config.write().unwrap_or_else(|poisoned| poisoned.into_inner()) = next.clone();
        Ok(next)
    }

    pub async fn update_setting(
        &self,
        pmid: impl AsRef<str>,
        command: UpdateSettingCommand,
    ) -> Result<SettingPropertyView, ConfigError> {
        let pmid = pmid.as_ref();
        let UpdateSettingCommand { op, value } = command;
        let command = match (op, value) {
            (UpdateSettingOperation::Set, Some(value)) if secret(pmid) => {
                let current_value = self.settings.value(pmid).await?;
                UpdateSettingCommand {
                    op: UpdateSettingOperation::Set,
                    value: Some(restore_secret_placeholders(pmid, value, Some(&current_value))),
                }
            }
            (op, value) => UpdateSettingCommand { op, value },
        };
        self.settings.update(pmid, command).await?;
        self.refresh().await?;
        self.property(pmid).await
    }

    pub async fn model_download_source_preference(
        &self,
    ) -> Result<ModelDownloadSourcePreference, ConfigError> {
        let value = self.settings.value(PMID.models.download_source().as_str()).await?;
        serde_json::from_value(value.into_json_value()).map_err(|error| {
            ConfigError::Internal(format!("invalid models.download_source setting value: {error}"))
        })
    }
}

fn load_config(settings: &SettingsDocument) -> PmidConfig {
    PmidConfig {
        logging: settings.logging.clone(),
        telemetry: settings.telemetry.clone(),
        setup: SetupConfig {
            initialized: false,
            ffmpeg: SetupFfmpegConfig {
                auto_download: settings.tools.ffmpeg.auto_download,
                dir: normalize_string(settings.tools.ffmpeg.install_dir.clone()),
            },
            backends: SetupBackendsConfig {
                dir: normalize_string(settings.runtime.ggml.install_dir.clone()),
            },
        },
        agent: settings.agent.clone(),
        runtime: RuntimeConfig {
            model_cache_dir: normalize_string(settings.models.cache_dir.clone()),
            llama: RuntimeLlamaConfig {
                num_workers: resolve_backend_concurrency(settings, RuntimeBackend::Llama),
                context_length: settings.runtime.ggml.backends.llama.context_length,
                flash_attn: settings.runtime.ggml.backends.llama.flash_attn,
            },
            whisper: RuntimeWhisperConfig {
                num_workers: resolve_backend_concurrency(settings, RuntimeBackend::Whisper),
                flash_attn: settings.runtime.ggml.backends.whisper.flash_attn,
            },
            diffusion: RuntimeWorkerConfig {
                num_workers: resolve_backend_concurrency(settings, RuntimeBackend::Diffusion),
            },
            model_auto_unload: RuntimeModelAutoUnloadConfig {
                enabled: settings.models.auto_unload.enabled,
                idle_minutes: settings.models.auto_unload.idle_minutes,
                min_free_system_memory_bytes: settings
                    .models
                    .auto_unload
                    .min_free_system_memory_bytes,
                min_free_gpu_memory_bytes: settings.models.auto_unload.min_free_gpu_memory_bytes,
                max_pressure_evictions_per_load: settings
                    .models
                    .auto_unload
                    .max_pressure_evictions_per_load,
            },
        },
        launch: LaunchConfig {
            transport: settings.runtime.transport,
            queue_capacity: settings.runtime.capacity.queue,
            backend_capacity: settings.runtime.capacity.concurrent_requests,
            runtime_ipc_dir: None,
            runtime_log_dir: normalize_string(
                settings.runtime.logging.path.clone().or_else(|| settings.logging.path.clone()),
            ),
            backends: LaunchBackendsConfig {
                llama: LaunchBackendConfig {
                    enabled: settings.runtime.ggml.backends.llama.enabled,
                },
                whisper: LaunchBackendConfig {
                    enabled: settings.runtime.ggml.backends.whisper.enabled,
                },
                diffusion: LaunchBackendConfig {
                    enabled: settings.runtime.ggml.backends.diffusion.enabled,
                },
            },
            profiles: LaunchProfilesConfig {
                server: ServerLaunchProfileConfig {
                    gateway_bind: settings.server.address.clone(),
                    runtime_bind_host: DEFAULT_SERVER_RUNTIME_BIND_HOST.to_owned(),
                    runtime_bind_base_port: DEFAULT_SERVER_RUNTIME_BASE_PORT,
                },
                desktop: DesktopLaunchProfileConfig {
                    runtime_bind_host: DEFAULT_DESKTOP_RUNTIME_BIND_HOST.to_owned(),
                    runtime_bind_base_port: DEFAULT_DESKTOP_RUNTIME_BASE_PORT,
                },
            },
        },
        chat: ChatConfig {
            providers: settings
                .providers
                .registry
                .iter()
                .map(provider_registry_entry_to_cloud_provider)
                .collect(),
        },
        diffusion: DiffusionConfig {
            performance: DiffusionPerformanceConfig {
                flash_attn: settings.runtime.ggml.backends.diffusion.flash_attn,
                ..Default::default()
            },
            ..Default::default()
        },
    }
}

async fn build_document_view(
    settings: &SettingsDocumentProvider,
) -> Result<SettingsDocumentView, ConfigError> {
    let current = settings.document().await;
    let default_document = settings.default_document();
    let mut sections = empty_sections();

    for pmid in PMID.all() {
        let property =
            build_property_view_from_documents(pmid.as_str(), &current, &default_document)?;
        push_property(&mut sections, property)?;
    }
    attach_settings_i18n(&mut sections);

    Ok(SettingsDocumentView {
        schema_version: current.schema_version,
        settings_path: settings.path().display().to_string(),
        warnings: settings.warnings().await,
        sections,
    })
}

async fn build_property_view(
    settings: &SettingsDocumentProvider,
    pmid: &str,
) -> Result<SettingPropertyView, ConfigError> {
    let current = settings.document().await;
    let default_document = settings.default_document();
    build_property_view_from_documents(pmid, &current, &default_document)
}

fn build_property_view_from_documents(
    pmid: &str,
    current: &SettingsDocument,
    default_document: &SettingsDocument,
) -> Result<SettingPropertyView, ConfigError> {
    let effective_value = setting_value(current, pmid)?;
    let default_value = setting_value(default_document, pmid)?;
    let is_overridden = effective_value != default_value;
    let is_secret = secret(pmid);
    let view_effective_value = if is_secret {
        redact_setting_value(pmid, effective_value.clone())
    } else {
        effective_value.clone()
    };
    let view_default_value = if is_secret {
        redact_setting_value(pmid, default_value.clone())
    } else {
        default_value.clone()
    };
    let view_override_value = is_overridden.then(|| {
        if is_secret {
            redact_setting_value(pmid, effective_value.clone())
        } else {
            effective_value.clone()
        }
    });

    Ok(SettingPropertyView {
        pmid: pmid.to_owned(),
        label: property_label(pmid),
        description_md: property_description(pmid),
        i18n: property_i18n(pmid),
        editable: true,
        schema: SettingPropertySchema {
            value_type: value_type(pmid, &effective_value, &default_value),
            enum_values: enum_values(pmid),
            minimum: minimum_value(pmid),
            maximum: None,
            pattern: None,
            json_schema: json_schema(pmid),
            default_value: view_default_value,
            secret: is_secret,
            multiline: multiline(pmid),
            order: 0,
        },
        effective_value: view_effective_value,
        override_value: view_override_value,
        is_overridden,
        search_terms: search_terms(pmid),
    })
}

fn empty_sections() -> Vec<SettingsSectionView> {
    vec![
        SettingsSectionView {
            id: "general".to_owned(),
            title: "General".to_owned(),
            description_md: "Desktop application preferences shared across the frontend shell."
                .to_owned(),
            i18n: None,
            subsections: vec![SettingsSubsectionView {
                id: "general".to_owned(),
                title: "General".to_owned(),
                description_md:
                    "Choose how the desktop app should present shared interface preferences."
                        .to_owned(),
                i18n: None,
                properties: Vec::new(),
            }],
        },
        SettingsSectionView {
            id: "database".to_owned(),
            title: "Database".to_owned(),
            description_md: "Persistent application data storage and connection settings."
                .to_owned(),
            i18n: None,
            subsections: vec![SettingsSubsectionView {
                id: "general".to_owned(),
                title: "General".to_owned(),
                description_md:
                    "Configure the primary database connection used by the desktop app and server."
                        .to_owned(),
                i18n: None,
                properties: Vec::new(),
            }],
        },
        SettingsSectionView {
            id: "logging".to_owned(),
            title: "Logging".to_owned(),
            description_md:
                "Global logging defaults inherited by runtime workers and server processes."
                    .to_owned(),
            i18n: None,
            subsections: vec![SettingsSubsectionView {
                id: "general".to_owned(),
                title: "General".to_owned(),
                description_md:
                    "Choose the default log level, output format, and optional log directory."
                        .to_owned(),
                i18n: None,
                properties: Vec::new(),
            }],
        },
        SettingsSectionView {
            id: "telemetry".to_owned(),
            title: "Telemetry".to_owned(),
            description_md: "OpenTelemetry export, local telemetry files, and GenAI content capture."
                .to_owned(),
            i18n: None,
            subsections: vec![SettingsSubsectionView {
                id: "general".to_owned(),
                title: "General".to_owned(),
                description_md:
                    "Configure telemetry enablement, local export, and optional remote OTLP targets."
                        .to_owned(),
                i18n: None,
                properties: Vec::new(),
            }],
        },
        SettingsSectionView {
            id: "tools".to_owned(),
            title: "Tools".to_owned(),
            description_md: "External helper binaries managed by the application.".to_owned(),
            i18n: None,
            subsections: vec![SettingsSubsectionView {
                id: "ffmpeg".to_owned(),
                title: "FFmpeg".to_owned(),
                description_md: "Configure FFmpeg installation and download behavior.".to_owned(),
                i18n: None,
                properties: Vec::new(),
            }],
        },
        SettingsSectionView {
            id: "runtime".to_owned(),
            title: "Runtime".to_owned(),
            description_md:
                "Shared inference runtime topology, transport, and backend-specific overrides."
                    .to_owned(),
            i18n: None,
            subsections: vec![
                SettingsSubsectionView {
                    id: "general".to_owned(),
                    title: "General".to_owned(),
                    description_md:
                        "Configure shared transport, session state, and fallback capacity limits."
                            .to_owned(),
                    i18n: None,
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "ggml".to_owned(),
                    title: "GGML".to_owned(),
                    description_md: "Family-level defaults shared by GGML worker processes."
                        .to_owned(),
                    i18n: None,
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "llama".to_owned(),
                    title: "Llama".to_owned(),
                    description_md: "Overrides specific to the GGML llama worker.".to_owned(),
                    i18n: None,
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "whisper".to_owned(),
                    title: "Whisper".to_owned(),
                    description_md: "Overrides specific to the GGML whisper worker.".to_owned(),
                    i18n: None,
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "diffusion".to_owned(),
                    title: "Diffusion".to_owned(),
                    description_md: "Overrides specific to the GGML diffusion worker.".to_owned(),
                    i18n: None,
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "candle".to_owned(),
                    title: "Candle".to_owned(),
                    description_md: "Shared configuration for the Candle runtime family."
                        .to_owned(),
                    i18n: None,
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "onnx".to_owned(),
                    title: "ONNX".to_owned(),
                    description_md: "Shared configuration for the ONNX runtime family.".to_owned(),
                    i18n: None,
                    properties: Vec::new(),
                },
            ],
        },
        SettingsSectionView {
            id: "providers".to_owned(),
            title: "Providers".to_owned(),
            description_md: "Cloud and remote model provider definitions.".to_owned(),
            i18n: None,
            subsections: vec![SettingsSubsectionView {
                id: "registry".to_owned(),
                title: "Registry".to_owned(),
                description_md: "Manage provider endpoints, credentials, and request defaults."
                    .to_owned(),
                i18n: None,
                properties: Vec::new(),
            }],
        },
        SettingsSectionView {
            id: "models".to_owned(),
            title: "Models".to_owned(),
            description_md: "Model cache locations and automatic unload behavior.".to_owned(),
            i18n: None,
            subsections: vec![
                SettingsSubsectionView {
                    id: "general".to_owned(),
                    title: "General".to_owned(),
                    description_md: "Configure model cache and config directory locations."
                        .to_owned(),
                    i18n: None,
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "auto_unload".to_owned(),
                    title: "Auto Unload".to_owned(),
                    description_md: "Unload idle models automatically to reclaim memory."
                        .to_owned(),
                    i18n: None,
                    properties: Vec::new(),
                },
            ],
        },
        SettingsSectionView {
            id: "plugin".to_owned(),
            title: "Plugins".to_owned(),
            description_md: "Runtime plugin installation and registration settings.".to_owned(),
            i18n: None,
            subsections: vec![SettingsSubsectionView {
                id: "general".to_owned(),
                title: "General".to_owned(),
                description_md: "Choose where installed plugin packages are loaded from."
                    .to_owned(),
                i18n: None,
                properties: Vec::new(),
            }],
        },
        SettingsSectionView {
            id: "agent".to_owned(),
            title: "Agent".to_owned(),
            description_md: "Agent tool configuration used by built-in deterministic tools."
                .to_owned(),
            i18n: None,
            subsections: vec![
                SettingsSubsectionView {
                    id: "general".to_owned(),
                    title: "General".to_owned(),
                    description_md: "Agent diagnostics and runtime behavior.".to_owned(),
                    i18n: None,
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "mcp".to_owned(),
                    title: "MCP".to_owned(),
                    description_md: "Control exposure of configured MCP servers as agent tools."
                        .to_owned(),
                    i18n: None,
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "websearch".to_owned(),
                    title: "Web Search".to_owned(),
                    description_md:
                        "Configure the agent web search provider defaults and credentials."
                            .to_owned(),
                    i18n: None,
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "hooks".to_owned(),
                    title: "Hooks".to_owned(),
                    description_md:
                        "Control external plugin and script hooks for agent lifecycle events."
                            .to_owned(),
                    i18n: None,
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "memories".to_owned(),
                    title: "Memories".to_owned(),
                    description_md: "Configure the built-in agent memory pipeline and workspace."
                        .to_owned(),
                    i18n: None,
                    properties: Vec::new(),
                },
            ],
        },
        SettingsSectionView {
            id: "server".to_owned(),
            title: "Server".to_owned(),
            description_md: "HTTP gateway configuration, access control, and API tooling."
                .to_owned(),
            i18n: None,
            subsections: vec![
                SettingsSubsectionView {
                    id: "general".to_owned(),
                    title: "General".to_owned(),
                    description_md:
                        "Configure the gateway bind address and server-side logging behavior."
                            .to_owned(),
                    i18n: None,
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "cors".to_owned(),
                    title: "CORS".to_owned(),
                    description_md: "Allowed browser origins for the HTTP API.".to_owned(),
                    i18n: None,
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "admin".to_owned(),
                    title: "Admin".to_owned(),
                    description_md: "Protect management routes with an optional bearer token."
                        .to_owned(),
                    i18n: None,
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "swagger".to_owned(),
                    title: "Swagger".to_owned(),
                    description_md: "Enable or disable the OpenAPI and Swagger UI endpoints."
                        .to_owned(),
                    i18n: None,
                    properties: Vec::new(),
                },
            ],
        },
    ]
}

fn attach_settings_i18n(sections: &mut [SettingsSectionView]) {
    for section in sections {
        section.i18n = section_i18n(&section.id);
        for subsection in &mut section.subsections {
            subsection.i18n = subsection_i18n(&section.id, &subsection.id);
        }
    }
}

fn section_i18n(section_id: &str) -> Option<I18nPayload> {
    match section_id {
        "general" => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSectionGeneralTitle),
            Some(ServerI18nKey::SettingsSectionGeneralDescription),
        )),
        "database" => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSectionDatabaseTitle),
            Some(ServerI18nKey::SettingsSectionDatabaseDescription),
        )),
        "logging" => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSectionLoggingTitle),
            Some(ServerI18nKey::SettingsSectionLoggingDescription),
        )),
        "telemetry" => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSectionTelemetryTitle),
            Some(ServerI18nKey::SettingsSectionTelemetryDescription),
        )),
        "tools" => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSectionToolsTitle),
            Some(ServerI18nKey::SettingsSectionToolsDescription),
        )),
        "runtime" => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSectionRuntimeTitle),
            Some(ServerI18nKey::SettingsSectionRuntimeDescription),
        )),
        "providers" => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSectionProvidersTitle),
            Some(ServerI18nKey::SettingsSectionProvidersDescription),
        )),
        "models" => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSectionModelsTitle),
            Some(ServerI18nKey::SettingsSectionModelsDescription),
        )),
        "plugin" => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSectionPluginTitle),
            Some(ServerI18nKey::SettingsSectionPluginDescription),
        )),
        "agent" => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSectionAgentTitle),
            Some(ServerI18nKey::SettingsSectionAgentDescription),
        )),
        "server" => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSectionServerTitle),
            Some(ServerI18nKey::SettingsSectionServerDescription),
        )),
        _ => None,
    }
}

fn subsection_i18n(section_id: &str, subsection_id: &str) -> Option<I18nPayload> {
    match (section_id, subsection_id) {
        ("general", "general") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionGeneralGeneralTitle),
            Some(ServerI18nKey::SettingsSubsectionGeneralGeneralDescription),
        )),
        ("database", "general") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionGeneralGeneralTitle),
            Some(ServerI18nKey::SettingsSubsectionDatabaseGeneralDescription),
        )),
        ("logging", "general") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionGeneralGeneralTitle),
            Some(ServerI18nKey::SettingsSubsectionLoggingGeneralDescription),
        )),
        ("telemetry", "general") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionGeneralGeneralTitle),
            Some(ServerI18nKey::SettingsSubsectionTelemetryGeneralDescription),
        )),
        ("tools", "ffmpeg") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionToolsFfmpegTitle),
            Some(ServerI18nKey::SettingsSubsectionToolsFfmpegDescription),
        )),
        ("runtime", "general") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionGeneralGeneralTitle),
            Some(ServerI18nKey::SettingsSubsectionRuntimeGeneralDescription),
        )),
        ("runtime", "ggml") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionRuntimeGgmlTitle),
            Some(ServerI18nKey::SettingsSubsectionRuntimeGgmlDescription),
        )),
        ("runtime", "llama") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionRuntimeLlamaTitle),
            Some(ServerI18nKey::SettingsSubsectionRuntimeLlamaDescription),
        )),
        ("runtime", "whisper") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionRuntimeWhisperTitle),
            Some(ServerI18nKey::SettingsSubsectionRuntimeWhisperDescription),
        )),
        ("runtime", "diffusion") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionRuntimeDiffusionTitle),
            Some(ServerI18nKey::SettingsSubsectionRuntimeDiffusionDescription),
        )),
        ("runtime", "candle") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionRuntimeCandleTitle),
            Some(ServerI18nKey::SettingsSubsectionRuntimeCandleDescription),
        )),
        ("runtime", "onnx") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionRuntimeOnnxTitle),
            Some(ServerI18nKey::SettingsSubsectionRuntimeOnnxDescription),
        )),
        ("providers", "registry") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionProvidersRegistryTitle),
            Some(ServerI18nKey::SettingsSubsectionProvidersRegistryDescription),
        )),
        ("models", "general") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionGeneralGeneralTitle),
            Some(ServerI18nKey::SettingsSubsectionModelsGeneralDescription),
        )),
        ("models", "auto_unload") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionModelsAutoUnloadTitle),
            Some(ServerI18nKey::SettingsSubsectionModelsAutoUnloadDescription),
        )),
        ("plugin", "general") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionGeneralGeneralTitle),
            Some(ServerI18nKey::SettingsSubsectionPluginGeneralDescription),
        )),
        ("agent", "general") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionGeneralGeneralTitle),
            Some(ServerI18nKey::SettingsSubsectionAgentGeneralDescription),
        )),
        ("agent", "mcp") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionAgentMcpTitle),
            Some(ServerI18nKey::SettingsSubsectionAgentMcpDescription),
        )),
        ("agent", "websearch") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionAgentWebsearchTitle),
            Some(ServerI18nKey::SettingsSubsectionAgentWebsearchDescription),
        )),
        ("agent", "hooks") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionAgentHooksTitle),
            Some(ServerI18nKey::SettingsSubsectionAgentHooksDescription),
        )),
        ("agent", "memories") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionAgentMemoriesTitle),
            Some(ServerI18nKey::SettingsSubsectionAgentMemoriesDescription),
        )),
        ("server", "general") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionGeneralGeneralTitle),
            Some(ServerI18nKey::SettingsSubsectionServerGeneralDescription),
        )),
        ("server", "cors") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionServerCorsTitle),
            Some(ServerI18nKey::SettingsSubsectionServerCorsDescription),
        )),
        ("server", "admin") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionServerAdminTitle),
            Some(ServerI18nKey::SettingsSubsectionServerAdminDescription),
        )),
        ("server", "swagger") => Some(metadata_i18n(
            Some(ServerI18nKey::SettingsSubsectionServerSwaggerTitle),
            Some(ServerI18nKey::SettingsSubsectionServerSwaggerDescription),
        )),
        _ => None,
    }
}

fn property_i18n(path: &str) -> Option<I18nPayload> {
    let label = property_label_key(path);
    let description = property_description_key(path);
    (label.is_some() || description.is_some()).then(|| {
        let mut payload = I18nPayload::new();
        if let Some(key) = label {
            payload.insert("label", I18nMessageRef::new(key));
        }
        if let Some(key) = description {
            payload.insert("description_md", I18nMessageRef::new(key));
        }
        payload
    })
}

fn metadata_i18n(
    title_or_label: Option<ServerI18nKey>,
    description: Option<ServerI18nKey>,
) -> I18nPayload {
    let mut payload = I18nPayload::new();
    if let Some(key) = title_or_label {
        payload.insert("title", I18nMessageRef::new(key));
    }
    if let Some(key) = description {
        payload.insert("description_md", I18nMessageRef::new(key));
    }
    payload
}

fn push_property(
    sections: &mut [SettingsSectionView],
    property: SettingPropertyView,
) -> Result<(), ConfigError> {
    let (section_id, subsection_id) = section_location(&property.pmid);
    let section =
        sections.iter_mut().find(|section| section.id == section_id).ok_or_else(|| {
            ConfigError::Internal(format!("unknown settings section '{}'", section_id))
        })?;
    let subsection = section
        .subsections
        .iter_mut()
        .find(|subsection| subsection.id == subsection_id)
        .ok_or_else(|| {
            ConfigError::Internal(format!(
                "unknown settings subsection '{}.{}'",
                section_id, subsection_id
            ))
        })?;
    subsection.properties.push(property);
    Ok(())
}

fn section_location(path: &str) -> (&'static str, &'static str) {
    match path {
        _ if path.starts_with("general.") => ("general", "general"),
        _ if path.starts_with("database.") => ("database", "general"),
        _ if path.starts_with("logging.") => ("logging", "general"),
        _ if path.starts_with("telemetry.") => ("telemetry", "general"),
        _ if path.starts_with("tools.ffmpeg.") => ("tools", "ffmpeg"),
        "agent.debug" => ("agent", "general"),
        _ if path.starts_with("agent.tools.mcp.") => ("agent", "mcp"),
        _ if path.starts_with("agent.tools.websearch.") => ("agent", "websearch"),
        _ if path.starts_with("agent.hooks.") => ("agent", "hooks"),
        _ if path.starts_with("agent.memories.") => ("agent", "memories"),
        _ if path.starts_with("runtime.ggml.backends.llama.") => ("runtime", "llama"),
        _ if path.starts_with("runtime.ggml.backends.whisper.") => ("runtime", "whisper"),
        _ if path.starts_with("runtime.ggml.backends.diffusion.") => ("runtime", "diffusion"),
        _ if path.starts_with("runtime.ggml.") => ("runtime", "ggml"),
        _ if path.starts_with("runtime.candle.") => ("runtime", "candle"),
        _ if path.starts_with("runtime.onnx.") => ("runtime", "onnx"),
        _ if path.starts_with("runtime.") => ("runtime", "general"),
        _ if path.starts_with("providers.") => ("providers", "registry"),
        _ if path.starts_with("models.auto_unload.") => ("models", "auto_unload"),
        _ if path.starts_with("models.") => ("models", "general"),
        _ if path.starts_with("plugin.") => ("plugin", "general"),
        _ if path.starts_with("server.cors.") => ("server", "cors"),
        _ if path.starts_with("server.admin.") => ("server", "admin"),
        _ if path.starts_with("server.swagger.") => ("server", "swagger"),
        _ if path.starts_with("server.") => ("server", "general"),
        _ => ("server", "general"),
    }
}

fn value_type(path: &str, effective: &SettingValue, default: &SettingValue) -> SettingValueType {
    if path == "providers.registry"
        || path == "server.cors.allowed_origins"
        || path == "agent.hooks.scripts"
    {
        return SettingValueType::Array;
    }
    if path == "agent.tools.websearch.providers"
        || path == "telemetry.metrics_exporter"
        || path == "telemetry.span_attributes"
        || path == "telemetry.tracestate"
    {
        return SettingValueType::Object;
    }
    if path.ends_with(".enabled")
        || path.ends_with(".debug")
        || path.ends_with(".json")
        || path.ends_with(".auto_download")
        || path.ends_with(".flash_attn")
        || path == "telemetry.capture_content"
        || path == "server.cloud_http_trace"
    {
        return SettingValueType::Boolean;
    }
    if path.ends_with(".queue")
        || path.ends_with(".concurrent_requests")
        || path.ends_with(".idle_minutes")
        || path.ends_with(".context_length")
        || path.ends_with("_limit")
        || path.ends_with("_concurrency")
        || path.ends_with("_seconds")
        || path.ends_with("_days")
    {
        return SettingValueType::Integer;
    }

    match effective {
        SettingValue::Boolean(_) => SettingValueType::Boolean,
        SettingValue::Integer(_) | SettingValue::Number(_) => SettingValueType::Integer,
        SettingValue::Array(_) => SettingValueType::Array,
        SettingValue::Object(_) => SettingValueType::Object,
        SettingValue::Null => match default {
            SettingValue::Boolean(_) => SettingValueType::Boolean,
            SettingValue::Integer(_) | SettingValue::Number(_) => SettingValueType::Integer,
            SettingValue::Array(_) => SettingValueType::Array,
            SettingValue::Object(_) => SettingValueType::Object,
            _ => SettingValueType::String,
        },
        _ => SettingValueType::String,
    }
}

fn enum_values(path: &str) -> Option<Vec<String>> {
    match path {
        "general.language" => Some(vec!["auto".to_owned(), "en-US".to_owned(), "zh-CN".to_owned()]),
        "runtime.mode" => {
            Some(vec!["managed_children".to_owned(), "external_endpoints".to_owned()])
        }
        "runtime.transport" => Some(vec!["http".to_owned(), "ipc".to_owned()]),
        "plugin.js_runtime_transport" | "plugin.python_runtime_transport" => {
            Some(vec!["stdio".to_owned(), "uds".to_owned()])
        }
        "agent.tools.mcp.enabled" => None,
        "agent.tools.websearch.default_provider" => Some(vec![
            "duckduckgo".to_owned(),
            "arxiv".to_owned(),
            "google".to_owned(),
            "tavily".to_owned(),
            "exa".to_owned(),
            "serpapi".to_owned(),
            "brave".to_owned(),
            "searxng".to_owned(),
        ]),
        "models.download_source" => {
            Some(vec!["auto".to_owned(), "hugging_face".to_owned(), "model_scope".to_owned()])
        }
        _ => None,
    }
}

fn minimum_value(path: &str) -> Option<i64> {
    if path.ends_with(".queue")
        || path.ends_with(".concurrent_requests")
        || path.ends_with(".idle_minutes")
        || path.ends_with(".context_length")
        || path.ends_with("_limit")
        || path.ends_with("_concurrency")
        || path.ends_with("_seconds")
        || path.ends_with("_days")
    {
        Some(0)
    } else {
        None
    }
}

fn json_schema(path: &str) -> Option<Value> {
    match path {
        "providers.registry" => Some(provider_registry_json_schema()),
        "agent.tools.mcp.servers" => Some(mcp_servers_json_schema()),
        "agent.tools.websearch.providers" => Some(websearch_providers_json_schema()),
        "server.cors.allowed_origins" => Some(string_list_json_schema(
            "Allowed Origins",
            ServerI18nKey::SettingsPropertyLabelAllowedOrigins,
        )),
        "telemetry.span_attributes" => Some(string_map_json_schema(
            "Span Attributes",
            ServerI18nKey::SettingsPropertyLabelSpanAttributes,
        )),
        "telemetry.tracestate" => Some(string_map_json_schema(
            "Trace State",
            ServerI18nKey::SettingsPropertyLabelTraceState,
        )),
        _ => None,
    }
}

fn string_map_json_schema(title: &str, title_key: ServerI18nKey) -> Value {
    json!({
        "type": "object",
        "title": title,
        "x-i18n": metadata_i18n(Some(title_key), None),
        "default": {},
        "additionalProperties": { "type": "string" }
    })
}

fn secret(path: &str) -> bool {
    path == "server.admin.token"
        || path == "providers.registry"
        || path == "agent.tools.websearch.providers"
}

fn redact_setting_value(path: &str, value: SettingValue) -> SettingValue {
    match path {
        "server.admin.token" => redact_secret_leaf(value),
        "providers.registry" | "agent.tools.websearch.providers" => redact_api_key_fields(value),
        _ => value,
    }
}

fn redact_secret_leaf(value: SettingValue) -> SettingValue {
    match value {
        SettingValue::String(value) if !value.is_empty() => {
            SettingValue::String(SECRET_PLACEHOLDER.to_owned())
        }
        other => other,
    }
}

fn redact_api_key_fields(value: SettingValue) -> SettingValue {
    match value {
        SettingValue::Array(values) => {
            SettingValue::Array(values.into_iter().map(redact_api_key_fields).collect())
        }
        SettingValue::Object(values) => SettingValue::Object(
            values
                .into_iter()
                .map(|(key, value)| {
                    let value = if key == "api_key" {
                        redact_secret_leaf(value)
                    } else {
                        redact_api_key_fields(value)
                    };
                    (key, value)
                })
                .collect(),
        ),
        other => other,
    }
}

fn restore_secret_placeholders(
    path: &str,
    requested: SettingValue,
    current: Option<&SettingValue>,
) -> SettingValue {
    match path {
        "server.admin.token" => restore_secret_leaf(requested, current),
        "providers.registry" | "agent.tools.websearch.providers" => {
            restore_api_key_placeholders(requested, current)
        }
        _ => requested,
    }
}

fn restore_secret_leaf(requested: SettingValue, current: Option<&SettingValue>) -> SettingValue {
    match requested {
        SettingValue::String(value) if value == SECRET_PLACEHOLDER => {
            current.cloned().unwrap_or(SettingValue::Null)
        }
        other => other,
    }
}

fn restore_api_key_placeholders(
    requested: SettingValue,
    current: Option<&SettingValue>,
) -> SettingValue {
    match requested {
        SettingValue::Array(values) => SettingValue::Array(
            values
                .into_iter()
                .enumerate()
                .map(|(index, value)| {
                    let current_item = current_array_item_for_request(current, index, &value);
                    restore_api_key_placeholders(value, current_item)
                })
                .collect(),
        ),
        SettingValue::Object(values) => SettingValue::Object(
            values
                .into_iter()
                .map(|(key, value)| {
                    let current_child = current_object_child(current, &key);
                    let value = if key == "api_key" {
                        restore_secret_leaf(value, current_child)
                    } else {
                        restore_api_key_placeholders(value, current_child)
                    };
                    (key, value)
                })
                .collect(),
        ),
        other => other,
    }
}

fn current_object_child<'a>(
    current: Option<&'a SettingValue>,
    key: &str,
) -> Option<&'a SettingValue> {
    match current {
        Some(SettingValue::Object(values)) => values.get(key),
        _ => None,
    }
}

fn current_array_item_for_request<'a>(
    current: Option<&'a SettingValue>,
    index: usize,
    requested: &SettingValue,
) -> Option<&'a SettingValue> {
    let Some(SettingValue::Array(values)) = current else {
        return None;
    };

    setting_object_string(requested, "id")
        .and_then(|id| values.iter().find(|value| setting_object_string(value, "id") == Some(id)))
        .or_else(|| values.get(index))
}

fn setting_object_string<'a>(value: &'a SettingValue, key: &str) -> Option<&'a str> {
    match value {
        SettingValue::Object(values) => match values.get(key) {
            Some(SettingValue::String(value)) => Some(value.as_str()),
            _ => None,
        },
        _ => None,
    }
}

fn multiline(path: &str) -> bool {
    path == "providers.registry"
        || path == "agent.tools.mcp.servers"
        || path == "agent.tools.websearch.providers"
        || path == "agent.hooks.scripts"
        || path == "telemetry.metrics_exporter"
        || path == "telemetry.span_attributes"
        || path == "telemetry.tracestate"
}

fn property_label(path: &str) -> String {
    match path {
        "general.language" => "Interface Language".to_owned(),
        "database.url" => "Database URL".to_owned(),
        "logging.level" => "Log Level".to_owned(),
        "logging.json" => "JSON Logs".to_owned(),
        "logging.path" => "Log Directory".to_owned(),
        "telemetry.enabled" => "Telemetry".to_owned(),
        "telemetry.environment" => "Environment".to_owned(),
        "telemetry.service_name" => "Service Name".to_owned(),
        "telemetry.service_version" => "Service Version".to_owned(),
        "telemetry.metrics_exporter" => "Metrics Exporter".to_owned(),
        "telemetry.capture_content" => "Capture GenAI Content".to_owned(),
        "telemetry.span_attributes" => "Span Attributes".to_owned(),
        "telemetry.tracestate" => "Trace State".to_owned(),
        "runtime.mode" => "Runtime Mode".to_owned(),
        "runtime.transport" => "Transport".to_owned(),
        "runtime.sessions.state_dir" => "Session State Directory".to_owned(),
        "agent.debug" => "Agent Debug Trace".to_owned(),
        "agent.hooks.enabled" => "External Hooks".to_owned(),
        "agent.hooks.scripts" => "Legacy Hook Scripts".to_owned(),
        "agent.memories.enabled" => "Agent Memories".to_owned(),
        "agent.memories.memory_root" => "Memory Root".to_owned(),
        "agent.memories.phase1_scan_limit" => "Phase 1 Scan Limit".to_owned(),
        "agent.memories.phase1_concurrency" => "Phase 1 Concurrency".to_owned(),
        "agent.memories.phase1_idle_seconds" => "Phase 1 Idle Seconds".to_owned(),
        "agent.memories.phase1_lease_seconds" => "Phase 1 Lease Seconds".to_owned(),
        "agent.memories.phase1_retry_seconds" => "Phase 1 Retry Seconds".to_owned(),
        "agent.memories.phase1_max_age_days" => "Phase 1 Max Age Days".to_owned(),
        "agent.memories.phase2_limit" => "Phase 2 Limit".to_owned(),
        "agent.memories.phase2_lease_seconds" => "Phase 2 Lease Seconds".to_owned(),
        "agent.memories.max_unused_days" => "Max Unused Days".to_owned(),
        "agent.memories.extension_retention_days" => "Extension Retention Days".to_owned(),
        "agent.tools.mcp.enabled" => "MCP Tools".to_owned(),
        "agent.tools.mcp.servers" => "MCP Servers".to_owned(),
        "agent.tools.websearch.default_provider" => "Default Provider".to_owned(),
        "agent.tools.websearch.providers" => "Web Search Providers".to_owned(),
        _ if path.ends_with(".flash_attn") => "Flash Attention".to_owned(),
        "providers.registry" => "Provider Registry".to_owned(),
        "models.cache_dir" => "Model Cache Directory".to_owned(),
        "models.config_dir" => "Model Config Directory".to_owned(),
        "models.download_source" => "Model Source".to_owned(),
        "plugin.install_dir" => "Plugin Install Directory".to_owned(),
        "plugin.js_runtime_transport" => "JS Runtime Transport".to_owned(),
        "plugin.python_runtime_transport" => "Python Runtime Transport".to_owned(),
        "server.address" => "Bind Address".to_owned(),
        "server.admin.token" => "Admin Token".to_owned(),
        "server.cors.allowed_origins" => "Allowed Origins".to_owned(),
        "server.cloud_http_trace" => "Cloud HTTP Trace".to_owned(),
        _ => humanize_setting_label(path.rsplit('.').next().unwrap_or(path)),
    }
}

fn property_description(path: &str) -> String {
    match path {
        "general.language" => {
            "Choose how the desktop frontend selects translation resources.".to_owned()
        }
        "database.url" => "SQLx connection string used for the shared application database.".to_owned(),
        "logging.level" => "Default tracing filter inherited by server and runtime processes.".to_owned(),
        "logging.json" => "Emit newline-delimited JSON logs by default.".to_owned(),
        "logging.path" => "Optional directory used for persisted log files.".to_owned(),
        "telemetry.enabled" => {
            "Enable program-managed local telemetry export and session telemetry.".to_owned()
        }
        "telemetry.environment" => "Deployment environment attached to telemetry resources.".to_owned(),
        "telemetry.service_name" => "OpenTelemetry service.name resource value.".to_owned(),
        "telemetry.service_version" => {
            "Optional OpenTelemetry service.version resource value.".to_owned()
        }
        "telemetry.metrics_exporter" => {
            "Metrics exporter. Defaults to none until metrics collection is explicitly enabled.".to_owned()
        }
        "telemetry.capture_content" => {
            "Include GenAI prompt, output, and tool definition content in telemetry events.".to_owned()
        }
        "telemetry.span_attributes" => "Additional attributes attached to telemetry spans.".to_owned(),
        "telemetry.tracestate" => "W3C tracestate entries propagated with trace context.".to_owned(),
        "tools.ffmpeg.enabled" => "Enable FFmpeg integration for media tooling.".to_owned(),
        "tools.ffmpeg.auto_download" => "Download FFmpeg automatically when it is missing.".to_owned(),
        "tools.ffmpeg.install_dir" => "Optional install directory for the FFmpeg sidecar.".to_owned(),
        "agent.debug" => {
            "Write full-fidelity per-session agent trace files for prompt, tool, and runtime debugging.".to_owned()
        }
        "agent.hooks.enabled" => {
            "Enable external agent lifecycle hooks registered by plugins or legacy local script settings. Built-in hooks are unaffected.".to_owned()
        }
        "agent.hooks.scripts" => {
            "Legacy local script hooks executed through the supervised JS/Python runtimes when external hooks are enabled.".to_owned()
        }
        "agent.memories.enabled" => {
            "Enable the built-in agent memory instruction and consolidation pipeline.".to_owned()
        }
        "agent.memories.model" => {
            "Optional model override used by the agent memory pipeline.".to_owned()
        }
        "agent.memories.memory_root" => {
            "Optional filesystem root for generated and consolidated agent memories.".to_owned()
        }
        "agent.memories.phase1_scan_limit" => {
            "Maximum completed root agent threads scanned by each memory phase 1 run.".to_owned()
        }
        "agent.memories.phase1_concurrency" => {
            "Maximum concurrent memory phase 1 extraction tasks.".to_owned()
        }
        "agent.memories.phase1_idle_seconds" => {
            "Minimum idle age before a completed thread becomes eligible for memory phase 1.".to_owned()
        }
        "agent.memories.phase1_lease_seconds" => {
            "Lease duration for memory phase 1 extraction claims.".to_owned()
        }
        "agent.memories.phase1_retry_seconds" => {
            "Retry delay after a memory phase 1 extraction failure.".to_owned()
        }
        "agent.memories.phase1_max_age_days" => {
            "Maximum completed-thread age considered by memory phase 1 extraction.".to_owned()
        }
        "agent.memories.phase2_limit" => {
            "Maximum memory candidates consolidated during a phase 2 run.".to_owned()
        }
        "agent.memories.phase2_lease_seconds" => {
            "Lease duration for memory phase 2 consolidation.".to_owned()
        }
        "agent.memories.max_unused_days" => {
            "Maximum age for unused memories kept in phase 2 selection.".to_owned()
        }
        "agent.memories.extension_retention_days" => {
            "Retention window for extension memory files written by phase 2.".to_owned()
        }
        "agent.tools.mcp.enabled" => {
            "Expose configured MCP tools to the agent tool router. Disabled by default.".to_owned()
        }
        "agent.tools.mcp.servers" => {
            "Persistent stdio MCP server launch configurations used when MCP tools are enabled. Environment values reference host variables instead of storing secrets directly.".to_owned()
        }
        "agent.tools.websearch.default_provider" => {
            "Provider used by the agent web_search tool when the tool call omits provider.".to_owned()
        }
        "agent.tools.websearch.providers" => {
            "Provider-specific credentials and options for the agent web_search tool.".to_owned()
        }
        "runtime.mode" => "Choose whether runtimes are launched as managed child processes or discovered through explicit endpoints.".to_owned(),
        "runtime.transport" => "Transport protocol used between the gateway and runtime workers.".to_owned(),
        "runtime.sessions.state_dir" => "Directory used for persisted runtime-backed session state.".to_owned(),
        "providers.registry" => "Structured list of remote providers, credentials, and request defaults.".to_owned(),
        "models.cache_dir" => "Directory used for cached model artifacts.".to_owned(),
        "models.config_dir" => "Directory scanned for persisted model configuration documents.".to_owned(),
        "models.download_source" => "Preferred remote source used when downloading model artifacts. Auto follows the pack candidate order.".to_owned(),
        "plugin.install_dir" => {
            "Directory used as the plugin installation source for runtime registration. Defaults to the plugins directory next to settings.json.".to_owned()
        },
        "plugin.js_runtime_transport" => {
            "Transport used by slab-app-core when communicating with the JavaScript plugin sidecar runtime.".to_owned()
        }
        "plugin.python_runtime_transport" => {
            "Transport used by slab-app-core when communicating with the Python plugin sidecar runtime.".to_owned()
        }
        "models.auto_unload.enabled" => "Unload idle models automatically to reclaim memory.".to_owned(),
        "models.auto_unload.idle_minutes" => "Idle timeout in minutes before auto-unload triggers.".to_owned(),
        "models.auto_unload.min_free_system_memory_bytes" => {
            "Minimum free system memory to preserve before model loads stop evicting idle models."
                .to_owned()
        }
        "models.auto_unload.min_free_gpu_memory_bytes" => {
            "Minimum free GPU memory to preserve before model loads stop evicting idle models."
                .to_owned()
        }
        "models.auto_unload.max_pressure_evictions_per_load" => {
            "Maximum number of idle models evicted during a single load attempt under memory pressure."
                .to_owned()
        }
        "server.address" => "Bind address for the slab-server HTTP gateway.".to_owned(),
        "server.admin.token" => "Optional bearer token required for management endpoints.".to_owned(),
        "server.cors.allowed_origins" => "List of allowed browser origins for API requests.".to_owned(),
        "server.swagger.enabled" => "Expose the OpenAPI document and Swagger UI.".to_owned(),
        "server.cloud_http_trace" => "Log redacted cloud request and response payloads for debugging.".to_owned(),
        _ if path.ends_with(".enabled") => "Enable or disable this component-specific override.".to_owned(),
        _ if path.ends_with(".flash_attn") => {
            "Enable Flash Attention when the backend supports it.".to_owned()
        }
        _ if path.ends_with(".install_dir") => "Optional install directory override.".to_owned(),
        _ if path.ends_with(".level") => "Override the inherited log level for this scope.".to_owned(),
        _ if path.ends_with(".json") => "Override whether logs are emitted in JSON format for this scope.".to_owned(),
        _ if path.ends_with(".path") => "Optional filesystem path override for this scope.".to_owned(),
        _ if path.ends_with(".queue") => "Maximum queued requests allowed before new submissions wait.".to_owned(),
        _ if path.ends_with(".concurrent_requests") => "Maximum in-flight requests allowed for this runtime scope.".to_owned(),
        _ if path.ends_with(".address") => "Explicit HTTP bind or target address override.".to_owned(),
        _ if path.ends_with(".ipc.path") => "Explicit IPC socket or named-pipe path override.".to_owned(),
        _ if path.ends_with(".version") => "Optional version or release identifier override.".to_owned(),
        _ if path.ends_with(".artifact") => "Optional artifact or asset selector override.".to_owned(),
        _ if path.ends_with(".context_length") => "Override the llama context window length in tokens.".to_owned(),
        _ => String::new(),
    }
}

fn property_label_key(path: &str) -> Option<ServerI18nKey> {
    match path {
        "general.language" => Some(ServerI18nKey::SettingsPropertyLabelInterfaceLanguage),
        "database.url" => Some(ServerI18nKey::SettingsPropertyLabelDatabaseUrl),
        "logging.level" => Some(ServerI18nKey::SettingsPropertyLabelLogLevel),
        "logging.json" => Some(ServerI18nKey::SettingsPropertyLabelJsonLogs),
        "logging.path" => Some(ServerI18nKey::SettingsPropertyLabelLogDirectory),
        "telemetry.enabled" => Some(ServerI18nKey::SettingsPropertyLabelTelemetry),
        "telemetry.environment" => Some(ServerI18nKey::SettingsPropertyLabelEnvironment),
        "telemetry.service_name" => Some(ServerI18nKey::SettingsPropertyLabelServiceName),
        "telemetry.service_version" => Some(ServerI18nKey::SettingsPropertyLabelServiceVersion),
        "telemetry.metrics_exporter" => Some(ServerI18nKey::SettingsPropertyLabelMetricsExporter),
        "telemetry.capture_content" => {
            Some(ServerI18nKey::SettingsPropertyLabelCaptureGenaiContent)
        }
        "telemetry.span_attributes" => Some(ServerI18nKey::SettingsPropertyLabelSpanAttributes),
        "telemetry.tracestate" => Some(ServerI18nKey::SettingsPropertyLabelTraceState),
        "runtime.mode" => Some(ServerI18nKey::SettingsPropertyLabelRuntimeMode),
        "runtime.transport" => Some(ServerI18nKey::SettingsPropertyLabelTransport),
        "runtime.sessions.state_dir" => {
            Some(ServerI18nKey::SettingsPropertyLabelSessionStateDirectory)
        }
        "agent.debug" => Some(ServerI18nKey::SettingsPropertyLabelAgentDebugTrace),
        "agent.hooks.enabled" => Some(ServerI18nKey::SettingsPropertyLabelExternalHooks),
        "agent.hooks.scripts" => Some(ServerI18nKey::SettingsPropertyLabelLegacyHookScripts),
        "agent.memories.enabled" => Some(ServerI18nKey::SettingsPropertyLabelAgentMemories),
        "agent.memories.model" => Some(ServerI18nKey::SettingsPropertyLabelAgentMemoryModel),
        "agent.memories.memory_root" => Some(ServerI18nKey::SettingsPropertyLabelMemoryRoot),
        "agent.memories.phase1_scan_limit" => {
            Some(ServerI18nKey::SettingsPropertyLabelPhase1ScanLimit)
        }
        "agent.memories.phase1_concurrency" => {
            Some(ServerI18nKey::SettingsPropertyLabelPhase1Concurrency)
        }
        "agent.memories.phase1_idle_seconds" => {
            Some(ServerI18nKey::SettingsPropertyLabelPhase1IdleSeconds)
        }
        "agent.memories.phase1_lease_seconds" => {
            Some(ServerI18nKey::SettingsPropertyLabelPhase1LeaseSeconds)
        }
        "agent.memories.phase1_retry_seconds" => {
            Some(ServerI18nKey::SettingsPropertyLabelPhase1RetrySeconds)
        }
        "agent.memories.phase1_max_age_days" => {
            Some(ServerI18nKey::SettingsPropertyLabelPhase1MaxAgeDays)
        }
        "agent.memories.phase2_limit" => Some(ServerI18nKey::SettingsPropertyLabelPhase2Limit),
        "agent.memories.phase2_lease_seconds" => {
            Some(ServerI18nKey::SettingsPropertyLabelPhase2LeaseSeconds)
        }
        "agent.memories.max_unused_days" => Some(ServerI18nKey::SettingsPropertyLabelMaxUnusedDays),
        "agent.memories.extension_retention_days" => {
            Some(ServerI18nKey::SettingsPropertyLabelExtensionRetentionDays)
        }
        "agent.tools.mcp.enabled" => Some(ServerI18nKey::SettingsPropertyLabelMcpTools),
        "agent.tools.mcp.servers" => Some(ServerI18nKey::SettingsPropertyLabelMcpServers),
        "agent.tools.websearch.default_provider" => {
            Some(ServerI18nKey::SettingsPropertyLabelDefaultProvider)
        }
        "agent.tools.websearch.providers" => {
            Some(ServerI18nKey::SettingsPropertyLabelWebSearchProviders)
        }
        _ if path.ends_with(".flash_attn") => {
            Some(ServerI18nKey::SettingsPropertyLabelFlashAttention)
        }
        "providers.registry" => Some(ServerI18nKey::SettingsPropertyLabelProviderRegistry),
        "models.cache_dir" => Some(ServerI18nKey::SettingsPropertyLabelModelCacheDirectory),
        "models.config_dir" => Some(ServerI18nKey::SettingsPropertyLabelModelConfigDirectory),
        "models.download_source" => Some(ServerI18nKey::SettingsPropertyLabelModelSource),
        "plugin.install_dir" => Some(ServerI18nKey::SettingsPropertyLabelPluginInstallDirectory),
        "plugin.js_runtime_transport" => {
            Some(ServerI18nKey::SettingsPropertyLabelJsRuntimeTransport)
        }
        "plugin.python_runtime_transport" => {
            Some(ServerI18nKey::SettingsPropertyLabelPythonRuntimeTransport)
        }
        "server.address" => Some(ServerI18nKey::SettingsPropertyLabelBindAddress),
        "server.admin.token" => Some(ServerI18nKey::SettingsPropertyLabelAdminToken),
        "server.cors.allowed_origins" => Some(ServerI18nKey::SettingsPropertyLabelAllowedOrigins),
        "server.cloud_http_trace" => Some(ServerI18nKey::SettingsPropertyLabelCloudHttpTrace),
        "models.auto_unload.idle_minutes" => {
            Some(ServerI18nKey::SettingsPropertyLabelAutoUnloadIdleMinutes)
        }
        "models.auto_unload.min_free_system_memory_bytes" => {
            Some(ServerI18nKey::SettingsPropertyLabelAutoUnloadMinFreeSystemMemoryBytes)
        }
        "models.auto_unload.min_free_gpu_memory_bytes" => {
            Some(ServerI18nKey::SettingsPropertyLabelAutoUnloadMinFreeGpuMemoryBytes)
        }
        "models.auto_unload.max_pressure_evictions_per_load" => {
            Some(ServerI18nKey::SettingsPropertyLabelAutoUnloadMaxPressureEvictionsPerLoad)
        }
        _ if path.ends_with(".enabled") => Some(ServerI18nKey::SettingsPropertyLabelGenericEnabled),
        _ if path.ends_with(".auto_download") => {
            Some(ServerI18nKey::SettingsPropertyLabelGenericAutoDownload)
        }
        _ if path.ends_with(".install_dir") => {
            Some(ServerI18nKey::SettingsPropertyLabelGenericInstallDirectory)
        }
        _ if path.ends_with(".level") => Some(ServerI18nKey::SettingsPropertyLabelGenericLogLevel),
        _ if path.ends_with(".json") => Some(ServerI18nKey::SettingsPropertyLabelGenericJsonLogs),
        _ if path.ends_with(".path") => Some(ServerI18nKey::SettingsPropertyLabelGenericPath),
        _ if path.ends_with(".queue") => Some(ServerI18nKey::SettingsPropertyLabelGenericQueue),
        _ if path.ends_with(".concurrent_requests") => {
            Some(ServerI18nKey::SettingsPropertyLabelGenericConcurrentRequests)
        }
        _ if path.ends_with(".address") => Some(ServerI18nKey::SettingsPropertyLabelGenericAddress),
        _ if path.ends_with(".ipc.path") => {
            Some(ServerI18nKey::SettingsPropertyLabelGenericIpcPath)
        }
        _ if path.ends_with(".version") => Some(ServerI18nKey::SettingsPropertyLabelGenericVersion),
        _ if path.ends_with(".artifact") => {
            Some(ServerI18nKey::SettingsPropertyLabelGenericArtifact)
        }
        _ if path.ends_with(".context_length") => {
            Some(ServerI18nKey::SettingsPropertyLabelGenericContextLength)
        }
        _ => None,
    }
}

fn property_description_key(path: &str) -> Option<ServerI18nKey> {
    match path {
        "general.language" => Some(ServerI18nKey::SettingsPropertyDescriptionInterfaceLanguage),
        "database.url" => Some(ServerI18nKey::SettingsPropertyDescriptionDatabaseUrl),
        "logging.level" => Some(ServerI18nKey::SettingsPropertyDescriptionLogLevel),
        "logging.json" => Some(ServerI18nKey::SettingsPropertyDescriptionJsonLogs),
        "logging.path" => Some(ServerI18nKey::SettingsPropertyDescriptionLogDirectory),
        "telemetry.enabled" => Some(ServerI18nKey::SettingsPropertyDescriptionTelemetry),
        "telemetry.environment" => Some(ServerI18nKey::SettingsPropertyDescriptionEnvironment),
        "telemetry.service_name" => Some(ServerI18nKey::SettingsPropertyDescriptionServiceName),
        "telemetry.service_version" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionServiceVersion)
        }
        "telemetry.metrics_exporter" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionMetricsExporter)
        }
        "telemetry.capture_content" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionCaptureGenaiContent)
        }
        "telemetry.span_attributes" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionSpanAttributes)
        }
        "telemetry.tracestate" => Some(ServerI18nKey::SettingsPropertyDescriptionTraceState),
        "tools.ffmpeg.enabled" => Some(ServerI18nKey::SettingsPropertyDescriptionFfmpegEnabled),
        "tools.ffmpeg.auto_download" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionFfmpegAutoDownload)
        }
        "tools.ffmpeg.install_dir" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionFfmpegInstallDir)
        }
        "agent.debug" => Some(ServerI18nKey::SettingsPropertyDescriptionAgentDebugTrace),
        "agent.hooks.enabled" => Some(ServerI18nKey::SettingsPropertyDescriptionExternalHooks),
        "agent.hooks.scripts" => Some(ServerI18nKey::SettingsPropertyDescriptionLegacyHookScripts),
        "agent.memories.enabled" => Some(ServerI18nKey::SettingsPropertyDescriptionAgentMemories),
        "agent.memories.model" => Some(ServerI18nKey::SettingsPropertyDescriptionAgentMemoryModel),
        "agent.memories.memory_root" => Some(ServerI18nKey::SettingsPropertyDescriptionMemoryRoot),
        "agent.memories.phase1_scan_limit" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionPhase1ScanLimit)
        }
        "agent.memories.phase1_concurrency" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionPhase1Concurrency)
        }
        "agent.memories.phase1_idle_seconds" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionPhase1IdleSeconds)
        }
        "agent.memories.phase1_lease_seconds" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionPhase1LeaseSeconds)
        }
        "agent.memories.phase1_retry_seconds" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionPhase1RetrySeconds)
        }
        "agent.memories.phase1_max_age_days" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionPhase1MaxAgeDays)
        }
        "agent.memories.phase2_limit" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionPhase2Limit)
        }
        "agent.memories.phase2_lease_seconds" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionPhase2LeaseSeconds)
        }
        "agent.memories.max_unused_days" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionMaxUnusedDays)
        }
        "agent.memories.extension_retention_days" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionExtensionRetentionDays)
        }
        "agent.tools.mcp.enabled" => Some(ServerI18nKey::SettingsPropertyDescriptionMcpTools),
        "agent.tools.mcp.servers" => Some(ServerI18nKey::SettingsPropertyDescriptionMcpServers),
        "agent.tools.websearch.default_provider" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionDefaultProvider)
        }
        "agent.tools.websearch.providers" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionWebSearchProviders)
        }
        "runtime.mode" => Some(ServerI18nKey::SettingsPropertyDescriptionRuntimeMode),
        "runtime.transport" => Some(ServerI18nKey::SettingsPropertyDescriptionRuntimeTransport),
        "runtime.sessions.state_dir" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionSessionStateDirectory)
        }
        "providers.registry" => Some(ServerI18nKey::SettingsPropertyDescriptionProviderRegistry),
        "models.cache_dir" => Some(ServerI18nKey::SettingsPropertyDescriptionModelCacheDirectory),
        "models.config_dir" => Some(ServerI18nKey::SettingsPropertyDescriptionModelConfigDirectory),
        "models.download_source" => Some(ServerI18nKey::SettingsPropertyDescriptionModelSource),
        "plugin.install_dir" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionPluginInstallDirectory)
        }
        "plugin.js_runtime_transport" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionJsRuntimeTransport)
        }
        "plugin.python_runtime_transport" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionPythonRuntimeTransport)
        }
        "models.auto_unload.enabled" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionAutoUnloadEnabled)
        }
        "models.auto_unload.idle_minutes" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionAutoUnloadIdleMinutes)
        }
        "models.auto_unload.min_free_system_memory_bytes" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionAutoUnloadMinFreeSystemMemoryBytes)
        }
        "models.auto_unload.min_free_gpu_memory_bytes" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionAutoUnloadMinFreeGpuMemoryBytes)
        }
        "models.auto_unload.max_pressure_evictions_per_load" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionAutoUnloadMaxPressureEvictionsPerLoad)
        }
        "server.address" => Some(ServerI18nKey::SettingsPropertyDescriptionServerAddress),
        "server.admin.token" => Some(ServerI18nKey::SettingsPropertyDescriptionAdminToken),
        "server.cors.allowed_origins" => {
            Some(ServerI18nKey::SettingsPropertyDescriptionAllowedOrigins)
        }
        "server.swagger.enabled" => Some(ServerI18nKey::SettingsPropertyDescriptionSwaggerEnabled),
        "server.cloud_http_trace" => Some(ServerI18nKey::SettingsPropertyDescriptionCloudHttpTrace),
        _ if path.ends_with(".enabled") => {
            Some(ServerI18nKey::SettingsPropertyDescriptionGenericEnabled)
        }
        _ if path.ends_with(".flash_attn") => {
            Some(ServerI18nKey::SettingsPropertyDescriptionGenericFlashAttention)
        }
        _ if path.ends_with(".install_dir") => {
            Some(ServerI18nKey::SettingsPropertyDescriptionGenericInstallDirectory)
        }
        _ if path.ends_with(".level") => {
            Some(ServerI18nKey::SettingsPropertyDescriptionGenericLogLevel)
        }
        _ if path.ends_with(".json") => {
            Some(ServerI18nKey::SettingsPropertyDescriptionGenericJsonLogs)
        }
        _ if path.ends_with(".path") => Some(ServerI18nKey::SettingsPropertyDescriptionGenericPath),
        _ if path.ends_with(".queue") => {
            Some(ServerI18nKey::SettingsPropertyDescriptionGenericQueue)
        }
        _ if path.ends_with(".concurrent_requests") => {
            Some(ServerI18nKey::SettingsPropertyDescriptionGenericConcurrentRequests)
        }
        _ if path.ends_with(".address") => {
            Some(ServerI18nKey::SettingsPropertyDescriptionGenericAddress)
        }
        _ if path.ends_with(".ipc.path") => {
            Some(ServerI18nKey::SettingsPropertyDescriptionGenericIpcPath)
        }
        _ if path.ends_with(".version") => {
            Some(ServerI18nKey::SettingsPropertyDescriptionGenericVersion)
        }
        _ if path.ends_with(".artifact") => {
            Some(ServerI18nKey::SettingsPropertyDescriptionGenericArtifact)
        }
        _ if path.ends_with(".context_length") => {
            Some(ServerI18nKey::SettingsPropertyDescriptionGenericContextLength)
        }
        _ => None,
    }
}

fn search_terms(path: &str) -> Vec<String> {
    let mut search_terms: Vec<String> = path.split('.').map(|segment| segment.to_owned()).collect();
    search_terms
        .extend(property_label(path).split_whitespace().map(|segment| segment.to_lowercase()));
    search_terms.sort();
    search_terms.dedup();
    search_terms
}

fn humanize_setting_label(raw: &str) -> String {
    raw.split('_')
        .map(|segment| match segment {
            "api" => "API".to_owned(),
            "id" => "ID".to_owned(),
            "ipc" => "IPC".to_owned(),
            "http" => "HTTP".to_owned(),
            "url" => "URL".to_owned(),
            "ffmpeg" => "FFmpeg".to_owned(),
            "ggml" => "GGML".to_owned(),
            "onnx" => "ONNX".to_owned(),
            other => {
                let mut chars = other.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_string(raw: Option<String>) -> Option<String> {
    raw.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
    })
}

fn provider_registry_entry_to_cloud_provider(entry: &ProviderRegistryEntry) -> CloudProviderConfig {
    CloudProviderConfig {
        id: entry.id.clone(),
        name: entry.display_name.clone(),
        api_base: entry.api_base.clone(),
        api_key: entry.auth.api_key.clone(),
        api_key_env: entry.auth.api_key_env.clone(),
    }
}

#[derive(Debug, Clone, Copy)]
enum RuntimeBackend {
    Llama,
    Whisper,
    Diffusion,
}

fn resolve_backend_concurrency(settings: &SettingsDocument, backend: RuntimeBackend) -> u32 {
    let family = &settings.runtime.ggml.capacity;
    let leaf = match backend {
        RuntimeBackend::Llama => settings.runtime.ggml.backends.llama.capacity.concurrent_requests,
        RuntimeBackend::Whisper => {
            settings.runtime.ggml.backends.whisper.capacity.concurrent_requests
        }
        RuntimeBackend::Diffusion => {
            settings.runtime.ggml.backends.diffusion.capacity.concurrent_requests
        }
    };

    leaf.or(family.concurrent_requests).unwrap_or(settings.runtime.capacity.concurrent_requests)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;

    use super::*;
    use crate::{
        InterfaceLanguagePreference, ProviderAuthConfig, ProviderDefaultsConfig, ProviderFamily,
    };

    fn temp_settings_path() -> PathBuf {
        let base = std::env::temp_dir().join(format!("slab-pmid-test-{}", uuid::Uuid::new_v4()));
        base.join("settings.json")
    }

    #[tokio::test]
    async fn load_from_path_supports_current_settings_document() {
        let path = temp_settings_path();
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        let mut document = SettingsDocument::default();
        document.models.cache_dir = Some("C:/models".to_owned());
        document.tools.ffmpeg.install_dir = Some("C:/ffmpeg".to_owned());
        document.providers.registry.push(ProviderRegistryEntry {
            id: "openai-main".to_owned(),
            family: ProviderFamily::OpenaiCompatible,
            display_name: "OpenAI".to_owned(),
            api_base: "https://api.openai.com/v1".to_owned(),
            auth: ProviderAuthConfig { api_key: Some("sk-test".to_owned()), api_key_env: None },
            defaults: ProviderDefaultsConfig::default(),
        });
        fs::write(&path, serde_json::to_string_pretty(&document).expect("serialize"))
            .expect("write");

        let service = PmidService::load_from_path(path.clone()).await.expect("pmid service");
        let config = service.config();
        let property = service.property("models.cache_dir").await.expect("property");
        let plugin_install_dir = service.property("plugin.install_dir").await.expect("plugin dir");
        let expected_plugin_dir =
            slab_utils::app_home::plugins_dir().to_string_lossy().into_owned();

        assert_eq!(config.runtime.model_cache_dir.as_deref(), Some("C:/models"));
        assert!(config.agent.debug);
        assert_eq!(config.setup.ffmpeg.dir.as_deref(), Some("C:/ffmpeg"));
        assert!(config.telemetry.enabled);
        assert!(!config.telemetry.capture_content);
        assert_eq!(config.chat.providers.len(), 1);
        assert_eq!(property.effective_value, json!("C:/models").into());
        assert_eq!(plugin_install_dir.effective_value, json!(expected_plugin_dir).into());
        assert_eq!(plugin_install_dir.schema.default_value, plugin_install_dir.effective_value);
        assert!(!plugin_install_dir.is_overridden);

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn secret_setting_views_redact_literal_secret_values() {
        let path = temp_settings_path();
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        let mut document = SettingsDocument::default();
        document.server.admin.token = Some("admin-secret-token".to_owned());
        document.providers.registry.push(ProviderRegistryEntry {
            id: "openai-main".to_owned(),
            family: ProviderFamily::OpenaiCompatible,
            display_name: "OpenAI".to_owned(),
            api_base: "https://api.openai.com/v1".to_owned(),
            auth: ProviderAuthConfig {
                api_key: Some("provider-secret-token".to_owned()),
                api_key_env: Some("OPENAI_API_KEY".to_owned()),
            },
            defaults: ProviderDefaultsConfig::default(),
        });
        document.agent.tools.websearch.providers.google.auth.api_key =
            Some("google-secret-token".to_owned());
        document.agent.tools.websearch.providers.google.auth.api_key_env =
            Some("GOOGLE_API_KEY".to_owned());
        fs::write(&path, serde_json::to_string_pretty(&document).expect("serialize"))
            .expect("write");

        let service = PmidService::load_from_path(path.clone()).await.expect("pmid service");
        let admin = service.property("server.admin.token").await.expect("admin token");
        let registry = service.property("providers.registry").await.expect("provider registry");
        let websearch =
            service.property("agent.tools.websearch.providers").await.expect("websearch providers");
        let mcp = service.property("agent.tools.mcp.servers").await.expect("mcp servers");
        let registry_value = registry.effective_value.clone().into_json_value();
        let websearch_value = websearch.effective_value.clone().into_json_value();

        assert!(admin.schema.secret);
        assert_eq!(admin.effective_value, SettingValue::String(SECRET_PLACEHOLDER.to_owned()));
        assert!(registry.schema.secret);
        assert_eq!(registry_value.pointer("/0/auth/api_key"), Some(&json!(SECRET_PLACEHOLDER)));
        assert_eq!(registry_value.pointer("/0/auth/api_key_env"), Some(&json!("OPENAI_API_KEY")));
        assert!(websearch.schema.secret);
        assert_eq!(
            websearch_value.pointer("/google/auth/api_key"),
            Some(&json!(SECRET_PLACEHOLDER))
        );
        assert_eq!(
            websearch_value.pointer("/google/auth/api_key_env"),
            Some(&json!("GOOGLE_API_KEY"))
        );
        assert!(!mcp.schema.secret);
        assert!(!registry_value.to_string().contains("provider-secret-token"));
        assert!(!websearch_value.to_string().contains("google-secret-token"));

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn update_setting_preserves_redacted_secret_placeholders() {
        let path = temp_settings_path();
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        let mut document = SettingsDocument::default();
        document.server.admin.token = Some("admin-secret-token".to_owned());
        document.providers.registry.push(ProviderRegistryEntry {
            id: "openai-main".to_owned(),
            family: ProviderFamily::OpenaiCompatible,
            display_name: "OpenAI".to_owned(),
            api_base: "https://api.openai.com/v1".to_owned(),
            auth: ProviderAuthConfig {
                api_key: Some("provider-secret-token".to_owned()),
                api_key_env: None,
            },
            defaults: ProviderDefaultsConfig::default(),
        });
        fs::write(&path, serde_json::to_string_pretty(&document).expect("serialize"))
            .expect("write");

        let service = PmidService::load_from_path(path.clone()).await.expect("pmid service");
        let admin = service.property("server.admin.token").await.expect("admin token");
        service
            .update_setting(
                "server.admin.token",
                UpdateSettingCommand {
                    op: crate::UpdateSettingOperation::Set,
                    value: Some(admin.effective_value),
                },
            )
            .await
            .expect("admin token update");

        let registry = service.property("providers.registry").await.expect("provider registry");
        let mut registry_value = registry.effective_value.into_json_value();
        registry_value[0]["display_name"] = json!("OpenAI Updated");
        service
            .update_setting(
                "providers.registry",
                UpdateSettingCommand {
                    op: crate::UpdateSettingOperation::Set,
                    value: Some(registry_value.into()),
                },
            )
            .await
            .expect("registry update");

        let persisted: SettingsDocument =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");
        let provider = persisted.providers.registry.first().expect("provider");

        assert_eq!(persisted.server.admin.token.as_deref(), Some("admin-secret-token"));
        assert_eq!(provider.display_name, "OpenAI Updated");
        assert_eq!(provider.auth.api_key.as_deref(), Some("provider-secret-token"));

        let returned = service.property("providers.registry").await.expect("provider registry");
        assert!(
            !returned
                .effective_value
                .into_json_value()
                .to_string()
                .contains("provider-secret-token")
        );

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn load_from_paths_applies_workspace_overlay() {
        let path = temp_settings_path();
        let overlay_path = path.parent().expect("parent").join("workspace").join("settings.json");
        fs::create_dir_all(path.parent().expect("parent")).expect("base dir");
        fs::create_dir_all(overlay_path.parent().expect("parent")).expect("overlay dir");
        let mut document = SettingsDocument::default();
        document.models.cache_dir = Some("C:/global-models".to_owned());
        fs::write(&path, serde_json::to_string_pretty(&document).expect("serialize"))
            .expect("write base");
        fs::write(
            &overlay_path,
            serde_json::to_string_pretty(&json!({
                "models": {
                    "cache_dir": "D:/workspace-models"
                }
            }))
            .expect("serialize"),
        )
        .expect("write overlay");

        let service = PmidService::load_from_paths(path.clone(), Some(overlay_path.clone()))
            .await
            .expect("pmid service");
        let document_view = service.document().await;

        assert_eq!(
            service.config().runtime.model_cache_dir.as_deref(),
            Some("D:/workspace-models")
        );
        assert_eq!(document_view.settings_path, overlay_path.display().to_string());

        service
            .update_setting(
                "models.cache_dir",
                UpdateSettingCommand { op: crate::UpdateSettingOperation::Unset, value: None },
            )
            .await
            .expect("unset");

        assert_eq!(service.config().runtime.model_cache_dir.as_deref(), Some("C:/global-models"));

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn update_setting_refreshes_cached_snapshot() {
        let path = temp_settings_path();
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        fs::write(
            &path,
            serde_json::to_string_pretty(&SettingsDocument::default()).expect("serialize"),
        )
        .expect("write");

        let service = PmidService::load_from_path(path.clone()).await.expect("pmid service");

        service
            .update_setting(
                "models.cache_dir",
                UpdateSettingCommand {
                    op: crate::UpdateSettingOperation::Set,
                    value: Some(json!("D:/models").into()),
                },
            )
            .await
            .expect("update");

        let persisted: SettingsDocument =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");

        assert_eq!(service.config().runtime.model_cache_dir.as_deref(), Some("D:/models"));
        assert_eq!(persisted.models.cache_dir.as_deref(), Some("D:/models"));

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn refresh_picks_up_external_settings_file_change() {
        let path = temp_settings_path();
        let service = PmidService::load_from_path(path.clone()).await.expect("pmid service");
        let mut document: SettingsDocument =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");
        document.logging.level = "debug".to_owned();
        fs::write(&path, serde_json::to_string_pretty(&document).expect("serialize"))
            .expect("external write");

        service.refresh().await.expect("refresh");

        assert_eq!(service.config().logging.level, "debug");

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn general_language_setting_is_grouped_and_persisted() {
        let path = temp_settings_path();
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        fs::write(
            &path,
            serde_json::to_string_pretty(&SettingsDocument::default()).expect("serialize"),
        )
        .expect("write");

        let service = PmidService::load_from_path(path.clone()).await.expect("pmid service");
        let document = service.document().await;
        let general_section = document
            .sections
            .iter()
            .find(|section| section.id == "general")
            .expect("general section");
        let general_subsection = general_section
            .subsections
            .iter()
            .find(|subsection| subsection.id == "general")
            .expect("general subsection");

        assert!(
            general_subsection
                .properties
                .iter()
                .any(|property| property.pmid == "general.language")
        );

        service
            .update_setting(
                "general.language",
                UpdateSettingCommand {
                    op: crate::UpdateSettingOperation::Set,
                    value: Some(json!("zh-CN").into()),
                },
            )
            .await
            .expect("update");

        let persisted: SettingsDocument =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");

        assert_eq!(persisted.general.language, InterfaceLanguagePreference::ZhCn);

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn document_view_includes_agent_web_search_settings() {
        let path = temp_settings_path();
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        fs::write(
            &path,
            serde_json::to_string_pretty(&SettingsDocument::default()).expect("serialize"),
        )
        .expect("write");

        let service = PmidService::load_from_path(path.clone()).await.expect("pmid service");
        let document = service.document().await;
        let agent_section =
            document.sections.iter().find(|section| section.id == "agent").expect("agent section");
        let general_subsection = agent_section
            .subsections
            .iter()
            .find(|subsection| subsection.id == "general")
            .expect("general subsection");
        let mcp_subsection = agent_section
            .subsections
            .iter()
            .find(|subsection| subsection.id == "mcp")
            .expect("mcp subsection");
        let websearch_subsection = agent_section
            .subsections
            .iter()
            .find(|subsection| subsection.id == "websearch")
            .expect("websearch subsection");
        let hooks_subsection = agent_section
            .subsections
            .iter()
            .find(|subsection| subsection.id == "hooks")
            .expect("hooks subsection");
        let memories_subsection = agent_section
            .subsections
            .iter()
            .find(|subsection| subsection.id == "memories")
            .expect("memories subsection");
        let mcp_enabled = mcp_subsection
            .properties
            .iter()
            .find(|property| property.pmid == "agent.tools.mcp.enabled")
            .expect("mcp enabled property");
        let mcp_servers = mcp_subsection
            .properties
            .iter()
            .find(|property| property.pmid == "agent.tools.mcp.servers")
            .expect("mcp servers property");
        let default_provider = websearch_subsection
            .properties
            .iter()
            .find(|property| property.pmid == "agent.tools.websearch.default_provider")
            .expect("default provider property");
        let providers = websearch_subsection
            .properties
            .iter()
            .find(|property| property.pmid == "agent.tools.websearch.providers")
            .expect("providers property");
        let agent_debug = general_subsection
            .properties
            .iter()
            .find(|property| property.pmid == "agent.debug")
            .expect("agent debug property");
        let hooks_enabled = hooks_subsection
            .properties
            .iter()
            .find(|property| property.pmid == "agent.hooks.enabled")
            .expect("hooks enabled property");
        let hook_scripts = hooks_subsection
            .properties
            .iter()
            .find(|property| property.pmid == "agent.hooks.scripts")
            .expect("hook scripts property");
        let memories_enabled = memories_subsection
            .properties
            .iter()
            .find(|property| property.pmid == "agent.memories.enabled")
            .expect("memories enabled property");
        let memory_root = memories_subsection
            .properties
            .iter()
            .find(|property| property.pmid == "agent.memories.memory_root")
            .expect("memory root property");
        let phase1_scan_limit = memories_subsection
            .properties
            .iter()
            .find(|property| property.pmid == "agent.memories.phase1_scan_limit")
            .expect("phase1 scan limit property");
        let provider_enum = default_provider.schema.enum_values.as_ref().expect("provider enum");
        let schema = providers.schema.json_schema.as_ref().expect("providers schema");

        assert_eq!(agent_section.title, "Agent");
        assert_eq!(general_subsection.title, "General");
        assert_eq!(agent_debug.schema.value_type, SettingValueType::Boolean);
        assert_eq!(agent_debug.effective_value, SettingValue::Boolean(true));
        assert_eq!(hooks_subsection.title, "Hooks");
        assert_eq!(hooks_enabled.schema.value_type, SettingValueType::Boolean);
        assert_eq!(hooks_enabled.effective_value, SettingValue::Boolean(false));
        assert_eq!(hook_scripts.schema.value_type, SettingValueType::Array);
        assert!(hook_scripts.schema.multiline);
        assert_eq!(memories_subsection.title, "Memories");
        assert_eq!(memories_enabled.schema.value_type, SettingValueType::Boolean);
        assert_eq!(memory_root.schema.value_type, SettingValueType::String);
        assert_eq!(phase1_scan_limit.schema.value_type, SettingValueType::Integer);
        assert_eq!(mcp_subsection.title, "MCP");
        assert_eq!(mcp_enabled.schema.value_type, SettingValueType::Boolean);
        assert_eq!(mcp_servers.schema.value_type, SettingValueType::Array);
        assert!(mcp_servers.schema.multiline);
        assert_eq!(
            mcp_servers
                .schema
                .json_schema
                .as_ref()
                .expect("mcp servers schema")
                .pointer("/items/properties/env/additionalProperties/properties/env_var/type"),
            Some(&json!("string"))
        );
        assert_eq!(websearch_subsection.title, "Web Search");
        assert!(provider_enum.contains(&"duckduckgo".to_owned()));
        assert!(provider_enum.contains(&"searxng".to_owned()));
        assert_eq!(providers.schema.value_type, SettingValueType::Object);
        assert!(providers.schema.multiline);
        assert_eq!(schema["$defs"]["webSearchAuth"]["properties"]["api_key"]["writeOnly"], true);

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn document_view_includes_telemetry_settings() {
        let path = temp_settings_path();
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        fs::write(
            &path,
            serde_json::to_string_pretty(&SettingsDocument::default()).expect("serialize"),
        )
        .expect("write");

        let service = PmidService::load_from_path(path.clone()).await.expect("pmid service");
        let document = service.document().await;
        let telemetry_section = document
            .sections
            .iter()
            .find(|section| section.id == "telemetry")
            .expect("telemetry section");
        let general = telemetry_section
            .subsections
            .iter()
            .find(|subsection| subsection.id == "general")
            .expect("telemetry general subsection");

        let enabled = general
            .properties
            .iter()
            .find(|property| property.pmid == "telemetry.enabled")
            .expect("enabled property");
        let capture_content = general
            .properties
            .iter()
            .find(|property| property.pmid == "telemetry.capture_content")
            .expect("capture content property");
        let metrics_exporter = general
            .properties
            .iter()
            .find(|property| property.pmid == "telemetry.metrics_exporter")
            .expect("metrics exporter property");

        assert_eq!(enabled.schema.value_type, SettingValueType::Boolean);
        assert_eq!(enabled.effective_value, SettingValue::Boolean(true));
        assert_eq!(capture_content.schema.value_type, SettingValueType::Boolean);
        assert_eq!(capture_content.effective_value, SettingValue::Boolean(false));
        assert_eq!(metrics_exporter.schema.value_type, SettingValueType::Object);
        assert!(metrics_exporter.schema.multiline);
        assert!(!general.properties.iter().any(|property| property.pmid == "telemetry.slab_home"));
        assert!(!general.properties.iter().any(|property| property.pmid == "telemetry.exporter"));
        assert!(
            !general.properties.iter().any(|property| property.pmid == "telemetry.trace_exporter")
        );

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn update_websearch_providers_refreshes_cached_snapshot() {
        let path = temp_settings_path();
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        fs::write(
            &path,
            serde_json::to_string_pretty(&SettingsDocument::default()).expect("serialize"),
        )
        .expect("write");

        let service = PmidService::load_from_path(path.clone()).await.expect("pmid service");

        service
            .update_setting(
                "agent.tools.websearch.providers",
                UpdateSettingCommand {
                    op: crate::UpdateSettingOperation::Set,
                    value: Some(
                        json!({
                            "google": {
                                "auth": { "api_key_env": "GOOGLE_SEARCH_API_KEY" },
                                "cx": "search-engine-id"
                            }
                        })
                        .into(),
                    ),
                },
            )
            .await
            .expect("update");

        let config = service.config();
        let persisted: SettingsDocument =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");

        assert_eq!(
            config.agent.tools.websearch.providers.google.cx.as_deref(),
            Some("search-engine-id")
        );
        assert_eq!(
            config.agent.tools.websearch.providers.google.auth.api_key_env.as_deref(),
            Some("GOOGLE_SEARCH_API_KEY")
        );
        assert_eq!(
            persisted.agent.tools.websearch.providers.google.cx.as_deref(),
            Some("search-engine-id")
        );

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn update_agent_hooks_enabled_refreshes_cached_snapshot() {
        let path = temp_settings_path();
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        fs::write(
            &path,
            serde_json::to_string_pretty(&SettingsDocument::default()).expect("serialize"),
        )
        .expect("write");

        let service = PmidService::load_from_path(path.clone()).await.expect("pmid service");

        service
            .update_setting(
                "agent.hooks.enabled",
                UpdateSettingCommand {
                    op: crate::UpdateSettingOperation::Set,
                    value: Some(json!(true).into()),
                },
            )
            .await
            .expect("update");

        let config = service.config();
        let persisted: SettingsDocument =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");

        assert!(config.agent.hooks.enabled);
        assert!(persisted.agent.hooks.enabled);

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn update_setting_uses_not_found_for_unknown_pmid() {
        let path = temp_settings_path();
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        fs::write(
            &path,
            serde_json::to_string_pretty(&SettingsDocument::default()).expect("serialize"),
        )
        .expect("write");

        let service = PmidService::load_from_path(path.clone()).await.expect("pmid service");

        let error = service
            .update_setting(
                "missing.setting",
                UpdateSettingCommand {
                    op: crate::UpdateSettingOperation::Set,
                    value: Some(json!(true).into()),
                },
            )
            .await
            .expect_err("missing pmid should fail");

        assert!(matches!(error, ConfigError::NotFound(_)));
        assert!(error.to_string().contains("missing.setting"));

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }
}
