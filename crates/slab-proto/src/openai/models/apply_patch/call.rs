use serde::{Deserialize, Serialize};

/// ApplyPatchCallOutputStatusParam : Outcome values reported for apply_patch tool call outputs.
/// Outcome values reported for apply_patch tool call outputs.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ApplyPatchCallOutputStatusParam {
    #[serde(rename = "completed")]
    #[default]
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

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ApplyPatchCallOutputStatus {
    #[serde(rename = "completed")]
    #[default]
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

/// ApplyPatchCallStatusParam : ApplyPatchCallOutputStatus values reported for apply_patch tool calls.
/// ApplyPatchCallOutputStatus values reported for apply_patch tool calls.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ApplyPatchCallStatusParam {
    #[serde(rename = "in_progress")]
    #[default]
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

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ApplyPatchCallStatus {
    #[serde(rename = "in_progress")]
    #[default]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_display_matches_wire_values() {
        assert_eq!(ApplyPatchCallOutputStatusParam::Completed.to_string(), "completed");
        assert_eq!(ApplyPatchCallOutputStatusParam::Failed.to_string(), "failed");
        assert_eq!(ApplyPatchCallOutputStatus::Completed.to_string(), "completed");
        assert_eq!(ApplyPatchCallOutputStatus::Failed.to_string(), "failed");
        assert_eq!(ApplyPatchCallStatusParam::InProgress.to_string(), "in_progress");
        assert_eq!(ApplyPatchCallStatusParam::Completed.to_string(), "completed");
        assert_eq!(ApplyPatchCallStatus::InProgress.to_string(), "in_progress");
        assert_eq!(ApplyPatchCallStatus::Completed.to_string(), "completed");
    }
}
