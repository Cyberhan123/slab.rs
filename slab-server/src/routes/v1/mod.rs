pub mod audio;
pub mod chat;
pub mod ffmpeg;
pub mod images;
pub mod models;
pub mod session;
pub mod tasks;

use crate::state::AppState;
use utoipa::OpenApi;

use axum::Router;
use std::sync::Arc;

/// Routes nested under `/v1` (OpenAI-compatible).
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .merge(chat::router())
        .merge(audio::router())
        .merge(images::router())
        .merge(ffmpeg::router())
        .merge(session::router())
        .merge(models::router())
        .merge(tasks::router())
}

#[derive(OpenApi)]
#[openapi()]
pub struct V1Api;

pub fn api_docs() -> utoipa::openapi::OpenApi {
    let mut spec = V1Api::openapi();
    spec.merge(audio::AudioApi::openapi());
    spec.merge(chat::ChatApi::openapi());
    spec.merge(ffmpeg::FfmpegApi::openapi());
    spec.merge(images::ImagesApi::openapi());
    spec.merge(models::ModelsApi::openapi());
    spec.merge(session::SessionApi::openapi());
    spec.merge(tasks::TasksApi::openapi());

    spec
}
