use crate::state::AppState;
use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    req: Request,
    next: Next,
) -> Response {
    let provided = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let is_authorized = provided
        .zip(state.config.admin_api_token.as_deref())
        .map(|(p, a)| p == a)
        .unwrap_or(false);

    if is_authorized {
        return next.run(req).await;
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}
