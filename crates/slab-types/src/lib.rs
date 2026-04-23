//! `slab-types` - shared semantic types, JSON schema definitions, and the PMID catalog.
//!
//! # Modules
//! - [`agent`] shared agent lifecycle and tool-call status enums.
//! - [`backend`] runtime-facing backend identifiers.
//! - [`chat`] shared chat conversation and inference-control types.
//! - [`common`] universal building blocks: [`common::Id`], [`common::Timestamp`].
//! - [`diffusion`] normalized diffusion request and response types.
//! - [`error`] crate-level error type.
//! - [`inference`] shared inference request and response types.
//! - [`media`] reusable image and frame payload types.
//! - [`plugin`] shared plugin manifest and contribution contracts.
//! - [`runtime`] shared runtime model and load specifications.
//! - [`settings`] PMID catalog and typed configuration snapshots for the settings system.
//! - [`whisper`] shared whisper VAD and decode option types.

pub mod agent;
pub mod asset_ref;
pub mod backend;
pub mod chat;
pub mod common;
pub mod desktop_api;
pub mod diffusion;
pub mod error;
pub mod inference;
pub mod load_config;
pub mod media;
pub mod plugin;
pub mod runtime;
pub mod settings;
pub mod whisper;

pub use agent::{AgentThreadStatus, ToolCallStatus};
pub use asset_ref::{AssetRef, GbnfAssetRef, TemplateAssetRef};
pub use backend::RuntimeBackendId;
pub use chat::{
    ChatModelSource, ChatReasoningEffort, ChatVerbosity, ConversationContentPart,
    ConversationMessage, ConversationMessageContent, ConversationToolCall,
    ConversationToolFunction,
};
pub use common::{Id, Timestamp};
pub use desktop_api::{
    DESKTOP_API_BIND, DESKTOP_API_HOST, DESKTOP_API_ORIGIN, DESKTOP_API_PORT,
    DESKTOP_DEV_ALLOWED_ORIGINS, desktop_api_bind, desktop_api_host, desktop_api_origin,
    desktop_api_port, desktop_dev_allowed_origins,
};
pub use diffusion::{
    DiffusionImageBackend, DiffusionImageRequest, DiffusionImageResponse, DiffusionRequestCommon,
    DiffusionVideoBackend, DiffusionVideoRequest, DiffusionVideoResponse, GgmlDiffusionImageParams,
    GgmlDiffusionVideoParams,
};
pub use error::{SlabTypeError, ValidationError};
pub use inference::{
    AudioTranscriptionOpOptions, AudioTranscriptionRequest, AudioTranscriptionResponse,
    ImageEmbeddingRequest, ImageEmbeddingResponse, ImageGenerationRequest, ImageGenerationResponse,
    JsonOptions, TextGenerationChunk, TextGenerationOpOptions, TextGenerationRequest,
    TextGenerationResponse,
};
pub use load_config::{
    CandleDiffusionLoadConfig, CandleLlamaLoadConfig, CandleWhisperLoadConfig,
    GgmlDiffusionLoadConfig, GgmlLlamaLoadConfig, GgmlLlamaLoadDefaultsConfig,
    GgmlWhisperLoadConfig, OnnxLoadConfig, RuntimeBackendLoadSpec,
};
pub use media::{GeneratedFrame, GeneratedImage, RawImageInput};
pub use plugin::{
    PluginAgentCapabilityContribution, PluginCapabilityKind, PluginCapabilityTransport,
    PluginCapabilityTransportType, PluginCommandContribution, PluginCompatibilityManifest,
    PluginContributesManifest, PluginFilePermissions, PluginInfo, PluginIntegrityManifest,
    PluginManifest, PluginNetworkManifest, PluginNetworkMode, PluginPermissionsManifest,
    PluginRouteContribution, PluginRuntimeManifest, PluginSettingsContribution,
    PluginSidebarContribution, PluginUiManifest, PluginWasmManifest,
};
pub use runtime::{
    Capability, DiffusionLoadOptions, DriverDescriptor, DriverHints, DriverLoadStyle, ModelFamily,
    ModelSource, ModelSourceKind, ModelSpec, RuntimeModelLoadCommand, RuntimeModelLoadSpec,
    RuntimeModelStatus,
};
pub use whisper::{WhisperDecodeOptions, WhisperVadOptions, WhisperVadParams};
