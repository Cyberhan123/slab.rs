//! `slab-types` - shared semantic types, JSON schema definitions, and the PMID catalog.
//!
//! # Modules
//! - [`agent`] shared agent lifecycle and tool-call status enums.
//! - [`backend`] runtime-facing backend identifiers.
//! - [`chat`] shared chat conversation and inference-control types.
//! - [`common`] universal building blocks: [`common::Id`], [`common::JsonOptions`],
//!   [`common::Timestamp`].
//! - [`error`] crate-level error type.
//! - [`plugin`] shared plugin manifest and contribution contracts.
//! - [`runtime`] shared runtime model and load specifications.
//! - [`settings`] PMID catalog and typed configuration snapshots for the settings system.
//! - [`sqlite`] SQLite URL formatting helpers shared by desktop and server crates.

pub mod agent;
pub mod asset_ref;
pub mod backend;
pub mod chat;
pub mod common;
pub mod desktop_api;
pub mod error;
pub mod load_config;
pub mod plugin;
pub mod runtime;
pub mod settings;
pub mod sqlite;

pub use agent::{AgentThreadStatus, ToolCallStatus};
pub use asset_ref::{AssetRef, GbnfAssetRef, TemplateAssetRef};
pub use backend::RuntimeBackendId;
pub use chat::{
    ChatModelSource, ChatReasoningEffort, ChatVerbosity, ConversationContentPart,
    ConversationMessage, ConversationMessageContent, ConversationToolCall,
    ConversationToolFunction,
};
pub use common::{Id, JsonOptions, Timestamp};
pub use desktop_api::{
    DESKTOP_API_BIND, DESKTOP_API_HOST, DESKTOP_API_ORIGIN, DESKTOP_API_PORT,
    DESKTOP_DEV_ALLOWED_ORIGINS, desktop_api_bind, desktop_api_host, desktop_api_origin,
    desktop_api_port, desktop_dev_allowed_origins,
};
pub use error::{SlabTypeError, ValidationError};
pub use load_config::{
    CandleDiffusionLoadConfig, CandleLlamaLoadConfig, CandleWhisperLoadConfig,
    GgmlDiffusionLoadConfig, GgmlLlamaLoadConfig, GgmlLlamaLoadDefaultsConfig,
    GgmlWhisperLoadConfig, OnnxLoadConfig, RuntimeBackendLoadSpec,
};
pub use plugin::{
    PluginAgentCapabilityContribution, PluginCapabilityKind, PluginCapabilityTransport,
    PluginCapabilityTransportType, PluginCommandContribution, PluginCompatibilityManifest,
    PluginContributesManifest, PluginFilePermissions, PluginInfo, PluginIntegrityManifest,
    PluginLanguageServerContribution, PluginLanguageServerTransport, PluginManifest,
    PluginNetworkManifest, PluginNetworkMode, PluginPermissionsManifest, PluginRouteContribution,
    PluginRuntimeManifest, PluginSettingsContribution, PluginSidebarContribution, PluginUiManifest,
    PluginWasmManifest,
};
pub use runtime::{
    Capability, DiffusionLoadOptions, DriverDescriptor, DriverHints, DriverLoadStyle, ModelFamily,
    ModelSource, ModelSourceKind, ModelSpec, RuntimeModelLoadCommand, RuntimeModelLoadSpec,
    RuntimeModelStatus,
};
pub use sqlite::sqlite_url_for_path;
