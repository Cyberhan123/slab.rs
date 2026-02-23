
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigEntry {
    pub key: String,
    pub value: String,
}


#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SetConfigBody {
    pub value: String,
}
