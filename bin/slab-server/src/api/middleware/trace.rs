use axum::{
    body::{Body, Bytes},
    extract::{Request, State},
    http::{HeaderValue, header},
    middleware::Next,
    response::Response,
};
use http_body_util::BodyExt; // cargo add http_body_util
use slab_app_core::context::AppState;
use std::sync::Arc;
use std::time::Instant;
use tracing::{Instrument, info, info_span};
use uuid::Uuid;

pub static X_TRACE_ID: &str = "x-trace-id";

pub async fn trace_middleware(
    State(_state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let start_time = Instant::now();

    let trace_id = req
        .headers()
        .get(X_TRACE_ID)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or_else(Uuid::new_v4);

    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let skip_logging = path == "/v1/system/gpu";

    let span = if skip_logging {
        tracing::Span::none()
    } else {
        info_span!(
            "http_request",
            trace_id = %trace_id,
            method = %method,
            path = %path,
        )
    };

    async move {
        if !skip_logging {
            info!("→ request started");
        }
        let (parts, body) = req.into_parts();

        let req_bytes =
            buffer_and_log("request", &trace_id.to_string(), &parts.headers, body, skip_logging)
                .await;
        let mut req = Request::from_parts(parts, Body::from(req_bytes));

        // SAFETY: UUID strings consist only of ASCII hex digits and hyphens,
        // which are always valid HTTP header value bytes.
        let trace_id_value = HeaderValue::from_str(&trace_id.to_string())
            .expect("UUID is always a valid HTTP header value");
        req.headers_mut().insert(X_TRACE_ID, trace_id_value.clone());

        let response = next.run(req).await;

        let (parts, body) = response.into_parts();
        let content_type =
            parts.headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("");

        // Do not buffer SSE responses; preserve streaming semantics.
        let mut response = if content_type.contains("text/event-stream") {
            if !skip_logging {
                info!(
                    id = %trace_id,
                    "response Body: [Skipped: Type=text/event-stream, streaming passthrough]"
                );
            }
            Response::from_parts(parts, body)
        } else {
            let res_bytes = buffer_and_log(
                "response",
                &trace_id.to_string(),
                &parts.headers,
                body,
                skip_logging,
            )
            .await;
            Response::from_parts(parts, Body::from(res_bytes))
        };

        let latency = start_time.elapsed();

        response.headers_mut().insert(X_TRACE_ID, trace_id_value);

        if !skip_logging {
            info!(
                status = response.status().as_u16(),
                latency_ms = latency.as_millis(),
                "← response finished"
            );
        }

        response
    }
    .instrument(span)
    .await
}

async fn buffer_and_log(
    direction: &str,
    trace_id: &str,
    headers: &header::HeaderMap,
    body: Body,
    skip_logging: bool,
) -> Bytes {
    let content_type =
        headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("");
    let is_json = content_type.contains("application/json");

    let bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => return Bytes::new(),
    };

    if !skip_logging {
        if is_json && bytes.len() < 1024 {
            if let Ok(text) = std::str::from_utf8(&bytes) {
                info!(id = %trace_id, "{} Body: {}", direction, text);
            }
        } else if !bytes.is_empty() {
            info!(id = %trace_id, "{} Body: [Skipped: Type={}, Size={}]", direction, content_type, bytes.len());
        }
    }

    bytes
}
