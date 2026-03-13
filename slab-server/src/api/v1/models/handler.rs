pub type ModelsApi = crate::services::models::ModelsApi;

pub fn router() -> axum::Router<std::sync::Arc<crate::context::AppState>> {
    crate::services::models::router()
}
