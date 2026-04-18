use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::launch::LaunchConfig;

const fn default_flash_attn_enabled() -> bool {
    true
}

/// A configured cloud/remote AI provider.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct CloudProviderConfig {
    /// Unique provider identifier.
    #[serde(alias = "provider_id", alias = "providerId")]
    pub id: String,
    /// Human-readable display name.
    #[serde(default, alias = "displayName", alias = "provider_name")]
    pub name: String,
    /// Base URL for the provider's API.
    #[serde(alias = "apiBase", alias = "base_url", alias = "baseUrl")]
    pub api_base: String,
    /// Optional API key (stored as plain text; treat as sensitive).
    #[serde(default, alias = "apiKey", skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Optional environment variable that holds the API key.
    #[serde(default, alias = "apiKeyEnv", skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
}

// ── Snapshot of all PMID-managed settings ────────────────────────────────────

/// Typed snapshot of all PMID-managed settings values.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PmidConfig {
    pub setup: SetupConfig,
    pub runtime: RuntimeConfig,
    pub launch: LaunchConfig,
    pub chat: ChatConfig,
    pub diffusion: DiffusionConfig,
}

/// Setup / first-run settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SetupConfig {
    /// Whether the initial setup wizard has been completed.
    pub initialized: bool,
    pub ffmpeg: SetupFfmpegConfig,
    pub backends: SetupBackendsConfig,
}

/// FFmpeg-related setup settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SetupFfmpegConfig {
    /// Whether FFmpeg should be downloaded automatically when not found.
    /// Reserved for future use; not yet wired to any download logic.
    pub auto_download: bool,
    /// Custom directory for the FFmpeg binary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
}

/// Backend library setup settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SetupBackendsConfig {
    /// Directory where backend libraries are stored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
}

// ── Runtime settings ─────────────────────────────────────────────────────────

/// Runtime engine settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RuntimeConfig {
    /// Directory used to cache downloaded models.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_cache_dir: Option<String>,
    pub llama: RuntimeLlamaConfig,
    pub whisper: RuntimeWhisperConfig,
    pub diffusion: RuntimeWorkerConfig,
    pub model_auto_unload: RuntimeModelAutoUnloadConfig,
}

/// Llama runtime settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RuntimeLlamaConfig {
    /// Number of parallel llama workers.
    pub num_workers: u32,
    /// Context window length in tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u32>,
    #[serde(default = "default_flash_attn_enabled")]
    pub flash_attn: bool,
}

impl Default for RuntimeLlamaConfig {
    fn default() -> Self {
        Self { num_workers: 0, context_length: None, flash_attn: true }
    }
}

/// Whisper runtime settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RuntimeWhisperConfig {
    /// Number of parallel workers for this backend.
    pub num_workers: u32,
    #[serde(default = "default_flash_attn_enabled")]
    pub flash_attn: bool,
}

impl Default for RuntimeWhisperConfig {
    fn default() -> Self {
        Self { num_workers: 0, flash_attn: true }
    }
}

/// Generic single-backend worker settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RuntimeWorkerConfig {
    /// Number of parallel workers for this backend.
    pub num_workers: u32,
}

/// Model auto-unload settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RuntimeModelAutoUnloadConfig {
    /// Whether idle models should be unloaded automatically.
    pub enabled: bool,
    /// Minutes of inactivity before a model is unloaded.
    pub idle_minutes: u32,
}

// ── Chat settings ─────────────────────────────────────────────────────────────

/// Chat / LLM provider settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ChatConfig {
    /// Configured cloud/remote AI providers.
    pub providers: Vec<CloudProviderConfig>,
}

// ── Diffusion settings ────────────────────────────────────────────────────────

/// Image diffusion settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DiffusionConfig {
    pub paths: DiffusionPathsConfig,
    pub performance: DiffusionPerformanceConfig,
}

/// Paths to diffusion model files.
///
/// Fields are stored as `Option<String>` (raw setting values from the PMID store).
/// Callers that perform file I/O should convert to `PathBuf` at the boundary where
/// the path is used, consistent with how `DiffusionLoadOptions` (which uses `PathBuf`)
/// is populated from these values.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DiffusionPathsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vae: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taesd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lora_model_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip_l: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip_g: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t5xxl: Option<String>,
}

/// Diffusion performance tuning settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DiffusionPerformanceConfig {
    #[serde(default = "default_flash_attn_enabled")]
    pub flash_attn: bool,
    pub vae_device: String,
    pub clip_device: String,
    pub offload_params_to_cpu: bool,
}

impl Default for DiffusionPerformanceConfig {
    fn default() -> Self {
        Self {
            flash_attn: true,
            vae_device: String::new(),
            clip_device: String::new(),
            offload_params_to_cpu: false,
        }
    }
}
