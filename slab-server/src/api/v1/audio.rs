pub type AudioApi = crate::services::audio::AudioApi;

pub fn router() -> axum::Router<std::sync::Arc<crate::context::AppState>> {
    crate::services::audio::router()
}
