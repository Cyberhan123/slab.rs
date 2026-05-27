use crate::openai::models;
use serde::{Deserialize, Serialize};

// GenerateSummary not yet generated; use serde_json::Value as placeholder
type GenerateSummary = serde_json::Value;
use super::misc::Summary;
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reasoning {
    #[serde(
        rename = "effort",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub effort: Option<Option<models::ReasoningEffort>>,
    /// A summary of the reasoning performed by the model. This can be useful for debugging and understanding the model's reasoning process. One of `auto`, `concise`, or `detailed`.  `concise` is supported for `computer-use-preview` models and all reasoning models after `gpt-5`.
    #[serde(
        rename = "summary",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub summary: Option<Option<Summary>>,
    /// **Deprecated:** use `summary` instead.  A summary of the reasoning performed by the model. This can be useful for debugging and understanding the model's reasoning process. One of `auto`, `concise`, or `detailed`.
    #[serde(
        rename = "generate_summary",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub generate_summary: Option<Option<GenerateSummary>>,
}

impl Reasoning {
    /// **gpt-5 and o-series models only**  Configuration options for [reasoning models](https://platform.openai.com/docs/guides/reasoning).
    pub fn new() -> Reasoning {
        Reasoning { effort: None, summary: None, generate_summary: None }
    }
}
// A summary of the reasoning performed by the model. This can be useful for debugging and understanding the model's reasoning process. One of `auto`, `concise`, or `detailed`.  `concise` is supported for `computer-use-preview` models and all reasoning models after `gpt-5`.

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ReasoningEffort {
    #[serde(rename = "none")]
    #[default]
    None,
    #[serde(rename = "minimal")]
    Minimal,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "xhigh")]
    Xhigh,
}

impl std::fmt::Display for ReasoningEffort {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Minimal => write!(f, "minimal"),
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Xhigh => write!(f, "xhigh"),
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReasoningItem {
    /// The type of the object. Always `reasoning`.
    #[serde(rename = "type")]
    pub r#type: ReasoningItemType,
    /// The unique identifier of the reasoning content.
    #[serde(rename = "id")]
    pub id: String,
    /// Reasoning summary content.
    #[serde(rename = "summary")]
    pub summary: Vec<models::SummaryTextContent>,
    /// The encrypted content of the reasoning item - populated when a response is generated with `reasoning.encrypted_content` in the `include` parameter.
    #[serde(
        rename = "encrypted_content",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub encrypted_content: Option<Option<String>>,
    /// Reasoning text content.
    #[serde(rename = "content", skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<models::ReasoningTextContent>>,
    /// The status of the item. One of `in_progress`, `completed`, or `incomplete`. Populated when items are returned via API.
    #[serde(rename = "status", skip_serializing_if = "Option::is_none")]
    pub status: Option<models::Status>,
}

impl ReasoningItem {
    /// A description of the chain of thought used by a reasoning model while generating a response. Be sure to include these items in your `input` to the Responses API for subsequent turns of a conversation if you are manually [managing context](/docs/guides/conversation-state).
    pub fn new(
        r#type: ReasoningItemType,
        id: String,
        summary: Vec<models::SummaryTextContent>,
    ) -> ReasoningItem {
        ReasoningItem { r#type, id, summary, encrypted_content: None, content: None, status: None }
    }
}
/// The type of the object. Always `reasoning`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ReasoningItemType {
    #[serde(rename = "reasoning")]
    #[default]
    Reasoning,
}

// The status of the item. One of `in_progress`, `completed`, or `incomplete`. Populated when items are returned via API.

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReasoningTextContent {
    /// The type of the reasoning text. Always `reasoning_text`.
    #[serde(rename = "type")]
    pub r#type: ReasoningTextContentType,
    /// The reasoning text from the model.
    #[serde(rename = "text")]
    pub text: String,
}

impl ReasoningTextContent {
    /// Reasoning text from the model.
    pub fn new(r#type: ReasoningTextContentType, text: String) -> ReasoningTextContent {
        ReasoningTextContent { r#type, text }
    }
}
/// The type of the reasoning text. Always `reasoning_text`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ReasoningTextContentType {
    #[serde(rename = "reasoning_text")]
    #[default]
    ReasoningText,
}
