use serde::{Deserialize, Serialize};

/// ChatSessionRateLimits : Active per-minute request limit for the session.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatSessionRateLimits {
    /// Maximum allowed requests per one-minute window.
    #[serde(rename = "max_requests_per_1_minute")]
    pub max_requests_per_1_minute: i32,
}

impl ChatSessionRateLimits {
    /// Active per-minute request limit for the session.
    pub fn new(max_requests_per_1_minute: i32) -> ChatSessionRateLimits {
        ChatSessionRateLimits { max_requests_per_1_minute }
    }
}
