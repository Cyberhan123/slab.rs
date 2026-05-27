use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum RoleDiscriminator {
    #[serde(rename = "assistant")]
    Assistant,
}

impl Default for RoleDiscriminator {
    fn default() -> RoleDiscriminator {
        Self::Assistant
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Role {
    /// Always `role`.
    #[serde(rename = "object")]
    pub object: Object,
    /// Identifier for the role.
    #[serde(rename = "id")]
    pub id: String,
    /// Unique name for the role.
    #[serde(rename = "name")]
    pub name: String,
    /// Optional description of the role.
    #[serde(rename = "description", deserialize_with = "Option::deserialize")]
    pub description: Option<String>,
    /// Permissions granted by the role.
    #[serde(rename = "permissions")]
    pub permissions: Vec<String>,
    /// Resource type the role is bound to (for example `api.organization` or `api.project`).
    #[serde(rename = "resource_type")]
    pub resource_type: String,
    /// Whether the role is predefined and managed by OpenAI.
    #[serde(rename = "predefined_role")]
    pub predefined_role: bool,
}

impl Role {
    /// Details about a role that can be assigned through the public Roles API.
    pub fn new(
        object: Object,
        id: String,
        name: String,
        description: Option<String>,
        permissions: Vec<String>,
        resource_type: String,
        predefined_role: bool,
    ) -> Role {
        Role { object, id, name, description, permissions, resource_type, predefined_role }
    }
}
/// Always `role`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum Object {
    #[serde(rename = "role")]
    Role,
}

impl Default for Object {
    fn default() -> Object {
        Self::Role
    }
}
