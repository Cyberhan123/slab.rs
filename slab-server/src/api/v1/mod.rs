use crate::context::AppState;
use crate::api::{model, worker};
use utoipa::OpenApi;

use axum::Router;
use std::sync::Arc;

#[derive(OpenApi)]
#[openapi()]
pub struct V1Api;

/// Routes nested under `/v1`.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .merge(model::router())
        .merge(worker::router())
}

pub fn api_docs() -> utoipa::openapi::OpenApi {
    let mut spec = V1Api::openapi();
    model::merge_api_docs(&mut spec);
    worker::merge_api_docs(&mut spec);

    spec
}

