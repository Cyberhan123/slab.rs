use axum::Router;
use std::sync::Arc;
use utoipa::OpenApi;

use crate::context::AppState;

pub mod audio;
pub mod ffmpeg;
pub mod images;
pub mod system;
pub mod tasks;
pub mod video;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .merge(audio::router())
        .merge(images::router())
        .merge(video::router())
        .merge(ffmpeg::router())
        .merge(system::router())
        .merge(tasks::router())
}

pub fn merge_api_docs(spec: &mut utoipa::openapi::OpenApi) {
    spec.merge(audio::AudioApi::openapi());
    spec.merge(ffmpeg::FfmpegApi::openapi());
    spec.merge(images::ImagesApi::openapi());
    spec.merge(video::VideoApi::openapi());
    spec.merge(system::SystemApi::openapi());
    spec.merge(tasks::TasksApi::openapi());
}
