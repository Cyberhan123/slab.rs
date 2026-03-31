//! Shared chat types used across `slab-server`, `slab-runtime`, and `slab-core`.
//!
//! These are the canonical semantic types for the chat subsystem.  They intentionally
//! carry no HTTP, SSE, or transport-layer concerns so they can be freely reused
//! across crate boundaries without pulling in server or runtime dependencies.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A single message in a conversation.
///
/// The schema is intentionally richer than plain `role + text` so higher layers
/// can preserve multimodal parts, tool results, and assistant tool calls even
/// when a specific backend eventually needs to flatten them to text.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ConversationMessage {
    /// The role of the message author.
    pub role: String,
    /// The message body. String payloads remain supported for backward compatibility.
    #[serde(default)]
    pub content: ConversationMessageContent,
    /// Optional participant name for providers that support named turns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Tool call id attached to tool result messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Assistant-emitted tool calls.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ConversationToolCall>,
}

/// Backward-compatible message content.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(untagged)]
pub enum ConversationMessageContent {
    Text(String),
    Parts(Vec<ConversationContentPart>),
}

impl Default for ConversationMessageContent {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

/// A structured content fragment inside a chat message.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConversationContentPart {
    Text {
        text: String,
    },
    InputText {
        text: String,
    },
    OutputText {
        text: String,
    },
    Image {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        image_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    ToolResult {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_call_id: Option<String>,
        value: serde_json::Value,
    },
    Json {
        value: serde_json::Value,
    },
    Refusal {
        text: String,
    },
}

/// Assistant tool-call envelope.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ConversationToolCall {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default = "default_tool_call_type")]
    pub r#type: String,
    pub function: ConversationToolFunction,
}

/// Function-style tool call payload.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ConversationToolFunction {
    pub name: String,
    #[serde(default)]
    pub arguments: String,
}

fn default_tool_call_type() -> String {
    "function".to_owned()
}

impl ConversationMessage {
    /// Returns `true` when the message carries any meaningful user-visible data.
    pub fn has_meaningful_content(&self) -> bool {
        self.content.has_meaningful_content() || !self.tool_calls.is_empty()
    }

    /// Best-effort textual rendering used by prompt templates and providers that
    /// only accept plain text turns.
    pub fn rendered_text(&self) -> String {
        let mut segments = Vec::new();

        let content_text = self.content.rendered_text();
        if !content_text.is_empty() {
            segments.push(content_text);
        }

        if let Some(tool_call_id) = self.tool_call_id.as_deref().filter(|value| !value.is_empty()) {
            segments.push(format!("tool_call_id: {tool_call_id}"));
        }

        if !self.tool_calls.is_empty() {
            let tool_call_lines = self
                .tool_calls
                .iter()
                .map(|tool_call| {
                    let id = tool_call
                        .id
                        .as_deref()
                        .map(|value| format!(" id={value}"))
                        .unwrap_or_default();
                    format!(
                        "tool_call{idl}: {}({})",
                        tool_call.function.name,
                        tool_call.function.arguments,
                        idl = id
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            if !tool_call_lines.is_empty() {
                segments.push(tool_call_lines);
            }
        }

        segments.join("\n")
    }
}

impl ConversationMessageContent {
    pub fn has_meaningful_content(&self) -> bool {
        match self {
            Self::Text(text) => !text.trim().is_empty(),
            Self::Parts(parts) => parts.iter().any(ConversationContentPart::has_meaningful_content),
        }
    }

    pub fn rendered_text(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Parts(parts) => parts
                .iter()
                .map(ConversationContentPart::rendered_text)
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

impl ConversationContentPart {
    pub fn has_meaningful_content(&self) -> bool {
        match self {
            Self::Text { text }
            | Self::InputText { text }
            | Self::OutputText { text }
            | Self::Refusal { text } => !text.trim().is_empty(),
            Self::Image { image_url, mime_type, .. } => {
                image_url.as_deref().is_some_and(|value| !value.trim().is_empty())
                    || mime_type.as_deref().is_some_and(|value| !value.trim().is_empty())
            }
            Self::ToolResult { .. } | Self::Json { .. } => true,
        }
    }

    pub fn rendered_text(&self) -> String {
        match self {
            Self::Text { text } | Self::InputText { text } | Self::OutputText { text } => {
                text.clone()
            }
            Self::Image { image_url, mime_type, detail } => {
                let url = image_url.as_deref().unwrap_or("embedded");
                let mime = mime_type.as_deref().unwrap_or("unknown");
                let detail = detail
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .map(|value| format!(" detail={value}"))
                    .unwrap_or_default();
                format!("[image mime={mime} src={url}{detail}]")
            }
            Self::ToolResult { tool_call_id, value } => {
                let rendered = serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned());
                let prefix = tool_call_id
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .map(|value| format!("tool_result[{value}]"))
                    .unwrap_or_else(|| "tool_result".to_owned());
                format!("{prefix}: {rendered}")
            }
            Self::Json { value } => {
                serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned())
            }
            Self::Refusal { text } => format!("[refusal] {text}"),
        }
    }
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

/// Route-level chat capabilities exposed to clients via `GET /v1/chat/models`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
pub struct ChatModelCapabilities {
    /// Whether the route accepts raw `grammar` constraints.
    pub raw_grammar: bool,
    /// Whether the route accepts structured output controls.
    pub structured_output: bool,
    /// Whether the route accepts reasoning/verbosity controls.
    pub reasoning_controls: bool,
}

impl ChatModelCapabilities {
    pub fn local() -> Self {
        Self { raw_grammar: true, structured_output: true, reasoning_controls: false }
    }

    pub fn cloud() -> Self {
        Self { raw_grammar: false, structured_output: true, reasoning_controls: true }
    }
}
