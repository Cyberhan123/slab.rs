use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::v1::session::schema::{
    CreateSessionRequest, MessageResponse, SessionIdPath, SessionResponse,
};
use crate::api::validation::{ValidatedJson, validate};
use crate::error::ServerError;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::SessionService;

#[derive(OpenApi)]
#[openapi(
    paths(create_session, list_sessions, delete_session, list_session_messages),
    components(schemas(CreateSessionRequest, SessionResponse, MessageResponse, SessionIdPath))
)]
pub struct SessionApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sessions", post(create_session).get(list_sessions))
        .route("/sessions/{id}", delete(delete_session))
        .route("/sessions/{id}/messages", get(list_session_messages))
}

#[utoipa::path(
    post,
    path = "/v1/sessions",
    tag = "sessions",
    request_body = CreateSessionRequest,
    responses(
        (status = 200, description = "Session created", body = SessionResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn create_session(
    State(service): State<SessionService>,
    ValidatedJson(req): ValidatedJson<CreateSessionRequest>,
) -> Result<Json<SessionResponse>, ServerError> {
    Ok(Json(service.create_session(req.into()).await?.into()))
}

#[utoipa::path(
    get,
    path = "/v1/sessions",
    tag = "sessions",
    responses(
        (status = 200, description = "Session list retrieved", body = Vec<SessionResponse>),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn list_sessions(
    State(service): State<SessionService>,
) -> Result<Json<Vec<SessionResponse>>, ServerError> {
    let sessions = service.list_sessions().await?.into_iter().map(Into::into).collect();
    Ok(Json(sessions))
}

#[utoipa::path(
    delete,
    path = "/v1/sessions/{id}",
    tag = "sessions",
    params(SessionIdPath),
    responses(
        (status = 200, description = "Session deleted", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn delete_session(
    State(service): State<SessionService>,
    Path(params): Path<SessionIdPath>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let params = validate(params)?;
    Ok(Json(service.delete_session(&params.id).await?))
}

#[utoipa::path(
    get,
    path = "/v1/sessions/{id}/messages",
    tag = "sessions",
    params(SessionIdPath),
    responses(
        (status = 200, description = "Session messages retrieved", body = Vec<MessageResponse>),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn list_session_messages(
    State(service): State<SessionService>,
    Path(params): Path<SessionIdPath>,
) -> Result<Json<Vec<MessageResponse>>, ServerError> {
    let params = validate(params)?;
    let messages =
        service.list_session_messages(&params.id).await?.into_iter().map(Into::into).collect();
    Ok(Json(messages))
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use utoipa::OpenApi;

    use super::SessionApi;

    fn operation_parameters<'a>(openapi: &'a Value, path: &str, method: &str) -> &'a Vec<Value> {
        openapi["paths"][path][method]["parameters"].as_array().expect("operation parameters")
    }

    #[test]
    fn session_routes_publish_path_id_parameter_in_openapi() {
        let openapi =
            serde_json::to_value(SessionApi::openapi()).expect("serialize session openapi");

        for (path, method) in
            [("/v1/sessions/{id}", "delete"), ("/v1/sessions/{id}/messages", "get")]
        {
            let parameters = operation_parameters(&openapi, path, method);
            assert!(parameters.iter().any(|parameter| {
                parameter["name"] == Value::String("id".to_owned())
                    && parameter["in"] == Value::String("path".to_owned())
            }));
        }
    }
}
