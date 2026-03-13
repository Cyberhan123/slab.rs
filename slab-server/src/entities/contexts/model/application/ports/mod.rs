use crate::entities::contexts::model::domain::ModelCatalogRecord;
use chrono::{DateTime, Utc};
use std::future::Future;

pub trait ModelRepository: Send + Sync + 'static {
    fn insert_model(
        &self,
        record: ModelCatalogRecord,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn get_model(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<Option<ModelCatalogRecord>, sqlx::Error>> + Send;
    fn list_models(
        &self,
    ) -> impl Future<Output = Result<Vec<ModelCatalogRecord>, sqlx::Error>> + Send;
    fn update_model_metadata(
        &self,
        id: &str,
        display_name: &str,
        repo_id: &str,
        filename: &str,
        backend_ids: &[String],
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn delete_model(&self, id: &str) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn mark_model_downloaded(
        &self,
        id: &str,
        local_path: &str,
        task_id: &str,
        downloaded_at: DateTime<Utc>,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
}
