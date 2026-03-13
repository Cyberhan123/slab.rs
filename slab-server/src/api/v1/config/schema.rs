use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

use crate::domain::models::ConfigEntryView;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigEntry {
    pub key: String,
    pub value: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct SetConfigBody {
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "name must not be empty"
    ))]
    pub name: Option<String>,
    pub value: String,
}

impl From<ConfigEntryView> for ConfigEntry {
    fn from(entry: ConfigEntryView) -> Self {
        Self {
            key: entry.key,
            value: entry.value,
            name: entry.name,
        }
    }
}
