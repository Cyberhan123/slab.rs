use crate::state::AppState;
use axum::{
    body::{Body, Bytes},
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use http_body_util::BodyExt; // cargo add http_body_util
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, info_span, Instrument};
use uuid::Uuid;

pub static X_TRACE_ID: &str = "x-trace-id";

pub async fn trace_middleware(
    State(_state): State<Arc<AppState>>, // 自动获取 State
    req: Request<Body>,
    next: Next,
) -> Response {
    let start_time = Instant::now();

    // 1. 提取或生成 Trace ID
    let trace_id = req
        .headers()
        .get(X_TRACE_ID)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or_else(Uuid::new_v4);

    let method = req.method().clone();
    let path = req.uri().path().to_string();

    // 2. 创建 Tracing Span
    let span = info_span!(
        "http_request",
        trace_id = %trace_id,
        method = %method,
        path = %path,
    );

    // 在 Span 范围内执行逻辑
    async move {
        info!("→ request started");
        let (parts, body) = req.into_parts();
        
        let req_bytes = buffer_and_log("request", &trace_id.to_string(), &parts.headers, body).await;
        let mut req = Request::from_parts(parts, Body::from(req_bytes));

        req.headers_mut()
            .insert(X_TRACE_ID, trace_id.to_string().parse().unwrap());

        let response = next.run(req).await;

        let (parts, body) = response.into_parts();

        let res_bytes = buffer_and_log("response", &trace_id.to_string(), &parts.headers, body).await;

        let mut response = Response::from_parts(parts, Body::from(res_bytes));

        let latency = start_time.elapsed();

        response
            .headers_mut()
            .insert(X_TRACE_ID, trace_id.to_string().parse().unwrap());

        info!(
            status = response.status().as_u16(),
            latency_ms = latency.as_millis(),
            "← response finished"
        );

        response
    }
    .instrument(span)
    .await
}

/// 辅助函数：根据类型和大小决定是否打印 Body
async fn buffer_and_log(
    direction: &str,
    trace_id: &str,
    headers: &header::HeaderMap,
    body: Body,
) -> Bytes {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let is_json = content_type.contains("application/json");

    let bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => return Bytes::new(),
    };

    if is_json && bytes.len() < 1024 {
        if let Ok(text) = std::str::from_utf8(&bytes) {
            info!(id = %trace_id, "{} Body: {}", direction, text);
        }
    } else if !bytes.is_empty() {
        info!(id = %trace_id, "{} Body: [Skipped: Type={}, Size={}]", direction, content_type, bytes.len());
    }

    bytes
}
