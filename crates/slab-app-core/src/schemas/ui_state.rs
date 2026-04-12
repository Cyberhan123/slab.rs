use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::domain::models::{DeleteUiStateView, UiStateValueView, UpdateUiStateCommand};

#[derive(Debug, Clone, Deserialize, ToSchema, IntoParams, Validate)]
pub struct UiStateKeyPath {
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "key must not be empty"
    ))]
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UiStateValueResponse {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UiStateDeleteResponse {
    pub key: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, Deserialize, ToSchema, Validate)]
pub struct UpdateUiStateRequest {
    pub value: String,
}

impl From<UiStateValueView> for UiStateValueResponse {
    fn from(view: UiStateValueView) -> Self {
        Self { key: view.key, value: view.value, updated_at: view.updated_at }
    }
}

impl From<DeleteUiStateView> for UiStateDeleteResponse {
    fn from(view: DeleteUiStateView) -> Self {
        Self { key: view.key, deleted: view.deleted }
    }
}

impl From<UpdateUiStateRequest> for UpdateUiStateCommand {
    fn from(request: UpdateUiStateRequest) -> Self {
        Self { value: request.value }
    }
}
