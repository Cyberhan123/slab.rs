pub type ImagesApi = crate::services::images::ImagesApi;

pub fn router() -> axum::Router<std::sync::Arc<crate::context::AppState>> {
    crate::services::images::router()
}
