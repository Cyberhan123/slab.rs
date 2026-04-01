use serde::Serialize;
use slab_app_core::error::AppCoreError;
use validator::Validate;

#[derive(Serialize)]
struct TauriErrorPayload {
    code: u16,
    data: Option<serde_json::Value>,
    message: String,
    status: u16,
}

pub(crate) fn map_err(error: AppCoreError) -> String {
    match error {
        AppCoreError::NotFound(message) => encode_error(404, 4004, message, None),
        AppCoreError::BadRequest(message) => encode_error(400, 4000, message, None),
        AppCoreError::BadRequestData { message, data } => {
            encode_error(400, 4000, message, Some(data))
        }
        AppCoreError::BackendNotReady(message) => encode_error(503, 5003, message, None),
        AppCoreError::NotImplemented(message) => encode_error(501, 5010, message, None),
        AppCoreError::TooManyRequests(message) => encode_error(429, 4029, message, None),
        AppCoreError::Runtime(_) => encode_error(
            500,
            5000,
            "An error occurred while processing your request.".to_owned(),
            None,
        ),
        AppCoreError::Database(_) => encode_error(
            500,
            5001,
            "A database error occurred. Please try again later.".to_owned(),
            None,
        ),
        AppCoreError::Internal(_) => encode_error(
            500,
            5002,
            "An internal server error occurred. Please try again later.".to_owned(),
            None,
        ),
    }
}

pub(crate) fn validate<T: Validate>(value: T) -> Result<T, String> {
    value.validate().map_err(|error| encode_error(400, 4000, error.to_string(), None))?;
    Ok(value)
}

pub(crate) fn validate_id(id: &str) -> Result<(), String> {
    slab_app_core::schemas::validation::validate_non_blank(id)
        .map_err(|error| encode_error(400, 4000, error.to_string(), None))
}

fn encode_error(
    status: u16,
    code: u16,
    message: String,
    data: Option<serde_json::Value>,
) -> String {
    serde_json::to_string(&TauriErrorPayload { code, data, message, status }).unwrap_or_else(|_| {
        format!(r#"{{"code":5002,"data":null,"message":"internal error","status":500}}"#)
    })
}
