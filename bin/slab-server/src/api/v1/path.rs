use serde::Deserialize;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub(super) struct IdPath {
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "id must not be empty"
    ))]
    pub id: String,
}
