pub type FfmpegApi = crate::services::ffmpeg::FfmpegApi;

pub fn router() -> axum::Router<std::sync::Arc<crate::context::AppState>> {
    crate::services::ffmpeg::router()
}
