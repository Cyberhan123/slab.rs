pub mod agent;
pub mod audio;
pub mod backend;
pub mod chat;
#[path = "settings/mod.rs"]
pub mod configuration_routes;
pub mod ffmpeg;
pub mod images;
pub mod models;
mod path;
pub mod plugins;
pub mod session;
pub mod setup;
pub mod subtitles;
pub mod system;
pub mod tasks;
pub mod ui_state;
pub mod video;
pub mod workspace_lsp;

use std::sync::Arc;

use axum::Router;
use utoipa::OpenApi;

use slab_app_core::context::AppState;

#[derive(OpenApi)]
#[openapi()]
pub struct V1Api;

/// Routes nested under `/v1`.
pub fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .merge(agent::router())
        .merge(chat::router())
        .merge(models::router())
        .merge(plugins::router())
        .merge(session::router())
        .merge(audio::router())
        .merge(images::router())
        .merge(video::router())
        .merge(ffmpeg::router())
        .merge(system::router())
        .merge(tasks::router())
        .merge(configuration_routes::router(state.clone()))
        .merge(subtitles::router())
        .merge(ui_state::router())
        .merge(setup::router())
        .merge(workspace_lsp::router())
        .merge(backend::router(state))
}

pub fn api_docs() -> utoipa::openapi::OpenApi {
    let mut spec = V1Api::openapi();
    spec.merge(agent::AgentApi::openapi());
    spec.merge(chat::ChatApi::openapi());
    spec.merge(models::ModelsApi::openapi());
    spec.merge(plugins::PluginApi::openapi());
    spec.merge(session::SessionApi::openapi());
    spec.merge(audio::AudioApi::openapi());
    spec.merge(images::ImagesApi::openapi());
    spec.merge(video::VideoApi::openapi());
    spec.merge(ffmpeg::FfmpegApi::openapi());
    spec.merge(system::SystemApi::openapi());
    spec.merge(tasks::TasksApi::openapi());
    spec.merge(configuration_routes::SettingsApi::openapi());
    spec.merge(subtitles::SubtitleApi::openapi());
    spec.merge(ui_state::UiStateApi::openapi());
    spec.merge(setup::SetupApi::openapi());
    spec.merge(backend::BackendApi::openapi());

    spec
}
