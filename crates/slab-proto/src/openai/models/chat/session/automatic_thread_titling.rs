use serde::{Deserialize, Serialize};

/// ChatSessionAutomaticThreadTitling : Automatic thread title preferences for the session.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatSessionAutomaticThreadTitling {
    /// Whether automatic thread titling is enabled.
    #[serde(rename = "enabled")]
    pub enabled: bool,
}

impl ChatSessionAutomaticThreadTitling {
    /// Automatic thread title preferences for the session.
    pub fn new(enabled: bool) -> ChatSessionAutomaticThreadTitling {
        ChatSessionAutomaticThreadTitling { enabled }
    }
}
