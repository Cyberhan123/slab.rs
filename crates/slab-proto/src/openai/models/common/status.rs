use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActiveStatus {
    /// Status discriminator that is always `active`.
    #[serde(rename = "type")]
    pub r#type: ActiveStatusType,
}

impl ActiveStatus {
    /// Indicates that a thread is active.
    pub fn new(r#type: ActiveStatusType) -> ActiveStatus {
        ActiveStatus { r#type }
    }
}
/// ActiveStatus discriminator that is always `active`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum ActiveStatusType {
    #[serde(rename = "active")]
    Active,
}

impl Default for ActiveStatusType {
    fn default() -> ActiveStatusType {
        Self::Active
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClosedStatus {
    /// Status discriminator that is always `closed`.
    #[serde(rename = "type")]
    pub r#type: ClosedStatusType,
    /// Reason that the thread was closed. Defaults to null when no reason is recorded.
    #[serde(rename = "reason", deserialize_with = "Option::deserialize")]
    pub reason: Option<String>,
}

impl ClosedStatus {
    /// Indicates that a thread has been closed.
    pub fn new(r#type: ClosedStatusType, reason: Option<String>) -> ClosedStatus {
        ClosedStatus { r#type, reason }
    }
}
/// ClosedStatus discriminator that is always `closed`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum ClosedStatusType {
    #[serde(rename = "closed")]
    Closed,
}

impl Default for ClosedStatusType {
    fn default() -> ClosedStatusType {
        Self::Closed
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct LockedStatus {
    /// Status discriminator that is always `locked`.
    #[serde(rename = "type")]
    pub r#type: LockedStatusType,
    /// Reason that the thread was locked. Defaults to null when no reason is recorded.
    #[serde(rename = "reason", deserialize_with = "Option::deserialize")]
    pub reason: Option<String>,
}

impl LockedStatus {
    /// Indicates that a thread is locked and cannot accept new input.
    pub fn new(r#type: LockedStatusType, reason: Option<String>) -> LockedStatus {
        LockedStatus { r#type, reason }
    }
}
/// LockedStatus discriminator that is always `locked`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum LockedStatusType {
    #[serde(rename = "locked")]
    Locked,
}

impl Default for LockedStatusType {
    fn default() -> LockedStatusType {
        Self::Locked
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum Status {
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "incomplete")]
    Incomplete,
}

impl Default for Status {
    fn default() -> Status {
        Self::InProgress
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum ClientToolCallStatus {
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
}

impl std::fmt::Display for ClientToolCallStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::InProgress => write!(f, "in_progress"),
            Self::Completed => write!(f, "completed"),
        }
    }
}

impl Default for ClientToolCallStatus {
    fn default() -> ClientToolCallStatus {
        Self::InProgress
    }
}
