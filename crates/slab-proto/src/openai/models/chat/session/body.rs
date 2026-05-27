use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateChatSessionBody {
    /// Workflow that powers the session.
    #[serde(rename = "workflow")]
    pub workflow: Box<models::WorkflowParam>,
    /// A free-form string that identifies your end user; ensures this Session can access other objects that have the same `user` scope.
    #[serde(rename = "user")]
    pub user: String,
    /// Optional override for session expiration timing in seconds from creation. Defaults to 10 minutes.
    #[serde(rename = "expires_after", skip_serializing_if = "Option::is_none")]
    pub expires_after: Option<Box<serde_json::Value>>,
    /// Optional override for per-minute request limits. When omitted, defaults to 10.
    #[serde(rename = "rate_limits", skip_serializing_if = "Option::is_none")]
    pub rate_limits: Option<Box<serde_json::Value>>,
    /// Optional overrides for ChatKit runtime configuration features
    #[serde(rename = "chatkit_configuration", skip_serializing_if = "Option::is_none")]
    pub chatkit_configuration: Option<Box<models::ChatkitConfigurationParam>>,
}

impl CreateChatSessionBody {
    /// Parameters for provisioning a new ChatKit session.
    pub fn new(workflow: models::WorkflowParam, user: String) -> CreateChatSessionBody {
        CreateChatSessionBody {
            workflow: Box::new(workflow),
            user,
            expires_after: None,
            rate_limits: None,
            chatkit_configuration: None,
        }
    }
}
