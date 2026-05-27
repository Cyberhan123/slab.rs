use crate::models;
use serde::{Deserialize, Serialize};

/// ChatSessionResource : Represents a ChatKit session and its resolved configuration.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatSessionResource {
    /// Identifier for the ChatKit session.
    #[serde(rename = "id")]
    pub id: String,
    /// Type discriminator that is always `chatkit.session`.
    #[serde(rename = "object")]
    pub object: Object,
    /// Unix timestamp (in seconds) for when the session expires.
    #[serde(rename = "expires_at")]
    pub expires_at: i32,
    /// Ephemeral client secret that authenticates session requests.
    #[serde(rename = "client_secret")]
    pub client_secret: String,
    /// Workflow metadata for the session.
    #[serde(rename = "workflow")]
    pub workflow: Box<models::ChatkitWorkflow>,
    /// User identifier associated with the session.
    #[serde(rename = "user")]
    pub user: String,
    /// Resolved rate limit values.
    #[serde(rename = "rate_limits")]
    pub rate_limits: Box<models::ChatSessionRateLimits>,
    /// Convenience copy of the per-minute request limit.
    #[serde(rename = "max_requests_per_1_minute")]
    pub max_requests_per_1_minute: i32,
    /// Current lifecycle state of the session.
    #[serde(rename = "status")]
    pub status: models::ChatSessionStatus,
    /// Resolved ChatKit feature configuration for the session.
    #[serde(rename = "chatkit_configuration")]
    pub chatkit_configuration: Box<models::ChatSessionChatkitConfiguration>,
}

impl ChatSessionResource {
    /// Represents a ChatKit session and its resolved configuration.
    pub fn new(
        id: String,
        object: Object,
        expires_at: i32,
        client_secret: String,
        workflow: models::ChatkitWorkflow,
        user: String,
        rate_limits: models::ChatSessionRateLimits,
        max_requests_per_1_minute: i32,
        status: models::ChatSessionStatus,
        chatkit_configuration: models::ChatSessionChatkitConfiguration,
    ) -> ChatSessionResource {
        ChatSessionResource {
            id,
            object,
            expires_at,
            client_secret,
            workflow: Box::new(workflow),
            user,
            rate_limits: Box::new(rate_limits),
            max_requests_per_1_minute,
            status,
            chatkit_configuration: Box::new(chatkit_configuration),
        }
    }
}
/// Type discriminator that is always `chatkit.session`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum Object {
    #[serde(rename = "chatkit.session")]
    ChatkitSession,
}

impl Default for Object {
    fn default() -> Object {
        Self::ChatkitSession
    }
}
