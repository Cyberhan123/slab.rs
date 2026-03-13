pub type VideoApi = crate::services::video::VideoApi;

pub fn router() -> axum::Router<std::sync::Arc<crate::context::AppState>> {
    crate::services::video::router()
}
