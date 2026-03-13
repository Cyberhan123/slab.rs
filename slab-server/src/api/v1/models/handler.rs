use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use utoipa::OpenApi;
use validator::Validate;

use crate::api::v1::models::schema::{
    CreateModelRequest, DownloadModelRequest, ListAvailableQuery, ListModelsQuery,
    LoadModelRequest, ModelCatalogItemResponse, ModelListStatus, ModelStatusResponse,
    SwitchModelRequest, UpdateModelRequest,
};
use crate::api::v1::tasks::schema::OperationAcceptedResponse;
use crate::api::validation::{validate, ValidatedJson, ValidatedQuery};
use crate::context::AppState;
use crate::domain::models::{
    AvailableModelsQuery, CreateModelCommand, DeletedModelView, DownloadModelCommand,
    ListModelsFilter, ModelCatalogItemView, ModelCatalogStatus, ModelLoadCommand,
    UpdateModelCommand,
};
use crate::domain::services::to_operation_accepted_response;
use crate::domain::services::ModelService;
use crate::error::ServerError;

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
    State(service): State<ModelService>,
    ValidatedJson(req): ValidatedJson<CreateModelRequest>,
) -> Result<Json<ModelCatalogItemResponse>, ServerError> {
    Ok(Json(to_model_catalog_item_response(
        service
            .create_model(CreateModelCommand {
                display_name: req.display_name,
                repo_id: req.repo_id,
                filename: req.filename,
                backend_ids: req.backend_ids,
            })
            .await?,
    )))
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
    State(service): State<ModelService>,
    Path(params): Path<ModelIdPath>,
    ValidatedJson(req): ValidatedJson<UpdateModelRequest>,
) -> Result<Json<ModelCatalogItemResponse>, ServerError> {
    let params = validate(params)?;
    Ok(Json(to_model_catalog_item_response(
        service
            .update_model(
                &params.id,
                UpdateModelCommand {
                    display_name: req.display_name,
                    repo_id: req.repo_id,
                    filename: req.filename,
                    backend_ids: req.backend_ids,
                },
            )
            .await?,
    )))
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
    State(service): State<ModelService>,
    Path(params): Path<ModelIdPath>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let params = validate(params)?;
    Ok(Json(to_deleted_model_response(
        service.delete_model(&params.id).await?,
    )))
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
    State(service): State<ModelService>,
    Query(query): Query<ListModelsQuery>,
) -> Result<Json<Vec<ModelCatalogItemResponse>>, ServerError> {
    let items = service
        .list_models(ListModelsFilter {
            status: to_model_catalog_status(query.status),
        })
        .await?
        .into_iter()
        .map(to_model_catalog_item_response)
        .collect();
    Ok(Json(items))
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
    State(service): State<ModelService>,
    ValidatedJson(req): ValidatedJson<LoadModelRequest>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    Ok(Json(to_model_status_response(
        service.load_model(to_model_load_command(req)).await?,
    )))
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
    State(service): State<ModelService>,
    ValidatedJson(req): ValidatedJson<LoadModelRequest>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    Ok(Json(to_model_status_response(
        service.unload_model(to_model_load_command(req)).await?,
    )))
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
    State(service): State<ModelService>,
    ValidatedQuery(query): ValidatedQuery<ListAvailableQuery>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let response = service
        .list_available_models(AvailableModelsQuery {
            repo_id: query.repo_id,
        })
        .await?;
    Ok(Json(serde_json::json!({
        "repo_id": response.repo_id,
        "files": response.files,
    })))
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
    State(service): State<ModelService>,
    ValidatedJson(req): ValidatedJson<SwitchModelRequest>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    Ok(Json(to_model_status_response(
        service
            .switch_model(ModelLoadCommand {
                backend_id: req.backend_id,
                model_path: req.model_path,
                num_workers: req.num_workers,
            })
            .await?,
    )))
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
    State(service): State<ModelService>,
    ValidatedJson(req): ValidatedJson<DownloadModelRequest>,
) -> Result<(StatusCode, Json<OperationAcceptedResponse>), ServerError> {
    let response = service
        .download_model(DownloadModelCommand {
            model_id: req.model_id,
            backend_id: req.backend_id,
        })
        .await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(to_operation_accepted_response(response)),
    ))
}

#[derive(Debug, Deserialize, Validate)]
struct ModelIdPath {
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "id must not be empty"
    ))]
    id: String,
}

fn to_model_load_command(request: LoadModelRequest) -> ModelLoadCommand {
    ModelLoadCommand {
        backend_id: request.backend_id,
        model_path: request.model_path,
        num_workers: request.num_workers,
    }
}

fn to_model_status_response(status: crate::domain::models::ModelStatus) -> ModelStatusResponse {
    ModelStatusResponse {
        backend: status.backend,
        status: status.status,
    }
}

fn to_model_catalog_item_response(item: ModelCatalogItemView) -> ModelCatalogItemResponse {
    ModelCatalogItemResponse {
        id: item.id,
        display_name: item.display_name,
        repo_id: item.repo_id,
        filename: item.filename,
        backend_ids: item.backend_ids,
        is_vad_model: item.is_vad_model,
        status: match item.status {
            ModelCatalogStatus::Downloaded => ModelListStatus::Downloaded,
            ModelCatalogStatus::Pending => ModelListStatus::Pending,
            ModelCatalogStatus::NotDownloaded => ModelListStatus::NotDownloaded,
            ModelCatalogStatus::All => ModelListStatus::All,
        },
        local_path: item.local_path,
        last_downloaded_at: item.last_downloaded_at,
        pending_task_id: item.pending_task_id,
        pending_task_status: item.pending_task_status,
    }
}

fn to_model_catalog_status(status: ModelListStatus) -> ModelCatalogStatus {
    match status {
        ModelListStatus::Downloaded => ModelCatalogStatus::Downloaded,
        ModelListStatus::Pending => ModelCatalogStatus::Pending,
        ModelListStatus::NotDownloaded => ModelCatalogStatus::NotDownloaded,
        ModelListStatus::All => ModelCatalogStatus::All,
    }
}

fn to_deleted_model_response(view: DeletedModelView) -> serde_json::Value {
    serde_json::json!({
        "id": view.id,
        "status": view.status,
    })
}
