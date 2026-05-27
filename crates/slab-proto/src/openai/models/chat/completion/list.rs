use crate::openai::models;
use serde::{Deserialize, Serialize};

/// ChatCompletionList : An object representing a list of Chat Completions.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionList {
    /// The type of this object. It is always set to \"list\".
    #[serde(rename = "object")]
    pub object: Object,
    /// An array of chat completion objects.
    #[serde(rename = "data")]
    pub data: Vec<models::CreateChatCompletionResponse>,
    /// The identifier of the first chat completion in the data array.
    #[serde(rename = "first_id")]
    pub first_id: String,
    /// The identifier of the last chat completion in the data array.
    #[serde(rename = "last_id")]
    pub last_id: String,
    /// Indicates whether there are more Chat Completions available.
    #[serde(rename = "has_more")]
    pub has_more: bool,
}

impl ChatCompletionList {
    /// An object representing a list of Chat Completions.
    pub fn new(
        object: Object,
        data: Vec<models::CreateChatCompletionResponse>,
        first_id: String,
        last_id: String,
        has_more: bool,
    ) -> ChatCompletionList {
        ChatCompletionList { object, data, first_id, last_id, has_more }
    }
}
/// The type of this object. It is always set to \"list\".
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum Object {
    #[serde(rename = "list")]
    #[default]
    List,
}
