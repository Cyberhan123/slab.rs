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
//! - [`runtime`] shared runtime model and load specifications.
//! - [`settings`] PMID catalog and typed configuration snapshots for the settings system.
//! - [`whisper`] shared whisper VAD and decode option types.

pub mod agent;
pub mod backend;
pub mod chat;
pub mod common;
pub mod diffusion;
pub mod error;
pub mod inference;
pub mod load_config;
pub mod media;
pub mod runtime;
pub mod settings;
pub mod whisper;

pub use agent::{AgentThreadStatus, ToolCallStatus};
pub use backend::RuntimeBackendId;
pub use chat::{
    ChatModelSource, ChatReasoningEffort, ChatVerbosity, ConversationContentPart,
    ConversationMessage, ConversationMessageContent, ConversationToolCall,
    ConversationToolFunction,
};
pub use common::{Id, Timestamp};
pub use diffusion::{
    DiffusionImageRequest, DiffusionImageResponse, DiffusionVideoRequest, DiffusionVideoResponse,
};
pub use error::SlabTypeError;
pub use inference::{
    AudioTranscriptionOpOptions, AudioTranscriptionRequest, AudioTranscriptionResponse,
    ImageGenerationRequest, ImageGenerationResponse, JsonOptions, TextGenerationChunk,
    TextGenerationOpOptions, TextGenerationRequest, TextGenerationResponse,
};
pub use load_config::{
    CandleDiffusionLoadConfig, CandleLlamaLoadConfig, CandleWhisperLoadConfig,
    GgmlDiffusionLoadConfig, GgmlLlamaLoadConfig, GgmlWhisperLoadConfig, OnnxLoadConfig,
    RuntimeBackendLoadSpec,
};
pub use media::{GeneratedFrame, GeneratedImage, RawImageInput};
pub use runtime::{
    Capability, DiffusionLoadOptions, DriverHints, ModelFamily, ModelSource, ModelSpec,
    RuntimeModelLoadCommand, RuntimeModelLoadSpec, RuntimeModelStatus,
};
pub use whisper::{WhisperDecodeOptions, WhisperVadOptions, WhisperVadParams};
