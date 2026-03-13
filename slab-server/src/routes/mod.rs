//! Axum router construction.
//!
//! [`build`] assembles the complete application router, including:
//! - Middleware layers (CORS, per-request trace-ID injection)
//! - Optional Swagger UI / OpenAPI spec endpoint (disable with `SLAB_ENABLE_SWAGGER=false`)
//! - Health / heartbeat route
//! - OpenAI-compatible `/v1` routes
//! - admin `/admin` routes (optionally protected by bearer token)

mod admin;
pub mod doc;
pub mod health;
pub(crate) mod v1;
use crate::middleware::{cors, trace};
use crate::state::{AppState, ChatContext, ModelContext, TaskContext};
use axum::{
    middleware::{self},
    Router,
};
use std::sync::Arc;
use tower::ServiceBuilder;
use utoipa_swagger_ui::SwaggerUi;
// ── Router builder ────────────────────────────────────────────────────────────

/// Build the HTTP gateway router used by supervisor mode.
pub fn build(
    state: Arc<AppState>,
    chat_context: Arc<ChatContext>,
    model_context: Arc<ModelContext>,
    task_context: Arc<TaskContext>,
) -> Router {
    let api_router = Router::new()
        .merge(health::router())
        .nest(
            "/v1",
            v1::router(
                Arc::clone(&state),
                Arc::clone(&chat_context),
                Arc::clone(&model_context),
                Arc::clone(&task_context),
            ),
        )
        .nest("/admin", admin::router(state.clone()));

    let mut app = Router::new().merge(api_router);
    let api_doc = doc::get_docs();

    if state.config.enable_swagger {
        app = app.merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api_doc));
    }

    app.layer(ServiceBuilder::new().layer(cors::cors_layer(state.clone())))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            trace::trace_middleware,
        ))
        .with_state(state)
}
