use std::sync::Arc;

use axum::{middleware, Router};

use crate::api::middleware::auth;
use crate::context::AppState;

pub type ConfigApi = crate::services::config::ConfigApi;

pub fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    crate::services::config::router()
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ))
        .with_state(state)
}
