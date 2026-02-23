use axum::http::{Request, StatusCode};
use axum::middleware::{ Next};
use axum::response::{IntoResponse, Response};
use axum::{body::Body};


pub async fn check_management_auth(req: Request<Body>, next: Next) -> Response {
    let expected = std::env::var("SLAB_ADMIN_TOKEN").ok();
    if let Some(expected_token) = expected {
        let provided = req
            .headers()
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));
        match provided {
            Some(token) if token == expected_token => {}
            _ => {
                return (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(serde_json::json!({ "error": "unauthorised" })),
                )
                    .into_response();
            }
        }
    }
    next.run(req).await
}