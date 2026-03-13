use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigEntry {
    pub key: String,
    pub value: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct SetConfigBody {
    #[validate(
        custom(
            function = "crate::api::validation::validate_non_blank",
            message = "name must not be empty"
        )
    )]
    pub name: Option<String>,
    pub value: String,
}
