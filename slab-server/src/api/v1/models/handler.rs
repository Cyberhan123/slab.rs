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
    LoadModelRequest, ModelCatalogItemResponse, ModelStatusResponse, SwitchModelRequest,
    UpdateModelRequest,
};
use crate::api::v1::tasks::schema::OperationAcceptedResponse;
use crate::api::validation::{validate, ValidatedJson, ValidatedQuery};
use crate::context::AppState;
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
    Ok(Json(service.create_model(req.into()).await?.into()))
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
    Ok(Json(
        service.update_model(&params.id, req.into()).await?.into(),
    ))
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
    let view = service.delete_model(&params.id).await?;
    Ok(Json(serde_json::json!({
        "id": view.id,
        "status": view.status,
    })))
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
        .list_models(query.into())
        .await?
        .into_iter()
        .map(Into::into)
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
    Ok(Json(service.load_model(req.into()).await?.into()))
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
    Ok(Json(service.unload_model(req.into()).await?.into()))
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
    let response = service.list_available_models(query.into()).await?;
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
    Ok(Json(service.switch_model(req.into()).await?.into()))
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
    let response = service.download_model(req.into()).await?;
    Ok((StatusCode::ACCEPTED, Json(response.into())))
}

#[derive(Debug, Deserialize, Validate)]
struct ModelIdPath {
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "id must not be empty"
    ))]
    id: String,
}
