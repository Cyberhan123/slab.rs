use serde::{Deserialize, Serialize};

/// ChatSessionHistory : History retention preferences returned for the session.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatSessionHistory {
    /// Indicates if chat history is persisted for the session.
    #[serde(rename = "enabled")]
    pub enabled: bool,
    /// Number of prior threads surfaced in history views. Defaults to null when all history is retained.
    #[serde(rename = "recent_threads", deserialize_with = "Option::deserialize")]
    pub recent_threads: Option<i32>,
}

impl ChatSessionHistory {
    /// History retention preferences returned for the session.
    pub fn new(enabled: bool, recent_threads: Option<i32>) -> ChatSessionHistory {
        ChatSessionHistory { enabled, recent_threads }
    }
}
