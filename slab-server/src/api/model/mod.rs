use axum::Router;
use std::sync::Arc;
use utoipa::OpenApi;

use crate::context::AppState;

pub mod chat;
pub mod models;
pub mod session;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .merge(chat::router())
        .merge(models::router())
        .merge(session::router())
}

pub fn merge_api_docs(spec: &mut utoipa::openapi::OpenApi) {
    spec.merge(chat::ChatApi::openapi());
    spec.merge(models::ModelsApi::openapi());
    spec.merge(session::SessionApi::openapi());
}
