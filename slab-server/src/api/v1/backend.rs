use std::sync::Arc;

use axum::{middleware, Router};

use crate::api::middleware::auth;
use crate::context::AppState;

pub type BackendApi = crate::services::backend::BackendApi;

pub fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    crate::services::backend::router()
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ))
        .with_state(state)
}
