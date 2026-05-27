use serde::{Deserialize, Serialize};

/// ApplyPatchCallOutputStatusParam : Outcome values reported for apply_patch tool call outputs.
/// Outcome values reported for apply_patch tool call outputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum ApplyPatchCallOutputStatusParam {
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
}

impl std::fmt::Display for ApplyPatchCallOutputStatusParam {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

impl Default for ApplyPatchCallOutputStatusParam {
    fn default() -> ApplyPatchCallOutputStatusParam {
        Self::Completed
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum ApplyPatchCallOutputStatus {
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
}

impl std::fmt::Display for ApplyPatchCallOutputStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

impl Default for ApplyPatchCallOutputStatus {
    fn default() -> ApplyPatchCallOutputStatus {
        Self::Completed
    }
}

/// ApplyPatchCallStatusParam : ApplyPatchCallOutputStatus values reported for apply_patch tool calls.
/// ApplyPatchCallOutputStatus values reported for apply_patch tool calls.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum ApplyPatchCallStatusParam {
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
}

impl std::fmt::Display for ApplyPatchCallStatusParam {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::InProgress => write!(f, "in_progress"),
            Self::Completed => write!(f, "completed"),
        }
    }
}

impl Default for ApplyPatchCallStatusParam {
    fn default() -> ApplyPatchCallStatusParam {
        Self::InProgress
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum ApplyPatchCallStatus {
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
}

impl std::fmt::Display for ApplyPatchCallStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::InProgress => write!(f, "in_progress"),
            Self::Completed => write!(f, "completed"),
        }
    }
}

impl Default for ApplyPatchCallStatus {
    fn default() -> ApplyPatchCallStatus {
        Self::InProgress
    }
}
