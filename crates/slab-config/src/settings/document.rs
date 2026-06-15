use std::collections::BTreeMap;

use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use slab_otel::config::OtelSettings;

use super::defaults;
use super::launch::RuntimeTransportMode;
use slab_types::DESKTOP_API_BIND;

pub const PUBLIC_SETTINGS_DOCUMENT_SCHEMA_URL: &str =
    "https://slab.reorgix.com/manifests/v1/settings-document.schema.json";

fn default_schema_ref() -> Option<String> {
    Some(PUBLIC_SETTINGS_DOCUMENT_SCHEMA_URL.to_owned())
}

const fn default_schema_version() -> u32 {
    2
}

fn default_log_level() -> String {
    "info".to_owned()
}

const fn default_runtime_queue() -> u32 {
    64
}

const fn default_runtime_concurrent_requests() -> u32 {
    4
}

fn default_root_capacity() -> CapacityConfig {
    CapacityConfig {
        queue: default_runtime_queue(),
        concurrent_requests: default_runtime_concurrent_requests(),
    }
}

const fn default_swagger_enabled() -> bool {
    true
}

const fn default_enabled() -> bool {
    true
}

const fn default_disabled() -> bool {
    false
}

/// Frontend interface language preference stored in settings.json.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
pub enum InterfaceLanguagePreference {
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "en-US")]
    EnUs,
    #[serde(rename = "zh-CN")]
    ZhCn,
}

/// User-facing settings document persisted as nested JSON.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SettingsDocument {
    /// Relative schema reference for editor tooling.
    #[serde(
        rename = "$schema",
        default = "default_schema_ref",
        skip_serializing_if = "Option::is_none"
    )]
    pub schema: Option<String>,
    /// Settings document schema version.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub general: GeneralSettingsConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub telemetry: OtelSettings,
    #[serde(default)]
    pub tools: ToolsConfig,
    #[serde(default)]
    pub agent: AgentSettingsConfig,
    #[serde(default)]
    pub runtime: RuntimeSettingsConfig,
    #[serde(default)]
    pub providers: ProvidersConfig,
    #[serde(default)]
    pub models: ModelSettingsConfig,
    #[serde(default)]
    pub plugin: PluginSettingsConfig,
    #[serde(default, skip_serializing_if = "WorkspaceSettingsConfig::is_empty")]
    pub workspace: WorkspaceSettingsConfig,
    #[serde(default)]
    pub server: ServerSettingsConfig,
}

impl Default for SettingsDocument {
    fn default() -> Self {
        Self {
            schema: default_schema_ref(),
            schema_version: default_schema_version(),
            general: GeneralSettingsConfig::default(),
            database: DatabaseConfig::default(),
            logging: LoggingConfig::default(),
            telemetry: OtelSettings::default(),
            tools: ToolsConfig::default(),
            agent: AgentSettingsConfig::default(),
            runtime: RuntimeSettingsConfig::default(),
            providers: ProvidersConfig::default(),
            models: ModelSettingsConfig::default(),
            plugin: PluginSettingsConfig::default(),
            workspace: WorkspaceSettingsConfig::default(),
            server: ServerSettingsConfig::default(),
        }
    }
}

/// General desktop-app settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct GeneralSettingsConfig {
    /// Preferred desktop interface language.
    #[serde(default)]
    pub language: InterfaceLanguagePreference,
}

impl Default for GeneralSettingsConfig {
    fn default() -> Self {
        Self { language: InterfaceLanguagePreference::Auto }
    }
}

/// Shared database configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DatabaseConfig {
    /// Database connection string.
    pub url: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self { url: "sqlite://slab.db?mode=rwc".to_owned() }
    }
}

/// Shared logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct LoggingConfig {
    /// Tracing filter or log level.
    #[serde(default = "default_log_level")]
    pub level: String,
    /// Emit newline-delimited JSON logs.
    #[serde(default)]
    pub json: bool,
    /// Optional log directory path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self { level: default_log_level(), json: false, path: None }
    }
}

/// Logging overrides applied on top of inherited defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct LoggingOverrideConfig {
    /// Override tracing filter or log level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    /// Override JSON logging mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json: Option<bool>,
    /// Override log directory path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Shared queue and concurrency limits.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct CapacityConfig {
    /// Submission queue size.
    #[serde(default = "default_runtime_queue")]
    pub queue: u32,
    /// Maximum in-flight requests for the target node.
    #[serde(default = "default_runtime_concurrent_requests")]
    pub concurrent_requests: u32,
}

impl Default for CapacityConfig {
    fn default() -> Self {
        default_root_capacity()
    }
}

/// Capacity overrides applied on top of inherited defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct CapacityOverrideConfig {
    /// Override submission queue size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue: Option<u32>,
    /// Override maximum in-flight requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concurrent_requests: Option<u32>,
}

/// HTTP and IPC endpoint configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct EndpointConfig {
    #[serde(default)]
    pub http: HttpEndpointConfig,
    #[serde(default)]
    pub ipc: IpcEndpointConfig,
}

/// HTTP endpoint configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct HttpEndpointConfig {
    /// Bind or target address used for HTTP/gRPC transport.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

/// IPC endpoint configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct IpcEndpointConfig {
    /// IPC socket or named-pipe path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Download source metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SourceConfig {
    /// Version or release identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Artifact name or asset selector.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<String>,
}

/// External tool configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ToolsConfig {
    #[serde(default)]
    pub ffmpeg: FfmpegToolConfig,
}

/// FFmpeg dependency configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct FfmpegToolConfig {
    /// Whether FFmpeg integration is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Whether FFmpeg should be auto-downloaded when missing.
    #[serde(default = "default_enabled")]
    pub auto_download: bool,
    /// Installation directory for the ffmpeg sidecar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_dir: Option<String>,
    #[serde(default)]
    pub source: SourceConfig,
}

impl Default for FfmpegToolConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_download: true,
            install_dir: None,
            source: SourceConfig::default(),
        }
    }
}

/// Agent-specific settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AgentSettingsConfig {
    #[serde(default = "default_enabled")]
    pub debug: bool,
    #[serde(default)]
    pub tools: AgentToolsConfig,
    #[serde(default)]
    pub hooks: AgentHooksConfig,
    #[serde(default)]
    pub memories: AgentMemoriesConfig,
}

impl Default for AgentSettingsConfig {
    fn default() -> Self {
        Self {
            debug: true,
            tools: AgentToolsConfig::default(),
            hooks: AgentHooksConfig::default(),
            memories: AgentMemoriesConfig::default(),
        }
    }
}

/// Agent lifecycle hook settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AgentHooksConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scripts: Vec<AgentHookScriptConfig>,
}

/// Explicit local script registered for agent lifecycle hook execution.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AgentHookScriptConfig {
    pub name: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub language: AgentHookScriptLanguage,
    pub root_dir: String,
    pub entry: String,
    #[serde(default = "default_hook_export_name")]
    pub export_name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentHookScriptLanguage {
    #[serde(rename = "javascript")]
    JavaScript,
    Python,
}

fn default_hook_export_name() -> String {
    "run".to_owned()
}

/// Agent memory pipeline settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AgentMemoriesConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_root: Option<String>,
    #[serde(default = "default_memory_phase1_scan_limit")]
    pub phase1_scan_limit: u32,
    #[serde(default = "default_memory_phase1_concurrency")]
    pub phase1_concurrency: u32,
    #[serde(default = "default_memory_phase1_idle_seconds")]
    pub phase1_idle_seconds: u64,
    #[serde(default = "default_memory_phase1_lease_seconds")]
    pub phase1_lease_seconds: u64,
    #[serde(default = "default_memory_phase1_retry_seconds")]
    pub phase1_retry_seconds: u64,
    #[serde(default = "default_memory_phase1_max_age_days")]
    pub phase1_max_age_days: u32,
    #[serde(default = "default_memory_phase2_limit")]
    pub phase2_limit: u32,
    #[serde(default = "default_memory_phase2_lease_seconds")]
    pub phase2_lease_seconds: u64,
    #[serde(default = "default_memory_max_unused_days")]
    pub max_unused_days: i64,
    #[serde(default = "default_memory_extension_retention_days")]
    pub extension_retention_days: i64,
}

impl Default for AgentMemoriesConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model: None,
            memory_root: None,
            phase1_scan_limit: default_memory_phase1_scan_limit(),
            phase1_concurrency: default_memory_phase1_concurrency(),
            phase1_idle_seconds: default_memory_phase1_idle_seconds(),
            phase1_lease_seconds: default_memory_phase1_lease_seconds(),
            phase1_retry_seconds: default_memory_phase1_retry_seconds(),
            phase1_max_age_days: default_memory_phase1_max_age_days(),
            phase2_limit: default_memory_phase2_limit(),
            phase2_lease_seconds: default_memory_phase2_lease_seconds(),
            max_unused_days: default_memory_max_unused_days(),
            extension_retention_days: default_memory_extension_retention_days(),
        }
    }
}

const fn default_memory_phase1_scan_limit() -> u32 {
    8
}

const fn default_memory_phase1_concurrency() -> u32 {
    2
}

const fn default_memory_phase1_idle_seconds() -> u64 {
    300
}

const fn default_memory_phase1_lease_seconds() -> u64 {
    900
}

const fn default_memory_phase1_retry_seconds() -> u64 {
    3600
}

const fn default_memory_phase1_max_age_days() -> u32 {
    30
}

const fn default_memory_phase2_limit() -> u32 {
    64
}

const fn default_memory_phase2_lease_seconds() -> u64 {
    1800
}

const fn default_memory_max_unused_days() -> i64 {
    180
}

const fn default_memory_extension_retention_days() -> i64 {
    30
}

/// Agent tool settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AgentToolsConfig {
    #[serde(default)]
    pub mcp: AgentMcpConfig,
    #[serde(default)]
    pub websearch: AgentWebSearchConfig,
}

/// Agent MCP tool integration settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AgentMcpConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub servers: Vec<AgentMcpServerConfig>,
}

/// External stdio MCP server registered for Agent tool access.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AgentMcpServerConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub name: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, AgentMcpEnvValueConfig>,
}

/// Environment value resolved from the host process environment at launch time.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AgentMcpEnvValueConfig {
    pub env_var: String,
}

/// Supported `websearch` crate providers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum WebSearchProviderId {
    #[default]
    Duckduckgo,
    Arxiv,
    Google,
    Tavily,
    Exa,
    Serpapi,
    Brave,
    Searxng,
}

impl WebSearchProviderId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Duckduckgo => "duckduckgo",
            Self::Arxiv => "arxiv",
            Self::Google => "google",
            Self::Tavily => "tavily",
            Self::Exa => "exa",
            Self::Serpapi => "serpapi",
            Self::Brave => "brave",
            Self::Searxng => "searxng",
        }
    }
}

impl std::fmt::Display for WebSearchProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for WebSearchProviderId {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "duckduckgo" => Ok(Self::Duckduckgo),
            "arxiv" => Ok(Self::Arxiv),
            "google" => Ok(Self::Google),
            "tavily" => Ok(Self::Tavily),
            "exa" => Ok(Self::Exa),
            "serpapi" => Ok(Self::Serpapi),
            "brave" => Ok(Self::Brave),
            "searxng" => Ok(Self::Searxng),
            _ => Err(format!("unsupported web search provider '{value}'")),
        }
    }
}

/// Agent web search tool configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AgentWebSearchConfig {
    #[serde(default)]
    pub default_provider: WebSearchProviderId,
    #[serde(default)]
    pub providers: WebSearchProvidersConfig,
}

impl Default for AgentWebSearchConfig {
    fn default() -> Self {
        Self {
            default_provider: WebSearchProviderId::Duckduckgo,
            providers: WebSearchProvidersConfig::default(),
        }
    }
}

/// Provider-specific configuration for the agent web search tool.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct WebSearchProvidersConfig {
    #[serde(default)]
    pub duckduckgo: WebSearchDuckDuckGoProviderConfig,
    #[serde(default)]
    pub arxiv: WebSearchArxivProviderConfig,
    #[serde(default)]
    pub google: WebSearchGoogleProviderConfig,
    #[serde(default)]
    pub tavily: WebSearchTavilyProviderConfig,
    #[serde(default)]
    pub exa: WebSearchExaProviderConfig,
    #[serde(default)]
    pub serpapi: WebSearchSerpApiProviderConfig,
    #[serde(default)]
    pub brave: WebSearchBraveProviderConfig,
    #[serde(default)]
    pub searxng: WebSearchSearxngProviderConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct WebSearchDuckDuckGoProviderConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_lite: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct WebSearchArxivProviderConfig {}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct WebSearchGoogleProviderConfig {
    #[serde(default)]
    pub auth: ProviderAuthConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cx: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct WebSearchTavilyProviderConfig {
    #[serde(default)]
    pub auth: ProviderAuthConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_depth: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_answer: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_images: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_raw_content: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct WebSearchExaProviderConfig {
    #[serde(default)]
    pub auth: ProviderAuthConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_contents: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct WebSearchSerpApiProviderConfig {
    #[serde(default)]
    pub auth: ProviderAuthConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct WebSearchBraveProviderConfig {
    #[serde(default)]
    pub auth: ProviderAuthConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct WebSearchSearxngProviderConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

/// Runtime topology mode.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMode {
    #[default]
    ManagedChildren,
    ExternalEndpoints,
}

/// Shared runtime configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RuntimeSettingsConfig {
    #[serde(default)]
    pub mode: RuntimeMode,
    #[serde(default = "default_runtime_transport")]
    pub transport: RuntimeTransportMode,
    #[serde(default)]
    pub sessions: RuntimeSessionsConfig,
    #[serde(default)]
    pub logging: LoggingOverrideConfig,
    #[serde(default = "default_root_capacity")]
    pub capacity: CapacityConfig,
    #[serde(default)]
    pub endpoint: EndpointConfig,
    #[serde(default)]
    pub ggml: GgmlRuntimeFamilyConfig,
    #[serde(default)]
    pub candle: SingleRuntimeFamilyConfig,
    #[serde(default)]
    pub onnx: SingleRuntimeFamilyConfig,
}

impl Default for RuntimeSettingsConfig {
    fn default() -> Self {
        Self {
            mode: RuntimeMode::ManagedChildren,
            transport: default_runtime_transport(),
            sessions: RuntimeSessionsConfig::default(),
            logging: LoggingOverrideConfig::default(),
            capacity: default_root_capacity(),
            endpoint: EndpointConfig::default(),
            ggml: GgmlRuntimeFamilyConfig::default(),
            candle: SingleRuntimeFamilyConfig::default(),
            onnx: SingleRuntimeFamilyConfig::default(),
        }
    }
}

const fn default_runtime_transport() -> RuntimeTransportMode {
    RuntimeTransportMode::Ipc
}

/// Shared session-state location for runtime-backed features.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RuntimeSessionsConfig {
    /// Directory where session state files are stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_dir: Option<String>,
}

/// GGML runtime family configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct GgmlRuntimeFamilyConfig {
    /// Shared install directory for GGML runtime libraries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_dir: Option<String>,
    #[serde(default)]
    pub source: SourceConfig,
    #[serde(default)]
    pub logging: LoggingOverrideConfig,
    #[serde(default)]
    pub capacity: CapacityOverrideConfig,
    #[serde(default)]
    pub endpoint: EndpointConfig,
    #[serde(default)]
    pub backends: GgmlRuntimeBackendsConfig,
}

/// GGML leaf backend configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct GgmlRuntimeBackendsConfig {
    #[serde(default)]
    pub llama: LlamaRuntimeLeafConfig,
    #[serde(default)]
    pub whisper: RuntimeLeafConfig,
    #[serde(default)]
    pub diffusion: RuntimeLeafConfig,
}

/// Shared runtime leaf configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RuntimeLeafConfig {
    /// Whether the leaf backend is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Whether Flash Attention is enabled for this backend when supported.
    #[serde(default = "defaults::flash_attn_enabled")]
    pub flash_attn: bool,
    #[serde(default)]
    pub source: SourceConfig,
    #[serde(default)]
    pub logging: LoggingOverrideConfig,
    #[serde(default)]
    pub capacity: CapacityOverrideConfig,
    #[serde(default)]
    pub endpoint: EndpointConfig,
}

impl Default for RuntimeLeafConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            flash_attn: defaults::flash_attn_enabled(),
            source: SourceConfig::default(),
            logging: LoggingOverrideConfig::default(),
            capacity: CapacityOverrideConfig::default(),
            endpoint: EndpointConfig::default(),
        }
    }
}

/// Llama leaf config with llama-specific controls.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct LlamaRuntimeLeafConfig {
    /// Whether the llama backend is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Optional context length override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u32>,
    /// Whether Flash Attention is enabled for llama contexts.
    #[serde(default = "defaults::flash_attn_enabled")]
    pub flash_attn: bool,
    #[serde(default)]
    pub source: SourceConfig,
    #[serde(default)]
    pub logging: LoggingOverrideConfig,
    #[serde(default)]
    pub capacity: CapacityOverrideConfig,
    #[serde(default)]
    pub endpoint: EndpointConfig,
}

impl Default for LlamaRuntimeLeafConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            context_length: Some(2048),
            flash_attn: defaults::flash_attn_enabled(),
            source: SourceConfig::default(),
            logging: LoggingOverrideConfig::default(),
            capacity: CapacityOverrideConfig::default(),
            endpoint: EndpointConfig::default(),
        }
    }
}

/// Single-node runtime family configuration used for candle and onnx.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SingleRuntimeFamilyConfig {
    /// Whether this runtime family is enabled.
    #[serde(default = "default_disabled")]
    pub enabled: bool,
    /// Install directory for family-specific artifacts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_dir: Option<String>,
    #[serde(default)]
    pub source: SourceConfig,
    #[serde(default)]
    pub logging: LoggingOverrideConfig,
    #[serde(default)]
    pub capacity: CapacityOverrideConfig,
    #[serde(default)]
    pub endpoint: EndpointConfig,
}

/// Global provider registry configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ProvidersConfig {
    #[serde(default)]
    pub registry: Vec<ProviderRegistryEntry>,
}

/// Supported provider transport families.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFamily {
    #[default]
    OpenaiCompatible,
}

/// A single global provider entry.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ProviderRegistryEntry {
    /// Stable provider identifier.
    pub id: String,
    /// Provider family implementation.
    #[serde(default)]
    pub family: ProviderFamily,
    /// Human-readable display name.
    pub display_name: String,
    /// Provider API base URL.
    pub api_base: String,
    #[serde(default)]
    pub auth: ProviderAuthConfig,
    #[serde(default)]
    pub defaults: ProviderDefaultsConfig,
}

/// Provider authentication settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ProviderAuthConfig {
    /// Literal API key stored in settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Environment variable containing the API key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
}

/// Provider-level default headers and query parameters.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ProviderDefaultsConfig {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub query: BTreeMap<String, String>,
}

/// Model storage settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ModelSettingsConfig {
    /// Directory used for cached model artifacts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_dir: Option<String>,
    /// Directory containing model configuration documents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_dir: Option<String>,
    /// Preferred remote source used when downloading model artifacts.
    #[serde(default)]
    pub download_source: ModelDownloadSourcePreference,
    #[serde(default)]
    pub auto_unload: AutoUnloadConfig,
}

impl Default for ModelSettingsConfig {
    fn default() -> Self {
        Self {
            cache_dir: None,
            config_dir: None,
            download_source: ModelDownloadSourcePreference::Auto,
            auto_unload: AutoUnloadConfig::default(),
        }
    }
}

/// Preferred remote source used when downloading model artifacts.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModelDownloadSourcePreference {
    #[default]
    Auto,
    HuggingFace,
    ModelScope,
}

/// Automatic model-unload settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AutoUnloadConfig {
    /// Whether idle models should be unloaded automatically.
    #[serde(default)]
    pub enabled: bool,
    /// Idle timeout before unloading a model.
    #[serde(default = "default_idle_minutes")]
    pub idle_minutes: u32,
    /// Minimum free system memory required before model loads can proceed without eviction.
    #[serde(default = "defaults::auto_unload_min_free_system_memory_bytes")]
    pub min_free_system_memory_bytes: u64,
    /// Minimum free GPU memory required before model loads can proceed without eviction.
    #[serde(default = "defaults::auto_unload_min_free_gpu_memory_bytes")]
    pub min_free_gpu_memory_bytes: u64,
    /// Maximum number of idle-model evictions attempted for a single model load.
    #[serde(default = "defaults::auto_unload_max_pressure_evictions_per_load")]
    pub max_pressure_evictions_per_load: u32,
}

impl Default for AutoUnloadConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            idle_minutes: default_idle_minutes(),
            min_free_system_memory_bytes: defaults::auto_unload_min_free_system_memory_bytes(),
            min_free_gpu_memory_bytes: defaults::auto_unload_min_free_gpu_memory_bytes(),
            max_pressure_evictions_per_load: defaults::auto_unload_max_pressure_evictions_per_load(
            ),
        }
    }
}

const fn default_idle_minutes() -> u32 {
    10
}

/// Runtime plugin installation settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Default)]
pub struct PluginSettingsConfig {
    /// Directory containing installed runtime plugin packages. Defaults to the `plugins`
    /// directory under the Slab application home.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_dir: Option<String>,
    /// Transport mode used for JS sidecar communication.
    #[serde(default)]
    pub js_runtime_transport: PluginJsRuntimeTransport,
    /// Transport mode used for Python sidecar communication.
    #[serde(default)]
    pub python_runtime_transport: PluginPythonRuntimeTransport,
}

/// Runtime transport used for JS plugin sidecar communication.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PluginJsRuntimeTransport {
    #[default]
    Stdio,
    Uds,
}

/// Runtime transport used for Python plugin sidecar communication.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PluginPythonRuntimeTransport {
    #[default]
    Stdio,
    Uds,
}

/// Workspace-local settings stored as an overlay on top of global settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct WorkspaceSettingsConfig {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub plugins: BTreeMap<String, WorkspacePluginSettingsConfig>,
}

impl WorkspaceSettingsConfig {
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

/// Workspace-local plugin settings keyed by plugin id.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct WorkspacePluginSettingsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Server-only settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ServerSettingsConfig {
    /// HTTP bind address for slab-server.
    pub address: String,
    #[serde(default)]
    pub logging: LoggingOverrideConfig,
    #[serde(default)]
    pub cors: CorsConfig,
    #[serde(default)]
    pub admin: AdminConfig,
    #[serde(default)]
    pub swagger: SwaggerConfig,
    /// Whether to log redacted cloud HTTP payloads.
    #[serde(default)]
    pub cloud_http_trace: bool,
}

impl Default for ServerSettingsConfig {
    fn default() -> Self {
        Self {
            address: DESKTOP_API_BIND.to_owned(),
            logging: LoggingOverrideConfig::default(),
            cors: CorsConfig::default(),
            admin: AdminConfig::default(),
            swagger: SwaggerConfig::default(),
            cloud_http_trace: false,
        }
    }
}

/// Server CORS settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct CorsConfig {
    /// Allowed browser origins.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_origins: Vec<String>,
}

/// Server admin settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AdminConfig {
    /// Optional bearer token required for admin APIs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

/// Swagger exposure settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SwaggerConfig {
    /// Whether Swagger UI and OpenAPI docs are exposed.
    #[serde(default = "default_swagger_enabled")]
    pub enabled: bool,
}

impl Default for SwaggerConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

fn sort_json_object_keys(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for child in map.values_mut() {
                sort_json_object_keys(child);
            }

            let sorted =
                std::mem::take(map).into_iter().collect::<BTreeMap<_, _>>().into_iter().collect();
            *map = sorted;
        }
        Value::Array(values) => {
            for child in values {
                sort_json_object_keys(child);
            }
        }
        _ => {}
    }
}

pub fn settings_document_json_schema() -> Value {
    let mut schema = serde_json::to_value(schema_for!(SettingsDocument))
        .expect("SettingsDocument schema should serialize");
    let root = schema.as_object_mut().expect("SettingsDocument schema root should be an object");

    root.insert(
        "$schema".into(),
        Value::String("https://json-schema.org/draft/2020-12/schema".into()),
    );
    root.insert("$id".into(), Value::String(PUBLIC_SETTINGS_DOCUMENT_SCHEMA_URL.into()));
    root.insert("title".into(), Value::String("Slab Settings Document".into()));
    root.insert(
        "description".into(),
        Value::String("Schema for the persisted settings document used by Slab hosts.".into()),
    );

    sort_json_object_keys(&mut schema);

    schema
}

pub fn render_settings_document_json_schema() -> String {
    let mut rendered = serde_json::to_string_pretty(&settings_document_json_schema())
        .expect("SettingsDocument schema should render");
    rendered.push('\n');
    rendered
}

pub fn provider_registry_json_schema() -> Value {
    json!({
        "type": "array",
        "title": "Provider Registry",
        "default": [],
        "items": {
            "type": "object",
            "title": "Provider Entry",
            "required": ["id", "family", "display_name", "api_base"],
            "properties": {
                "id": {
                    "type": "string",
                    "title": "Provider ID",
                    "description": "Stable provider identifier.",
                    "default": ""
                },
                "family": {
                    "type": "string",
                    "title": "Provider Family",
                    "enum": ["openai_compatible"],
                    "default": "openai_compatible"
                },
                "display_name": {
                    "type": "string",
                    "title": "Display Name",
                    "default": ""
                },
                "api_base": {
                    "type": "string",
                    "title": "API Base URL",
                    "default": ""
                },
                "auth": {
                    "type": "object",
                    "title": "Authentication",
                    "default": {
                        "api_key": null,
                        "api_key_env": null
                    },
                    "properties": {
                        "api_key": {
                            "type": ["string", "null"],
                            "title": "API Key",
                            "writeOnly": true,
                            "default": null
                        },
                        "api_key_env": {
                            "type": ["string", "null"],
                            "title": "API Key Environment Variable",
                            "default": null
                        }
                    }
                },
                "defaults": {
                    "type": "object",
                    "title": "Request Defaults",
                    "default": {
                        "headers": {},
                        "query": {}
                    },
                    "properties": {
                        "headers": {
                            "type": "object",
                            "title": "Headers",
                            "default": {},
                            "additionalProperties": { "type": "string" }
                        },
                        "query": {
                            "type": "object",
                            "title": "Query Parameters",
                            "default": {},
                            "additionalProperties": { "type": "string" }
                        }
                    }
                }
            }
        }
    })
}

pub fn websearch_providers_json_schema() -> Value {
    json!({
        "type": "object",
        "title": "Web Search Providers",
        "default": {},
        "properties": {
            "duckduckgo": {
                "type": "object",
                "title": "DuckDuckGo",
                "properties": {
                    "base_url": { "type": ["string", "null"], "default": null },
                    "user_agent": { "type": ["string", "null"], "default": null },
                    "use_lite": { "type": ["boolean", "null"], "default": null }
                }
            },
            "arxiv": {
                "type": "object",
                "title": "ArXiv",
                "default": {},
                "properties": {}
            },
            "google": {
                "type": "object",
                "title": "Google Custom Search",
                "properties": {
                    "auth": { "$ref": "#/$defs/webSearchAuth" },
                    "cx": { "type": ["string", "null"], "default": null },
                    "base_url": { "type": ["string", "null"], "default": null }
                }
            },
            "tavily": {
                "type": "object",
                "title": "Tavily",
                "properties": {
                    "auth": { "$ref": "#/$defs/webSearchAuth" },
                    "base_url": { "type": ["string", "null"], "default": null },
                    "search_depth": {
                        "type": ["string", "null"],
                        "enum": ["basic", "advanced", null],
                        "default": null
                    },
                    "include_answer": { "type": ["boolean", "null"], "default": null },
                    "include_images": { "type": ["boolean", "null"], "default": null },
                    "include_raw_content": { "type": ["boolean", "null"], "default": null }
                }
            },
            "exa": {
                "type": "object",
                "title": "Exa",
                "properties": {
                    "auth": { "$ref": "#/$defs/webSearchAuth" },
                    "base_url": { "type": ["string", "null"], "default": null },
                    "model": {
                        "type": ["string", "null"],
                        "enum": ["keyword", "embeddings", null],
                        "default": null
                    },
                    "include_contents": { "type": ["boolean", "null"], "default": null }
                }
            },
            "serpapi": {
                "type": "object",
                "title": "SerpAPI",
                "properties": {
                    "auth": { "$ref": "#/$defs/webSearchAuth" },
                    "engine": { "type": ["string", "null"], "default": null },
                    "base_url": { "type": ["string", "null"], "default": null }
                }
            },
            "brave": {
                "type": "object",
                "title": "Brave",
                "properties": {
                    "auth": { "$ref": "#/$defs/webSearchAuth" }
                }
            },
            "searxng": {
                "type": "object",
                "title": "SearXNG",
                "properties": {
                    "base_url": { "type": ["string", "null"], "default": null }
                }
            }
        },
        "$defs": {
            "webSearchAuth": {
                "type": "object",
                "title": "Authentication",
                "default": {
                    "api_key": null,
                    "api_key_env": null
                },
                "properties": {
                    "api_key": {
                        "type": ["string", "null"],
                        "title": "API Key",
                        "writeOnly": true,
                        "default": null
                    },
                    "api_key_env": {
                        "type": ["string", "null"],
                        "title": "API Key Environment Variable",
                        "default": null
                    }
                }
            }
        }
    })
}

pub fn mcp_servers_json_schema() -> Value {
    json!({
        "type": "array",
        "title": "MCP Servers",
        "default": [],
        "items": {
            "type": "object",
            "title": "MCP Server",
            "required": ["name", "command"],
            "properties": {
                "enabled": {
                    "type": "boolean",
                    "title": "Enabled",
                    "default": true
                },
                "name": {
                    "type": "string",
                    "title": "Server Name",
                    "description": "Stable local name used to route MCP tool calls.",
                    "default": ""
                },
                "command": {
                    "type": "string",
                    "title": "Command",
                    "description": "Executable used to launch the stdio MCP server.",
                    "default": ""
                },
                "args": {
                    "type": "array",
                    "title": "Arguments",
                    "default": [],
                    "items": {
                        "type": "string",
                        "default": ""
                    }
                },
                "cwd": {
                    "type": ["string", "null"],
                    "title": "Working Directory",
                    "default": null
                },
                "env": {
                    "type": "object",
                    "title": "Environment Variable References",
                    "description": "Map target MCP process environment names to host environment variable references. Secret values are resolved at launch and are not stored here.",
                    "default": {},
                    "additionalProperties": {
                        "type": "object",
                        "title": "Environment Reference",
                        "required": ["env_var"],
                        "properties": {
                            "env_var": {
                                "type": "string",
                                "title": "Host Environment Variable",
                                "default": ""
                            }
                        }
                    }
                }
            }
        }
    })
}

pub fn string_list_json_schema(title: &str) -> Value {
    json!({
        "type": "array",
        "title": title,
        "default": [],
        "items": {
            "type": "string",
            "default": ""
        }
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn document_defaults_to_current_schema() {
        let settings = SettingsDocument::default();

        assert_eq!(settings.schema.as_deref(), Some(PUBLIC_SETTINGS_DOCUMENT_SCHEMA_URL));
        assert_eq!(settings.schema_version, 2);
        assert_eq!(settings.general.language, InterfaceLanguagePreference::Auto);
        assert_eq!(
            settings.agent.tools.websearch.default_provider,
            WebSearchProviderId::Duckduckgo
        );
        assert!(settings.agent.debug);
        assert!(!settings.agent.hooks.enabled);
        assert!(settings.agent.hooks.scripts.is_empty());
        assert!(!settings.agent.memories.enabled);
        assert_eq!(settings.agent.memories.phase1_concurrency, 2);
        assert_eq!(settings.runtime.transport, RuntimeTransportMode::Ipc);
        assert_eq!(settings.server.address, DESKTOP_API_BIND);
        assert!(settings.runtime.logging.level.is_none());
        assert!(settings.telemetry.enabled);
        assert!(!settings.telemetry.capture_content);
        assert!(matches!(
            settings.telemetry.exporter,
            slab_otel::config::OtelExporter::LocalFile { .. }
        ));
        assert!(settings.workspace.plugins.is_empty());
    }

    #[test]
    fn runtime_leaf_defaults_match_intended_shape() {
        let settings = SettingsDocument::default();

        assert!(settings.runtime.ggml.backends.llama.enabled);
        assert!(settings.runtime.ggml.backends.whisper.enabled);
        assert!(settings.runtime.ggml.backends.llama.flash_attn);
        assert!(settings.runtime.ggml.backends.whisper.flash_attn);
        assert!(settings.runtime.ggml.backends.diffusion.flash_attn);
        assert!(!settings.runtime.candle.enabled);
        assert!(settings.runtime.ggml.backends.llama.capacity.concurrent_requests.is_none());
        assert_eq!(settings.runtime.capacity.concurrent_requests, 4);
    }

    #[test]
    fn generated_document_schema_is_root_object() {
        let schema = settings_document_json_schema();

        assert_eq!(schema.get("type"), Some(&Value::String("object".to_owned())));
        assert_eq!(
            schema.get("$id"),
            Some(&Value::String(PUBLIC_SETTINGS_DOCUMENT_SCHEMA_URL.to_owned()))
        );
        assert!(schema.get("properties").and_then(Value::as_object).is_some());
    }

    #[test]
    fn generated_document_schema_omits_runtime_resolved_telemetry_paths() {
        let schema = settings_document_json_schema();
        let telemetry_properties = schema
            .pointer("/$defs/OtelSettings/properties")
            .and_then(Value::as_object)
            .expect("telemetry properties");
        let telemetry_default = schema
            .pointer("/properties/telemetry/default")
            .and_then(Value::as_object)
            .expect("telemetry default");

        assert!(!telemetry_properties.contains_key("slab_home"));
        assert!(!telemetry_properties.contains_key("exporter"));
        assert!(!telemetry_properties.contains_key("trace_exporter"));
        assert!(!telemetry_default.contains_key("slab_home"));
        assert!(!telemetry_default.contains_key("exporter"));
        assert!(!telemetry_default.contains_key("trace_exporter"));
    }

    #[test]
    fn generated_document_schema_matches_checked_in_file() {
        let schema_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/public/manifests/v1/settings-document.schema.json");
        let expected = fs::read_to_string(&schema_path).expect("read checked-in schema");

        assert_eq!(render_settings_document_json_schema(), expected);
    }

    #[test]
    fn agent_hook_scripts_parse_with_default_export() {
        let settings = serde_json::from_value::<SettingsDocument>(json!({
            "agent": {
                "hooks": {
                    "scripts": [{
                        "name": "local-memory",
                        "language": "javascript",
                        "root_dir": "C:/hooks",
                        "entry": "hook.mjs",
                        "events": ["on_llm_start"]
                    }]
                }
            }
        }))
        .expect("settings");

        let script = &settings.agent.hooks.scripts[0];
        assert!(script.enabled);
        assert_eq!(script.export_name, "run");
        assert_eq!(script.language, AgentHookScriptLanguage::JavaScript);
    }

    #[test]
    fn agent_mcp_servers_parse_secret_env_refs() {
        let settings = serde_json::from_value::<SettingsDocument>(json!({
            "agent": {
                "tools": {
                    "mcp": {
                        "enabled": true,
                        "servers": [{
                            "name": "github",
                            "command": "npx",
                            "args": ["-y", "@modelcontextprotocol/server-github"],
                            "cwd": "C:/workspace",
                            "env": {
                                "GITHUB_PERSONAL_ACCESS_TOKEN": {
                                    "env_var": "GITHUB_TOKEN"
                                }
                            }
                        }]
                    }
                }
            }
        }))
        .expect("settings");

        let mcp = &settings.agent.tools.mcp;
        assert!(mcp.enabled);
        assert_eq!(mcp.servers.len(), 1);
        assert!(mcp.servers[0].enabled);
        assert_eq!(mcp.servers[0].name, "github");
        assert_eq!(mcp.servers[0].env["GITHUB_PERSONAL_ACCESS_TOKEN"].env_var, "GITHUB_TOKEN");
    }

    #[test]
    fn generated_provider_registry_schema_is_editor_friendly() {
        let schema = provider_registry_json_schema();
        let items = schema.get("items").and_then(Value::as_object).expect("items");
        let properties = items.get("properties").and_then(Value::as_object).expect("properties");
        let auth_properties = properties
            .get("auth")
            .and_then(Value::as_object)
            .and_then(|auth| auth.get("properties"))
            .and_then(Value::as_object)
            .expect("auth properties");

        assert!(properties.contains_key("api_base"));
        assert_eq!(
            auth_properties
                .get("api_key")
                .and_then(Value::as_object)
                .and_then(|value| value.get("writeOnly")),
            Some(&Value::Bool(true))
        );
    }

    #[test]
    fn generated_websearch_schema_marks_api_keys_write_only() {
        let schema = websearch_providers_json_schema();
        let api_key = schema
            .get("$defs")
            .and_then(Value::as_object)
            .and_then(|defs| defs.get("webSearchAuth"))
            .and_then(Value::as_object)
            .and_then(|auth| auth.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("api_key"))
            .and_then(Value::as_object)
            .expect("api key schema");

        assert_eq!(api_key.get("writeOnly"), Some(&Value::Bool(true)));
    }

    #[test]
    fn generated_mcp_servers_schema_keeps_env_values_as_references() {
        let schema = mcp_servers_json_schema();
        let env_schema = schema
            .pointer("/items/properties/env/additionalProperties/properties")
            .and_then(Value::as_object)
            .expect("mcp server env properties");

        assert!(env_schema.contains_key("env_var"));
        assert!(!env_schema.contains_key("value"));
    }

    #[test]
    fn generated_document_schema_keeps_mcp_env_values_as_references() {
        let schema = settings_document_json_schema();
        let env_properties = schema
            .pointer("/$defs/AgentMcpEnvValueConfig/properties")
            .and_then(Value::as_object)
            .expect("mcp env value properties");

        assert!(env_properties.contains_key("env_var"));
        assert!(!env_properties.contains_key("value"));
    }
}
