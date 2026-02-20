use axum::Router;
mod subtitle;

pub fn app() -> Router {
    Router::new()
        .nest("/subtitle", subtitle::router())
}