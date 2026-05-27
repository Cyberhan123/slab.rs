use serde::{Deserialize, Serialize};

/// ChatCompletionRole : The role of the author of a message
/// The role of the author of a message
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum ChatCompletionRole {
    #[serde(rename = "developer")]
    Developer,
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "tool")]
    Tool,
    #[serde(rename = "function")]
    Function,
}

impl std::fmt::Display for ChatCompletionRole {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Developer => write!(f, "developer"),
            Self::System => write!(f, "system"),
            Self::User => write!(f, "user"),
            Self::Assistant => write!(f, "assistant"),
            Self::Tool => write!(f, "tool"),
            Self::Function => write!(f, "function"),
        }
    }
}

impl Default for ChatCompletionRole {
    fn default() -> ChatCompletionRole {
        Self::Developer
    }
}
