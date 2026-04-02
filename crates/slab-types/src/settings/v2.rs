use std::collections::BTreeMap;

use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::launch::RuntimeTransportMode;

fn default_schema_ref() -> Option<String> {
    Some("./settings-schema.json".to_owned())
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

/// V2 user-facing settings document persisted as nested JSON.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SettingsDocumentV2 {
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
    pub database: DatabaseConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub tools: ToolsConfig,
    #[serde(default)]
    pub runtime: RuntimeConfigV2,
    #[serde(default)]
    pub providers: ProvidersConfig,
    #[serde(default)]
    pub models: ModelsConfigV2,
    #[serde(default)]
    pub server: ServerConfigV2,
}

impl Default for SettingsDocumentV2 {
    fn default() -> Self {
        Self {
            schema: default_schema_ref(),
            schema_version: default_schema_version(),
            database: DatabaseConfig::default(),
            logging: LoggingConfig::default(),
            tools: ToolsConfig::default(),
            runtime: RuntimeConfigV2::default(),
            providers: ProvidersConfig::default(),
            models: ModelsConfigV2::default(),
            server: ServerConfigV2::default(),
        }
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
pub struct RuntimeConfigV2 {
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

impl Default for RuntimeConfigV2 {
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
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
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

impl Default for GgmlRuntimeFamilyConfig {
    fn default() -> Self {
        Self {
            install_dir: None,
            source: SourceConfig::default(),
            logging: LoggingOverrideConfig::default(),
            capacity: CapacityOverrideConfig::default(),
            endpoint: EndpointConfig::default(),
            backends: GgmlRuntimeBackendsConfig::default(),
        }
    }
}

/// GGML leaf backend configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct GgmlRuntimeBackendsConfig {
    #[serde(default)]
    pub llama: LlamaRuntimeLeafConfig,
    #[serde(default)]
    pub whisper: RuntimeLeafConfig,
    #[serde(default)]
    pub diffusion: RuntimeLeafConfig,
}

impl Default for GgmlRuntimeBackendsConfig {
    fn default() -> Self {
        Self {
            llama: LlamaRuntimeLeafConfig::default(),
            whisper: RuntimeLeafConfig::default(),
            diffusion: RuntimeLeafConfig::default(),
        }
    }
}

/// Shared runtime leaf configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RuntimeLeafConfig {
    /// Whether the leaf backend is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
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
            context_length: None,
            source: SourceConfig::default(),
            logging: LoggingOverrideConfig::default(),
            capacity: CapacityOverrideConfig::default(),
            endpoint: EndpointConfig::default(),
        }
    }
}

/// Single-node runtime family configuration used for candle and onnx.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
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

impl Default for SingleRuntimeFamilyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            install_dir: None,
            source: SourceConfig::default(),
            logging: LoggingOverrideConfig::default(),
            capacity: CapacityOverrideConfig::default(),
            endpoint: EndpointConfig::default(),
        }
    }
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
pub struct ModelsConfigV2 {
    /// Directory used for cached model artifacts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_dir: Option<String>,
    /// Directory containing model configuration documents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_dir: Option<String>,
    #[serde(default)]
    pub auto_unload: AutoUnloadConfig,
}

impl Default for ModelsConfigV2 {
    fn default() -> Self {
        Self { cache_dir: None, config_dir: None, auto_unload: AutoUnloadConfig::default() }
    }
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
}

impl Default for AutoUnloadConfig {
    fn default() -> Self {
        Self { enabled: false, idle_minutes: default_idle_minutes() }
    }
}

const fn default_idle_minutes() -> u32 {
    10
}

/// Server-only settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ServerConfigV2 {
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

impl Default for ServerConfigV2 {
    fn default() -> Self {
        Self {
            address: "127.0.0.1:3000".to_owned(),
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

pub fn settings_document_v2_json_schema() -> Value {
    serde_json::to_value(schema_for!(SettingsDocumentV2))
        .expect("SettingsDocumentV2 schema should serialize")
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
    use super::*;

    #[test]
    fn document_defaults_to_v2_schema() {
        let settings = SettingsDocumentV2::default();

        assert_eq!(settings.schema.as_deref(), Some("./settings-schema.json"));
        assert_eq!(settings.schema_version, 2);
        assert_eq!(settings.runtime.transport, RuntimeTransportMode::Ipc);
        assert_eq!(settings.server.address, "127.0.0.1:3000");
        assert!(settings.runtime.logging.level.is_none());
    }

    #[test]
    fn runtime_leaf_defaults_match_intended_shape() {
        let settings = SettingsDocumentV2::default();

        assert!(settings.runtime.ggml.backends.llama.enabled);
        assert!(settings.runtime.ggml.backends.whisper.enabled);
        assert!(!settings.runtime.candle.enabled);
        assert!(settings.runtime.ggml.backends.llama.capacity.concurrent_requests.is_none());
        assert_eq!(settings.runtime.capacity.concurrent_requests, 4);
    }

    #[test]
    fn generated_document_schema_is_root_object() {
        let schema = settings_document_v2_json_schema();

        assert_eq!(schema.get("type"), Some(&Value::String("object".to_owned())));
        assert!(schema.get("properties").and_then(Value::as_object).is_some());
    }

    #[test]
    fn generated_provider_registry_schema_is_editor_friendly() {
        let schema = provider_registry_json_schema();
        let items = schema.get("items").and_then(Value::as_object).expect("items");
        let properties = items
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");
        let auth_properties = properties
            .get("auth")
            .and_then(Value::as_object)
            .and_then(|auth| auth.get("properties"))
            .and_then(Value::as_object)
            .expect("auth properties");

        assert!(properties.contains_key("api_base"));
        assert_eq!(auth_properties.get("api_key").and_then(Value::as_object).and_then(|value| value.get("writeOnly")), Some(&Value::Bool(true)));
    }
}