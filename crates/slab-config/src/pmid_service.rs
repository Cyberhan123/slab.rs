use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::{
    ChatConfig, CloudProviderConfig, DesktopLaunchProfileConfig, DiffusionConfig,
    DiffusionPerformanceConfig, LaunchBackendConfig, LaunchBackendsConfig, LaunchConfig,
    LaunchProfilesConfig, ModelDownloadSourcePreference, PMID, PmidConfig, ProviderRegistryEntry,
    RuntimeConfig, RuntimeLlamaConfig, RuntimeModelAutoUnloadConfig, RuntimeWhisperConfig,
    RuntimeWorkerConfig, ServerLaunchProfileConfig, SettingsDocument, SetupBackendsConfig,
    SetupConfig, SetupFfmpegConfig, provider_registry_json_schema, string_list_json_schema,
    websearch_providers_json_schema,
};
use serde_json::Value;

use crate::descriptor::setting_value;
use crate::{
    ConfigError, LaunchHostPaths, LaunchProfile, ResolvedLaunchSpec, SettingsDocumentProvider,
};
use crate::{
    SettingPropertySchema, SettingPropertyView, SettingValue, SettingValueType,
    SettingsDocumentView, SettingsSectionView, SettingsSubsectionView, UpdateSettingCommand,
};

const DEFAULT_SERVER_RUNTIME_BIND_HOST: &str = "127.0.0.1";
const DEFAULT_SERVER_RUNTIME_BASE_PORT: u32 = 3001;
const DEFAULT_DESKTOP_RUNTIME_BIND_HOST: &str = "127.0.0.1";
const DEFAULT_DESKTOP_RUNTIME_BASE_PORT: u32 = 50051;

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

    Ok(SettingPropertyView {
        pmid: pmid.to_owned(),
        label: property_label(pmid),
        description_md: property_description(pmid),
        editable: true,
        schema: SettingPropertySchema {
            value_type: value_type(pmid, &effective_value, &default_value),
            enum_values: enum_values(pmid),
            minimum: minimum_value(pmid),
            maximum: None,
            pattern: None,
            json_schema: json_schema(pmid),
            default_value: default_value.clone(),
            secret: secret(pmid),
            multiline: multiline(pmid),
            order: 0,
        },
        effective_value: effective_value.clone(),
        override_value: is_overridden.then_some(effective_value),
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
            subsections: vec![SettingsSubsectionView {
                id: "general".to_owned(),
                title: "General".to_owned(),
                description_md:
                    "Choose how the desktop app should present shared interface preferences."
                        .to_owned(),
                properties: Vec::new(),
            }],
        },
        SettingsSectionView {
            id: "database".to_owned(),
            title: "Database".to_owned(),
            description_md: "Persistent application data storage and connection settings."
                .to_owned(),
            subsections: vec![SettingsSubsectionView {
                id: "general".to_owned(),
                title: "General".to_owned(),
                description_md:
                    "Configure the primary database connection used by the desktop app and server."
                        .to_owned(),
                properties: Vec::new(),
            }],
        },
        SettingsSectionView {
            id: "logging".to_owned(),
            title: "Logging".to_owned(),
            description_md:
                "Global logging defaults inherited by runtime workers and server processes."
                    .to_owned(),
            subsections: vec![SettingsSubsectionView {
                id: "general".to_owned(),
                title: "General".to_owned(),
                description_md:
                    "Choose the default log level, output format, and optional log directory."
                        .to_owned(),
                properties: Vec::new(),
            }],
        },
        SettingsSectionView {
            id: "tools".to_owned(),
            title: "Tools".to_owned(),
            description_md: "External helper binaries managed by the application.".to_owned(),
            subsections: vec![SettingsSubsectionView {
                id: "ffmpeg".to_owned(),
                title: "FFmpeg".to_owned(),
                description_md: "Configure FFmpeg installation and download behavior.".to_owned(),
                properties: Vec::new(),
            }],
        },
        SettingsSectionView {
            id: "runtime".to_owned(),
            title: "Runtime".to_owned(),
            description_md:
                "Shared inference runtime topology, transport, and backend-specific overrides."
                    .to_owned(),
            subsections: vec![
                SettingsSubsectionView {
                    id: "general".to_owned(),
                    title: "General".to_owned(),
                    description_md:
                        "Configure shared transport, session state, and fallback capacity limits."
                            .to_owned(),
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "ggml".to_owned(),
                    title: "GGML".to_owned(),
                    description_md: "Family-level defaults shared by GGML worker processes."
                        .to_owned(),
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "llama".to_owned(),
                    title: "Llama".to_owned(),
                    description_md: "Overrides specific to the GGML llama worker.".to_owned(),
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "whisper".to_owned(),
                    title: "Whisper".to_owned(),
                    description_md: "Overrides specific to the GGML whisper worker.".to_owned(),
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "diffusion".to_owned(),
                    title: "Diffusion".to_owned(),
                    description_md: "Overrides specific to the GGML diffusion worker.".to_owned(),
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "candle".to_owned(),
                    title: "Candle".to_owned(),
                    description_md: "Shared configuration for the Candle runtime family."
                        .to_owned(),
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "onnx".to_owned(),
                    title: "ONNX".to_owned(),
                    description_md: "Shared configuration for the ONNX runtime family.".to_owned(),
                    properties: Vec::new(),
                },
            ],
        },
        SettingsSectionView {
            id: "providers".to_owned(),
            title: "Providers".to_owned(),
            description_md: "Cloud and remote model provider definitions.".to_owned(),
            subsections: vec![SettingsSubsectionView {
                id: "registry".to_owned(),
                title: "Registry".to_owned(),
                description_md: "Manage provider endpoints, credentials, and request defaults."
                    .to_owned(),
                properties: Vec::new(),
            }],
        },
        SettingsSectionView {
            id: "models".to_owned(),
            title: "Models".to_owned(),
            description_md: "Model cache locations and automatic unload behavior.".to_owned(),
            subsections: vec![
                SettingsSubsectionView {
                    id: "general".to_owned(),
                    title: "General".to_owned(),
                    description_md: "Configure model cache and config directory locations."
                        .to_owned(),
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "auto_unload".to_owned(),
                    title: "Auto Unload".to_owned(),
                    description_md: "Unload idle models automatically to reclaim memory."
                        .to_owned(),
                    properties: Vec::new(),
                },
            ],
        },
        SettingsSectionView {
            id: "plugin".to_owned(),
            title: "Plugins".to_owned(),
            description_md: "Runtime plugin installation and registration settings.".to_owned(),
            subsections: vec![SettingsSubsectionView {
                id: "general".to_owned(),
                title: "General".to_owned(),
                description_md: "Choose where installed plugin packages are loaded from."
                    .to_owned(),
                properties: Vec::new(),
            }],
        },
        SettingsSectionView {
            id: "agent".to_owned(),
            title: "Agent".to_owned(),
            description_md: "Agent tool configuration used by built-in deterministic tools."
                .to_owned(),
            subsections: vec![
                SettingsSubsectionView {
                    id: "general".to_owned(),
                    title: "General".to_owned(),
                    description_md: "Agent diagnostics and runtime behavior.".to_owned(),
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "mcp".to_owned(),
                    title: "MCP".to_owned(),
                    description_md: "Control exposure of configured MCP servers as agent tools."
                        .to_owned(),
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "websearch".to_owned(),
                    title: "Web Search".to_owned(),
                    description_md:
                        "Configure the agent web search provider defaults and credentials."
                            .to_owned(),
                    properties: Vec::new(),
                },
            ],
        },
        SettingsSectionView {
            id: "server".to_owned(),
            title: "Server".to_owned(),
            description_md: "HTTP gateway configuration, access control, and API tooling."
                .to_owned(),
            subsections: vec![
                SettingsSubsectionView {
                    id: "general".to_owned(),
                    title: "General".to_owned(),
                    description_md:
                        "Configure the gateway bind address and server-side logging behavior."
                            .to_owned(),
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "cors".to_owned(),
                    title: "CORS".to_owned(),
                    description_md: "Allowed browser origins for the HTTP API.".to_owned(),
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "admin".to_owned(),
                    title: "Admin".to_owned(),
                    description_md: "Protect management routes with an optional bearer token."
                        .to_owned(),
                    properties: Vec::new(),
                },
                SettingsSubsectionView {
                    id: "swagger".to_owned(),
                    title: "Swagger".to_owned(),
                    description_md: "Enable or disable the OpenAPI and Swagger UI endpoints."
                        .to_owned(),
                    properties: Vec::new(),
                },
            ],
        },
    ]
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
        _ if path.starts_with("tools.ffmpeg.") => ("tools", "ffmpeg"),
        "agent.debug" => ("agent", "general"),
        _ if path.starts_with("agent.tools.mcp.") => ("agent", "mcp"),
        _ if path.starts_with("agent.tools.websearch.") => ("agent", "websearch"),
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
    if path == "providers.registry" || path == "server.cors.allowed_origins" {
        return SettingValueType::Array;
    }
    if path == "agent.tools.websearch.providers" {
        return SettingValueType::Object;
    }
    if path.ends_with(".enabled")
        || path.ends_with(".debug")
        || path.ends_with(".json")
        || path.ends_with(".auto_download")
        || path.ends_with(".flash_attn")
        || path == "server.cloud_http_trace"
    {
        return SettingValueType::Boolean;
    }
    if path.ends_with(".queue")
        || path.ends_with(".concurrent_requests")
        || path.ends_with(".idle_minutes")
        || path.ends_with(".context_length")
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
    {
        Some(0)
    } else {
        None
    }
}

fn json_schema(path: &str) -> Option<Value> {
    match path {
        "providers.registry" => Some(provider_registry_json_schema()),
        "agent.tools.websearch.providers" => Some(websearch_providers_json_schema()),
        "server.cors.allowed_origins" => Some(string_list_json_schema("Allowed Origins")),
        _ => None,
    }
}

fn secret(path: &str) -> bool {
    path == "server.admin.token"
}

fn multiline(path: &str) -> bool {
    path == "providers.registry" || path == "agent.tools.websearch.providers"
}

fn property_label(path: &str) -> String {
    match path {
        "general.language" => "Interface Language".to_owned(),
        "database.url" => "Database URL".to_owned(),
        "logging.level" => "Log Level".to_owned(),
        "logging.json" => "JSON Logs".to_owned(),
        "logging.path" => "Log Directory".to_owned(),
        "runtime.mode" => "Runtime Mode".to_owned(),
        "runtime.transport" => "Transport".to_owned(),
        "runtime.sessions.state_dir" => "Session State Directory".to_owned(),
        "agent.debug" => "Agent Debug Trace".to_owned(),
        "agent.tools.mcp.enabled" => "MCP Tools".to_owned(),
        "agent.tools.websearch.default_provider" => "Default Provider".to_owned(),
        "agent.tools.websearch.providers" => "Web Search Providers".to_owned(),
        _ if path.ends_with(".flash_attn") => "Flash Attention".to_owned(),
        "providers.registry" => "Provider Registry".to_owned(),
        "models.cache_dir" => "Model Cache Directory".to_owned(),
        "models.config_dir" => "Model Config Directory".to_owned(),
        "models.download_source" => "Model Source".to_owned(),
        "plugin.install_dir" => "Plugin Install Directory".to_owned(),
        "plugin.js_runtime_transport" => "JS Runtime Transport".to_owned(),
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
        "tools.ffmpeg.enabled" => "Enable FFmpeg integration for media tooling.".to_owned(),
        "tools.ffmpeg.auto_download" => "Download FFmpeg automatically when it is missing.".to_owned(),
        "tools.ffmpeg.install_dir" => "Optional install directory for the FFmpeg sidecar.".to_owned(),
        "agent.debug" => {
            "Write full-fidelity per-session agent trace files for prompt, tool, and runtime debugging.".to_owned()
        }
        "agent.tools.mcp.enabled" => {
            "Expose configured MCP tools to the agent tool router. Disabled by default.".to_owned()
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
        assert_eq!(config.chat.providers.len(), 1);
        assert_eq!(property.effective_value, json!("C:/models").into());
        assert_eq!(plugin_install_dir.effective_value, json!(expected_plugin_dir).into());
        assert_eq!(plugin_install_dir.schema.default_value, plugin_install_dir.effective_value);
        assert!(!plugin_install_dir.is_overridden);

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
        let mcp_enabled = mcp_subsection
            .properties
            .iter()
            .find(|property| property.pmid == "agent.tools.mcp.enabled")
            .expect("mcp enabled property");
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
        let provider_enum = default_provider.schema.enum_values.as_ref().expect("provider enum");
        let schema = providers.schema.json_schema.as_ref().expect("providers schema");

        assert_eq!(agent_section.title, "Agent");
        assert_eq!(general_subsection.title, "General");
        assert_eq!(agent_debug.schema.value_type, SettingValueType::Boolean);
        assert_eq!(agent_debug.effective_value, SettingValue::Boolean(true));
        assert_eq!(mcp_subsection.title, "MCP");
        assert_eq!(mcp_enabled.schema.value_type, SettingValueType::Boolean);
        assert_eq!(websearch_subsection.title, "Web Search");
        assert!(provider_enum.contains(&"duckduckgo".to_owned()));
        assert!(provider_enum.contains(&"searxng".to_owned()));
        assert_eq!(providers.schema.value_type, SettingValueType::Object);
        assert!(providers.schema.multiline);
        assert_eq!(schema["$defs"]["webSearchAuth"]["properties"]["api_key"]["writeOnly"], true);

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
