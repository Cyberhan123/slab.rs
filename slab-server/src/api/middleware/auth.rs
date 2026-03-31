use crate::context::AppState;
use axum::{
    extract::{Request, State},
    http::{StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let provided = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    // If no admin token is configured, management routes are open to all callers.
    // When SLAB_ADMIN_TOKEN is set, the caller must supply a matching
    // `Authorization: Bearer <token>` header.
    let is_authorized = match state.context.config.admin_api_token.as_deref() {
        None => true,
        Some(expected) => provided.map(|p| p == expected).unwrap_or(false),
    };

    if is_authorized { next.run(req).await } else { StatusCode::UNAUTHORIZED.into_response() }
}
