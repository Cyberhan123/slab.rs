pub type ChatApi = crate::services::chat::ChatApi;

pub fn router() -> axum::Router<std::sync::Arc<crate::context::AppState>> {
    crate::services::chat::router()
}
