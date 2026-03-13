pub type SessionApi = crate::services::session::SessionApi;

pub fn router() -> axum::Router<std::sync::Arc<crate::context::AppState>> {
    crate::services::session::router()
}
