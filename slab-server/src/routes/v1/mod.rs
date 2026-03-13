pub mod audio;
pub mod chat;
pub mod ffmpeg;
pub mod images;
pub mod models;
pub mod session;
pub mod system;
pub mod tasks;
pub mod video;

use crate::state::{AppState, ChatContext, ModelContext, TaskContext};
use utoipa::OpenApi;

use axum::extract::FromRef;
use axum::Router;
use std::sync::Arc;

#[derive(OpenApi)]
#[openapi()]
pub struct V1Api;

/// Routes nested under `/v1`.
#[derive(Clone, FromRef)]
pub struct V1State {
    pub app_state: Arc<AppState>,
    pub chat_context: Arc<ChatContext>,
    pub model_context: Arc<ModelContext>,
    pub task_context: Arc<TaskContext>,
}

impl FromRef<Arc<V1State>> for Arc<AppState> {
    fn from_ref(input: &Arc<V1State>) -> Self {
        Arc::clone(&input.app_state)
    }
}

impl FromRef<Arc<V1State>> for Arc<ChatContext> {
    fn from_ref(input: &Arc<V1State>) -> Self {
        Arc::clone(&input.chat_context)
    }
}

impl FromRef<Arc<V1State>> for Arc<ModelContext> {
    fn from_ref(input: &Arc<V1State>) -> Self {
        Arc::clone(&input.model_context)
    }
}

impl FromRef<Arc<V1State>> for Arc<TaskContext> {
    fn from_ref(input: &Arc<V1State>) -> Self {
        Arc::clone(&input.task_context)
    }
}

pub fn router(
    app_state: Arc<AppState>,
    chat_context: Arc<ChatContext>,
    model_context: Arc<ModelContext>,
    task_context: Arc<TaskContext>,
) -> Router {
    let state = Arc::new(V1State {
        app_state,
        chat_context,
        model_context,
        task_context,
    });

    Router::new()
        .merge(chat::router())
        .merge(audio::router())
        .merge(images::router())
        .merge(video::router())
        .merge(ffmpeg::router())
        .merge(session::router())
        .merge(models::router())
        .merge(system::router())
        .merge(tasks::router())
        .with_state(state)
}

pub fn api_docs() -> utoipa::openapi::OpenApi {
    let mut spec = V1Api::openapi();
    spec.merge(audio::AudioApi::openapi());
    spec.merge(chat::ChatApi::openapi());
    spec.merge(ffmpeg::FfmpegApi::openapi());
    spec.merge(images::ImagesApi::openapi());
    spec.merge(video::VideoApi::openapi());
    spec.merge(models::ModelsApi::openapi());
    spec.merge(session::SessionApi::openapi());
    spec.merge(system::SystemApi::openapi());
    spec.merge(tasks::TasksApi::openapi());

    spec
}
