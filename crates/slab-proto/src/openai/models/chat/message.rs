use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    /// The role of the message (e.g. \"system\", \"assistant\", \"user\").
    #[serde(rename = "role")]
    pub role: String,
    /// The content of the message.
    #[serde(rename = "content")]
    pub content: String,
}

impl ChatMessage {
    pub fn new(role: String, content: String) -> ChatMessage {
        ChatMessage { role, content }
    }
}
