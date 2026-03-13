pub type SystemApi = crate::services::system::SystemApi;

pub fn router() -> axum::Router<std::sync::Arc<crate::context::AppState>> {
    crate::services::system::router()
}
