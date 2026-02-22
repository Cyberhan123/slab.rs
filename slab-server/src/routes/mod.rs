//! Axum router construction.
//!
//! [`build`] assembles the complete application router, including:
//! - Middleware layers (CORS, per-request trace-ID injection)
//! - Swagger UI / OpenAPI spec endpoint
//! - Health / heartbeat route
//! - OpenAI-compatible `/v1` routes
//! - Model-management `/api` routes

mod audio;
mod chat;
mod health;
mod images;
mod management;

use std::sync::Arc;

use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::middleware::TraceLayer;
use crate::state::AppState;

// ── OpenAPI spec ─────────────────────────────────────────────────────────────

#[derive(OpenApi)]
#[openapi(
    info(
        title = "slab-server",
        description = "slab-server AI inference API – OpenAI-compatible",
        version = "0.0.1",
        contact(name = "slab-rs", url = "https://github.com/Cyberhan123/slab.rs")
    ),
    paths(
        health::get_health,
        chat::chat_completions,
        audio::transcribe,
        images::generate_images,
        management::load_model,
        management::model_status,
    ),
    components(schemas(
        crate::models::openai::ChatMessage,
        crate::models::openai::ChatCompletionRequest,
        crate::models::openai::ChatChoice,
        crate::models::openai::ChatCompletionResponse,
        crate::models::openai::TranscriptionResponse,
        crate::models::openai::ImageGenerationRequest,
        crate::models::openai::ImageData,
        crate::models::openai::ImageGenerationResponse,
        crate::models::openai::ModelInfo,
        crate::models::openai::ModelListResponse,
        crate::models::management::LoadModelRequest,
        crate::models::management::ModelStatusResponse,
    )),
    tags(
        (name = "health",     description = "Health & heartbeat"),
        (name = "chat",       description = "OpenAI-compatible chat completions"),
        (name = "audio",      description = "Speech-to-text transcription"),
        (name = "images",     description = "Image generation"),
        (name = "management", description = "Model management"),
    )
)]
pub struct ApiDoc;

// ── Router builder ────────────────────────────────────────────────────────────

/// Build the complete Axum [`Router`] for the application.
pub fn build(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_headers(Any)
        .allow_methods(Any);

    let api_router = Router::new()
        .merge(health::router())
        .nest("/v1",  v1_router())
        .nest("/api", management::router());

    Router::new()
        // Swagger UI served at /swagger-ui; spec at /api-docs/openapi.json
        .merge(
            SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", ApiDoc::openapi()),
        )
        .merge(api_router)
        // Outermost layers execute first on the way in.
        .layer(TraceLayer::new(Arc::clone(&state)))
        .layer(cors)
        .with_state(state)
}

/// Routes nested under `/v1` (OpenAI-compatible).
fn v1_router() -> Router<Arc<AppState>> {
    Router::new()
        .merge(chat::router())
        .merge(audio::router())
        .merge(images::router())
}
