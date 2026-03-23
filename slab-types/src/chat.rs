//! Shared chat types used across `slab-server`, `slab-runtime`, and `slab-core`.
//!
//! These are the canonical semantic types for the chat subsystem.  They intentionally
//! carry no HTTP, SSE, or transport-layer concerns so they can be freely reused
//! across crate boundaries without pulling in server or runtime dependencies.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A single message in a conversation, identified by its role and text content.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ConversationMessage {
    /// The role of the message author (`"system"`, `"user"`, or `"assistant"`).
    pub role: String,
    /// The text content of the message.
    pub content: String,
}

/// Reasoning effort hint for inference providers that support chain-of-thought control.
///
/// Maps directly to provider-level reasoning parameters (e.g. DeepSeek, OpenAI o-series).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatReasoningEffort {
    None,
    Low,
    Medium,
    High,
    Minimal,
}

impl ChatReasoningEffort {
    /// Returns the canonical lowercase string representation of this variant.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Minimal => "minimal",
        }
    }
}

/// Verbosity hint for inference providers that expose thinking-trace verbosity control.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatVerbosity {
    Low,
    Medium,
    High,
}

impl ChatVerbosity {
    /// Returns the canonical lowercase string representation of this variant.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

/// Identifies whether a chat model option is backed by a local (on-device) or cloud-hosted model.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChatModelSource {
    Local,
    Cloud,
}

impl ChatModelSource {
    /// Returns the canonical lowercase string representation of this variant.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Cloud => "cloud",
        }
    }
}
