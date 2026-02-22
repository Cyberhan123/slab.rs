//! Request-tracing middleware.
//!
//! Assigns a `X-Trace-Id` UUID (v4) to every incoming request, injects it
//! into the [`tracing`] span so all log lines emitted during the request carry
//! the same `trace_id` field, and echoes it back in the response
//! `X-Trace-Id` header.
//!
//! Additionally, each request is inserted into the database on arrival and
//! updated with the final status code and latency once the response is ready.

use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Instant;

use axum::body::Body;
use axum::extract::Request;
use axum::http::header::HeaderName;
use axum::http::{HeaderValue, Response};
use futures::future::BoxFuture;
use tower::{Layer, Service};
use tracing::{info, info_span, Instrument};
use uuid::Uuid;

use crate::db::{RequestRecord, RequestStore};
use crate::state::AppState;

/// HTTP header carrying the per-request trace ID.
pub static X_TRACE_ID: HeaderName = HeaderName::from_static("x-trace-id");

// ── Layer ────────────────────────────────────────────────────────────────────

/// [`tower::Layer`] that wraps each handler with trace-ID injection and
/// request audit logging.
#[derive(Clone)]
pub struct TraceLayer {
    state: Arc<AppState>,
}

impl TraceLayer {
    /// Create a new [`TraceLayer`] backed by the given shared state.
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

impl<S> Layer<S> for TraceLayer {
    type Service = TraceMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TraceMiddleware {
            inner,
            state: Arc::clone(&self.state),
        }
    }
}

// ── Service ──────────────────────────────────────────────────────────────────

/// The middleware service produced by [`TraceLayer`].
#[derive(Clone)]
pub struct TraceMiddleware<S> {
    inner: S,
    state: Arc<AppState>,
}

impl<S> Service<Request<Body>> for TraceMiddleware<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        // ── Assign or inherit a trace ID ───────────────────────────────────────
        let trace_id: Uuid = req
            .headers()
            .get(&X_TRACE_ID)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Uuid::new_v4);

        // Inject the trace ID into request headers for downstream handlers.
        // SAFETY: UUID v4 is hex + hyphens – always a valid HeaderValue.
        req.headers_mut().insert(
            X_TRACE_ID.clone(),
            HeaderValue::from_str(&trace_id.to_string()).unwrap(),
        );

        let method  = req.method().to_string();
        let path    = req.uri().path().to_owned();
        let state   = Arc::clone(&self.state);
        let started = Instant::now();

        let span = info_span!(
            "http_request",
            trace_id = %trace_id,
            method   = %method,
            path     = %path,
        );

        // Log the incoming request (fire-and-forget; never blocks the handler).
        let store_req = Arc::clone(&state.store);
        let record = RequestRecord {
            id:         trace_id,
            method:     method.clone(),
            path:       path.clone(),
            status:     None,
            latency_ms: None,
            created_at: chrono::Utc::now(),
        };
        tokio::spawn(async move {
            if let Err(e) = store_req.insert(record).await {
                tracing::warn!(trace_id = %trace_id, error = %e, "failed to log request");
            }
        });

        let mut inner = self.inner.clone();
        Box::pin(
            async move {
                info!(%method, %path, "→ request");

                let mut response = inner.call(req).await?;

                let status     = response.status().as_u16();
                let latency_ms = started.elapsed().as_millis() as i64;

                info!(status, latency_ms, "← response");

                // Echo the trace ID back in the response headers.
                // SAFETY: UUID v4 is hex + hyphens – always a valid HeaderValue.
                response.headers_mut().insert(
                    X_TRACE_ID.clone(),
                    HeaderValue::from_str(&trace_id.to_string()).unwrap(),
                );

                // Update the database record with the final status and latency.
                let store_resp = Arc::clone(&state.store);
                tokio::spawn(async move {
                    if let Err(e) = store_resp.update_response(trace_id, status, latency_ms).await {
                        tracing::warn!(
                            trace_id = %trace_id,
                            error    = %e,
                            "failed to update request log"
                        );
                    }
                });

                Ok(response)
            }
            .instrument(span),
        )
    }
}
