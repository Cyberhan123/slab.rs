pub type TasksApi = crate::services::tasks::TasksApi;

pub fn router() -> axum::Router<std::sync::Arc<crate::context::AppState>> {
    crate::services::tasks::router()
}
