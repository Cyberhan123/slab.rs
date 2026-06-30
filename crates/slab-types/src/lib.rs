//! `slab-types` - shared semantic types and contract definitions.
//!
//! # Modules
//! - [`agent`] shared agent lifecycle and tool-call status enums.
//! - [`backend`] runtime-facing backend identifiers.
//! - [`chat`] shared chat conversation and inference-control types.
//! - [`common`] universal building blocks: [`common::Id`], [`common::JsonOptions`],
//!   [`common::Timestamp`].
//! - [`error`] crate-level error type.
//! - [`i18n`] server-to-frontend internationalization references.
//! - [`plugin`] shared plugin manifest and contribution contracts.
//! - [`plugin_runtime`] shared JS plugin runtime JSON-RPC contracts.
//! - [`runtime`] shared runtime model and load specifications.
//! - [`sqlite`] SQLite URL formatting helpers shared by desktop and server crates.

pub mod agent;
pub mod asset_ref;
pub mod backend;
pub mod chat;
pub mod common;
mod defaults;
pub mod desktop_api;
pub mod device;
pub mod error;
pub mod i18n;
pub mod load_config;
pub mod plugin;
pub mod plugin_capability;
pub mod plugin_runtime;
pub mod runtime;
pub mod sqlite;

pub use agent::{AgentThreadStatus, ToolCallStatus};
pub use asset_ref::{AssetRef, GbnfAssetRef, TemplateAssetRef};
pub use backend::RuntimeBackendId;
pub use chat::{
    ChatModelSource, ChatReasoningEffort, ChatVerbosity, ConversationContentPart,
    ConversationMessage, ConversationMessageContent, ConversationToolCall,
    ConversationToolFunction, StructuredOutput, StructuredOutputJsonSchema,
};
pub use common::{Id, JsonOptions, Timestamp};
pub use desktop_api::{
    DESKTOP_API_BIND, DESKTOP_API_HOST, DESKTOP_API_ORIGIN, DESKTOP_API_PORT,
    DESKTOP_DEV_ALLOWED_ORIGINS, desktop_api_bind, desktop_api_host, desktop_api_origin,
    desktop_api_port, desktop_dev_allowed_origins,
};
pub use device::RuntimeDevicePreference;
pub use error::{SlabTypeError, ValidationError};
pub use i18n::{I18nMessageRef, I18nPayload, ServerI18nKey};
pub use load_config::{
    CandleDiffusionLoadConfig, CandleLlamaLoadConfig, CandleWhisperLoadConfig,
    GgmlDiffusionLoadConfig, GgmlLlamaLoadConfig, GgmlLlamaLoadDefaultsConfig,
    GgmlWhisperLoadConfig, OnnxLoadConfig, RuntimeBackendLoadSpec,
};
pub use plugin::{
    PluginAgentCapabilityContribution, PluginAgentHookContribution, PluginAgentHookLifecycleEvent,
    PluginAgentHookRuntime, PluginAgentHookTransport, PluginCapabilityKind,
    PluginCapabilityTransport, PluginCapabilityTransportType, PluginCommandContribution,
    PluginCompatibilityManifest, PluginContributesManifest, PluginFilePermissions, PluginInfo,
    PluginIntegrityManifest, PluginJsManifest, PluginLanguageServerContribution,
    PluginLanguageServerTransport, PluginManifest, PluginNetworkManifest, PluginNetworkMode,
    PluginPermissionsManifest, PluginPythonManifest, PluginRouteContribution,
    PluginRuntimeManifest, PluginSettingsContribution, PluginSidebarContribution, PluginUiManifest,
    PluginWasmManifest,
};
pub use plugin_runtime::{
    PluginApiRequest, PluginApiResponse, PluginEventPayload, PluginRuntimeApiHostRequest,
    PluginRuntimeCallRequest, PluginRuntimeCallResponse, PluginRuntimeFileAccess,
    PluginRuntimeFileGrant, PluginRuntimeUiEmitRequest, authorize_plugin_slab_api_request,
    required_plugin_slab_api_permission,
};
pub use runtime::{
    ArtifactFormat, Capability, DiffusionLoadOptions, DriverDescriptor, DriverHints,
    DriverLoadStyle, ModelFamily, ModelSource, ModelSourceKind, ModelSpec, RuntimeModelLoadCommand,
    RuntimeModelStatus,
};
pub use sqlite::sqlite_url_for_path;
