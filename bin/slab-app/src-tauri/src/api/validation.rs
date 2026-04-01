use slab_app_core::error::AppCoreError;
use validator::Validate;

pub(crate) fn map_err(error: AppCoreError) -> String {
    error.to_string()
}

pub(crate) fn validate<T: Validate>(value: T) -> Result<T, String> {
    value.validate().map_err(|error| error.to_string())?;
    Ok(value)
}

pub(crate) fn validate_id(id: &str) -> Result<(), String> {
    slab_app_core::schemas::validation::validate_non_blank(id).map_err(|error| error.to_string())
}
