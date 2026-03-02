use std::str::FromStr;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{post, put};
use axum::{Json, Router};
use chrono::Utc;
use slab_core::api::Backend;
use utoipa::OpenApi;
use uuid::Uuid;

use crate::entities::{ModelCatalogRecord, ModelStore};
use crate::error::ServerError;
use crate::schemas::admin::models::{
    CreateModelRequest, ModelCatalogResponse, UpdateModelRequest,
};
use crate::state::AppState;

#[derive(OpenApi)]
#[openapi(
    paths(create_model, list_models, update_model, delete_model),
    components(schemas(
        CreateModelRequest,
        UpdateModelRequest,
        ModelCatalogResponse
    ))
)]
pub struct ModelsAdminApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/models", post(create_model).get(list_models))
        .route("/models/{id}", put(update_model).delete(delete_model))
}

fn normalize_backend_ids(raw: &[String]) -> Result<Vec<String>, ServerError> {
    if raw.is_empty() {
        return Err(ServerError::BadRequest(
            "backend_ids must include at least one backend".into(),
        ));
    }

    let mut out = Vec::with_capacity(raw.len());
    for backend_id in raw {
        let trimmed = backend_id.trim();
        if trimmed.is_empty() {
            return Err(ServerError::BadRequest(
                "backend_ids must not contain empty values".into(),
            ));
        }
        let backend = Backend::from_str(trimmed)
            .map_err(|_| ServerError::BadRequest(format!("unknown backend_id: {trimmed}")))?;
        out.push(backend.to_string());
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn validate_catalog_fields(display_name: &str, repo_id: &str, filename: &str) -> Result<(), ServerError> {
    if display_name.trim().is_empty() {
        return Err(ServerError::BadRequest(
            "display_name must not be empty".into(),
        ));
    }
    if repo_id.trim().is_empty() {
        return Err(ServerError::BadRequest("repo_id must not be empty".into()));
    }
    if filename.trim().is_empty() {
        return Err(ServerError::BadRequest("filename must not be empty".into()));
    }
    Ok(())
}

fn to_response(record: ModelCatalogRecord) -> ModelCatalogResponse {
    ModelCatalogResponse {
        id: record.id,
        display_name: record.display_name,
        repo_id: record.repo_id,
        filename: record.filename,
        backend_ids: record.backend_ids,
        local_path: record.local_path,
        last_download_task_id: record.last_download_task_id,
        last_downloaded_at: record.last_downloaded_at.map(|v| v.to_rfc3339()),
        created_at: record.created_at.to_rfc3339(),
        updated_at: record.updated_at.to_rfc3339(),
    }
}

#[utoipa::path(
    post,
    path = "/admin/models",
    tag = "admin",
    request_body = CreateModelRequest,
    responses(
        (status = 200, description = "Model catalog entry created", body = ModelCatalogResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorised (management token required)")
    )
)]
pub async fn create_model(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateModelRequest>,
) -> Result<Json<ModelCatalogResponse>, ServerError> {
    validate_catalog_fields(&req.display_name, &req.repo_id, &req.filename)?;
    let backend_ids = normalize_backend_ids(&req.backend_ids)?;

    let now = Utc::now();
    let record = ModelCatalogRecord {
        id: Uuid::new_v4().to_string(),
        display_name: req.display_name.trim().to_owned(),
        repo_id: req.repo_id.trim().to_owned(),
        filename: req.filename.trim().to_owned(),
        backend_ids,
        local_path: None,
        last_download_task_id: None,
        last_downloaded_at: None,
        created_at: now,
        updated_at: now,
    };
    state.store.insert_model(record.clone()).await?;

    Ok(Json(to_response(record)))
}

#[utoipa::path(
    get,
    path = "/admin/models",
    tag = "admin",
    responses(
        (status = 200, description = "List model catalog entries", body = [ModelCatalogResponse]),
        (status = 401, description = "Unauthorised (management token required)")
    )
)]
pub async fn list_models(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ModelCatalogResponse>>, ServerError> {
    let models = state.store.list_models().await?;
    Ok(Json(models.into_iter().map(to_response).collect()))
}

#[utoipa::path(
    put,
    path = "/admin/models/{id}",
    tag = "admin",
    request_body = UpdateModelRequest,
    responses(
        (status = 200, description = "Model catalog entry updated", body = ModelCatalogResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorised (management token required)"),
        (status = 404, description = "Model not found")
    )
)]
pub async fn update_model(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateModelRequest>,
) -> Result<Json<ModelCatalogResponse>, ServerError> {
    let existing = state
        .store
        .get_model(&id)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("model {id} not found")))?;

    let display_name = req
        .display_name
        .unwrap_or(existing.display_name)
        .trim()
        .to_owned();
    let repo_id = req.repo_id.unwrap_or(existing.repo_id).trim().to_owned();
    let filename = req.filename.unwrap_or(existing.filename).trim().to_owned();
    let backend_ids = if let Some(ids) = req.backend_ids {
        normalize_backend_ids(&ids)?
    } else {
        existing.backend_ids
    };

    validate_catalog_fields(&display_name, &repo_id, &filename)?;

    state
        .store
        .update_model_metadata(&id, &display_name, &repo_id, &filename, &backend_ids)
        .await?;

    let updated = state
        .store
        .get_model(&id)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("model {id} not found after update")))?;

    Ok(Json(to_response(updated)))
}

#[utoipa::path(
    delete,
    path = "/admin/models/{id}",
    tag = "admin",
    responses(
        (status = 200, description = "Model catalog entry deleted", body = serde_json::Value),
        (status = 401, description = "Unauthorised (management token required)"),
        (status = 404, description = "Model not found")
    )
)]
pub async fn delete_model(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let exists = state.store.get_model(&id).await?;
    if exists.is_none() {
        return Err(ServerError::NotFound(format!("model {id} not found")));
    }

    state.store.delete_model(&id).await?;
    Ok(Json(serde_json::json!({ "id": id, "status": "deleted" })))
}
