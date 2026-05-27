use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct UpdateChatCompletionRequest {
    /// Set of 16 key-value pairs that can be attached to an object. This can be useful for storing additional information about the object in a structured format, and querying for objects via API or the dashboard.  Keys are strings with a maximum length of 64 characters. Values are strings with a maximum length of 512 characters.
    #[serde(rename = "metadata", deserialize_with = "Option::deserialize")]
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

impl UpdateChatCompletionRequest {
    pub fn new(
        metadata: Option<std::collections::HashMap<String, String>>,
    ) -> UpdateChatCompletionRequest {
        UpdateChatCompletionRequest { metadata }
    }
}
