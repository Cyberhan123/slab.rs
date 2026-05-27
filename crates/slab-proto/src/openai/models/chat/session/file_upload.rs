use serde::{Deserialize, Serialize};

/// ChatSessionFileUpload : Upload permissions and limits applied to the session.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatSessionFileUpload {
    /// Indicates if uploads are enabled for the session.
    #[serde(rename = "enabled")]
    pub enabled: bool,
    /// Maximum upload size in megabytes.
    #[serde(rename = "max_file_size", deserialize_with = "Option::deserialize")]
    pub max_file_size: Option<i32>,
    /// Maximum number of uploads allowed during the session.
    #[serde(rename = "max_files", deserialize_with = "Option::deserialize")]
    pub max_files: Option<i32>,
}

impl ChatSessionFileUpload {
    /// Upload permissions and limits applied to the session.
    pub fn new(
        enabled: bool,
        max_file_size: Option<i32>,
        max_files: Option<i32>,
    ) -> ChatSessionFileUpload {
        ChatSessionFileUpload { enabled, max_file_size, max_files }
    }
}
