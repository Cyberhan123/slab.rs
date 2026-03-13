use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::v1::models::schema::{
    CreateModelRequest, DownloadModelRequest, ListAvailableQuery, ListModelsQuery,
    LoadModelRequest, ModelCatalogItemResponse, ModelStatusResponse, SwitchModelRequest,
    UpdateModelRequest,
};
use crate::api::v1::tasks::schema::OperationAcceptedResponse;
use crate::context::{AppState, ModelState, WorkerState};
use crate::error::ServerError;
use crate::services::models::ModelsService;

#[derive(OpenApi)]
#[openapi(
    paths(
        list_models,
        create_model,
        update_model,
        delete_model,
        load_model,
        unload_model,
        list_available_models,
        switch_model,
        download_model
    ),
    components(schemas(
        CreateModelRequest,
        UpdateModelRequest,
        LoadModelRequest,
        ModelStatusResponse,
        SwitchModelRequest,
        DownloadModelRequest,
        ListAvailableQuery,
        ListModelsQuery,
        ModelCatalogItemResponse,
        OperationAcceptedResponse
    ))
)]
pub struct ModelsApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/models", get(list_models).post(create_model))
        .route("/models/{id}", put(update_model).delete(delete_model))
        .route("/models/available", get(list_available_models))
        .route("/models/load", post(load_model))
        .route("/models/unload", post(unload_model))
        .route("/models/switch", post(switch_model))
        .route("/models/download", post(download_model))
}

#[utoipa::path(
    post,
    path = "/v1/models",
    tag = "models",
    request_body = CreateModelRequest,
    responses(
        (status = 200, description = "Model catalog entry created", body = ModelCatalogItemResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn create_model(
    State(model_state): State<ModelState>,
    State(worker_state): State<WorkerState>,
    Json(req): Json<CreateModelRequest>,
) -> Result<Json<ModelCatalogItemResponse>, ServerError> {
    let service = ModelsService::new(model_state, worker_state);
    Ok(Json(service.create_model(req).await?))
}

#[utoipa::path(
    put,
    path = "/v1/models/{id}",
    tag = "models",
    request_body = UpdateModelRequest,
    params(
        ("id" = String, Path, description = "Model catalog entry ID")
    ),
    responses(
        (status = 200, description = "Model catalog entry updated", body = ModelCatalogItemResponse),
        (status = 400, description = "Bad request"),
        (status = 404, description = "Model not found"),
        (status = 500, description = "Backend error"),
    )
)]
async fn update_model(
    State(model_state): State<ModelState>,
    State(worker_state): State<WorkerState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateModelRequest>,
) -> Result<Json<ModelCatalogItemResponse>, ServerError> {
    let service = ModelsService::new(model_state, worker_state);
    Ok(Json(service.update_model(&id, req).await?))
}

#[utoipa::path(
    delete,
    path = "/v1/models/{id}",
    tag = "models",
    params(
        ("id" = String, Path, description = "Model catalog entry ID")
    ),
    responses(
        (status = 200, description = "Model catalog entry deleted", body = serde_json::Value),
        (status = 404, description = "Model not found"),
        (status = 500, description = "Backend error"),
    )
)]
async fn delete_model(
    State(model_state): State<ModelState>,
    State(worker_state): State<WorkerState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let service = ModelsService::new(model_state, worker_state);
    Ok(Json(service.delete_model(&id).await?))
}

#[utoipa::path(
    get,
    path = "/v1/models",
    tag = "models",
    params(ListModelsQuery),
    responses(
        (status = 200, description = "List model catalog entries by download status", body = [ModelCatalogItemResponse]),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn list_models(
    State(model_state): State<ModelState>,
    State(worker_state): State<WorkerState>,
    Query(query): Query<ListModelsQuery>,
) -> Result<Json<Vec<ModelCatalogItemResponse>>, ServerError> {
    let service = ModelsService::new(model_state, worker_state);
    Ok(Json(service.list_models(query).await?))
}

#[utoipa::path(
    post,
    path = "/v1/models/load",
    tag = "models",
    request_body = LoadModelRequest,
    responses(
        (status = 200, description = "Model load initiated", body = ModelStatusResponse),
        (status = 400, description = "Unknown backend or invalid paths"),
        (status = 500, description = "Backend error"),
    )
)]
async fn load_model(
    State(model_state): State<ModelState>,
    State(worker_state): State<WorkerState>,
    Json(req): Json<LoadModelRequest>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    let service = ModelsService::new(model_state, worker_state);
    Ok(Json(service.load_model(req).await?))
}

#[utoipa::path(
    post,
    path = "/v1/models/unload",
    tag = "models",
    request_body = LoadModelRequest,
    responses(
        (status = 200, description = "Model unloaded", body = ModelStatusResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn unload_model(
    State(model_state): State<ModelState>,
    State(worker_state): State<WorkerState>,
    Json(req): Json<LoadModelRequest>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    let service = ModelsService::new(model_state, worker_state);
    Ok(Json(service.unload_model(req).await?))
}

#[utoipa::path(
    get,
    path = "/v1/models/available",
    tag = "models",
    params(ListAvailableQuery),
    responses(
        (status = 200, description = "List of available files", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn list_available_models(
    State(model_state): State<ModelState>,
    State(worker_state): State<WorkerState>,
    Query(query): Query<ListAvailableQuery>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let service = ModelsService::new(model_state, worker_state);
    Ok(Json(service.list_available_models(query).await?))
}

#[utoipa::path(
    post,
    path = "/v1/models/switch",
    tag = "models",
    request_body = SwitchModelRequest,
    responses(
        (status = 200, description = "Model switched successfully", body = ModelStatusResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn switch_model(
    State(model_state): State<ModelState>,
    State(worker_state): State<WorkerState>,
    Json(req): Json<SwitchModelRequest>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    let service = ModelsService::new(model_state, worker_state);
    Ok(Json(service.switch_model(req).await?))
}

#[utoipa::path(
    post,
    path = "/v1/models/download",
    tag = "models",
    request_body = DownloadModelRequest,
    responses(
        (status = 202, description = "Download task accepted", body = OperationAcceptedResponse),
        (status = 400, description = "Bad request"),
        (status = 404, description = "Model catalog entry not found"),
        (status = 500, description = "Backend error"),
    )
)]
async fn download_model(
    State(model_state): State<ModelState>,
    State(worker_state): State<WorkerState>,
    Json(req): Json<DownloadModelRequest>,
) -> Result<(StatusCode, Json<OperationAcceptedResponse>), ServerError> {
    let service = ModelsService::new(model_state, worker_state);
    let response = service.download_model(req).await?;
    Ok((StatusCode::ACCEPTED, Json(response)))
}
