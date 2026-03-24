pub mod agent;
pub mod audio;
pub mod backend;
pub mod chat;
pub mod ffmpeg;
pub mod images;
pub mod models;
pub mod session;
pub mod settings;
pub mod setup;
pub mod system;
pub mod tasks;
pub mod video;

use std::sync::Arc;

use axum::Router;
use utoipa::OpenApi;

use crate::context::AppState;

#[derive(OpenApi)]
#[openapi()]
pub struct V1Api;

/// Routes nested under `/v1`.
pub fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .merge(agent::router())
        .merge(chat::router())
        .merge(models::router())
        .merge(session::router())
        .merge(audio::router())
        .merge(images::router())
        .merge(video::router())
        .merge(ffmpeg::router())
        .merge(system::router())
        .merge(tasks::router())
        .merge(settings::router(state.clone()))
        .merge(setup::router())
        .merge(backend::router(state))
}

pub fn api_docs() -> utoipa::openapi::OpenApi {
    let mut spec = V1Api::openapi();
    spec.merge(agent::AgentApi::openapi());
    spec.merge(chat::ChatApi::openapi());
    spec.merge(models::ModelsApi::openapi());
    spec.merge(session::SessionApi::openapi());
    spec.merge(audio::AudioApi::openapi());
    spec.merge(images::ImagesApi::openapi());
    spec.merge(video::VideoApi::openapi());
    spec.merge(ffmpeg::FfmpegApi::openapi());
    spec.merge(system::SystemApi::openapi());
    spec.merge(tasks::TasksApi::openapi());
    spec.merge(settings::SettingsApi::openapi());
    spec.merge(setup::SetupApi::openapi());
    spec.merge(backend::BackendApi::openapi());

    spec
}
