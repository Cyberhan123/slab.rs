use std::sync::Arc;

use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use utoipa::{OpenApi, ToSchema};

const MAX_MODEL_PACK_SIZE: usize = 10 * 1024 * 1024 * 1024; // 10GB

use crate::api::v1::models::schema::{
    AvailableModelsResponse, CreateModelRequest, DeleteModelResponse, DownloadModelRequest,
    ListAvailableQuery, ListModelsQuery, LoadModelRequest, ModelConfigDocumentResponse,
    ModelRuntimeStateResponse, ModelStatusResponse, SwitchModelRequest, UnifiedModelResponse,
    UnloadModelRequest, UpdateModelConfigSelectionRequest, UpdateModelRequest,
};
use crate::api::v1::path::IdPath;
use crate::api::v1::tasks::schema::OperationAcceptedResponse;
use crate::api::validation::{ValidatedJson, ValidatedQuery, validate};
use crate::error::ServerError;
use slab_app_core::context::AppState;
use slab_app_core::domain::models::UnifiedModel;
use slab_app_core::domain::services::ModelService;

#[allow(dead_code)]
#[derive(ToSchema)]
struct ImportModelPackMultipartRequest {
    #[schema(value_type = String, format = Binary)]
    file: Vec<u8>,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        list_models,
        create_model,
        import_model_pack,
        get_model,
        get_model_config_document,
        update_model,
        update_model_config_selection,
        delete_model,
        load_model,
        unload_model,
        list_available_models,
        switch_model,
        download_model
    ),
    components(schemas(
        CreateModelRequest,
        ImportModelPackMultipartRequest,
        UpdateModelRequest,
        UpdateModelConfigSelectionRequest,
        LoadModelRequest,
        UnloadModelRequest,
        ModelStatusResponse,
        SwitchModelRequest,
        DownloadModelRequest,
        DeleteModelResponse,
        AvailableModelsResponse,
        ListAvailableQuery,
        ListModelsQuery,
        ModelRuntimeStateResponse,
        UnifiedModelResponse,
        ModelConfigDocumentResponse,
        OperationAcceptedResponse
    ))
)]
pub struct ModelsApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/models", get(list_models).post(create_model))
        .route("/models/import-pack", post(import_model_pack))
        .route("/models/{id}", get(get_model).put(update_model).delete(delete_model))
        .route("/models/{id}/config-document", get(get_model_config_document))
        .route("/models/{id}/config-selection", axum::routing::put(update_model_config_selection))
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
        (status = 200, description = "Model created", body = UnifiedModelResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn create_model(
    State(service): State<ModelService>,
    ValidatedJson(req): ValidatedJson<CreateModelRequest>,
) -> Result<Json<UnifiedModelResponse>, ServerError> {
    let model = service.create_model(req.into()).await?;
    Ok(Json(model_response(&service, model).await))
}

#[utoipa::path(
    post,
    path = "/v1/models/import-pack",
    tag = "models",
    request_body(
        content = ImportModelPackMultipartRequest,
        content_type = "multipart/form-data",
        description = "Upload a .slab model pack as a multipart file field named `file`."
    ),
    responses(
        (status = 200, description = "Model pack imported and stored", body = UnifiedModelResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn import_model_pack(
    State(service): State<ModelService>,
    mut multipart: Multipart,
) -> Result<Json<UnifiedModelResponse>, ServerError> {
    while let Some(field) = multipart.next_field().await.map_err(|error| {
        ServerError::BadRequest(format!("failed to read multipart field: {error}"))
    })? {
        let file_name = field.file_name().map(str::to_owned);
        if file_name.is_none() {
            continue;
        }
        if let Some(file_name) = file_name.as_deref()
            && !file_name.trim().to_ascii_lowercase().ends_with(".slab")
        {
            return Err(ServerError::BadRequest(format!(
                "uploaded model pack must use the .slab extension: {file_name}"
            )));
        }

        let bytes = field.bytes().await.map_err(|error| {
            ServerError::BadRequest(format!("failed to read model pack bytes: {error}"))
        })?;

        if bytes.is_empty() {
            return Err(ServerError::BadRequest("uploaded model pack is empty".into()));
        }

        if bytes.len() > MAX_MODEL_PACK_SIZE {
            return Err(ServerError::BadRequest(format!(
                "uploaded model pack is too large ({} bytes); maximum size is {} bytes (10GB)",
                bytes.len(),
                MAX_MODEL_PACK_SIZE
            )));
        }

        let model = service.import_model_pack_bytes(bytes.as_ref()).await?;
        return Ok(Json(model_response(&service, model).await));
    }

    Err(ServerError::BadRequest("multipart body must contain a .slab file field".into()))
}

#[utoipa::path(
    get,
    path = "/v1/models/{id}",
    tag = "models",
    params(
        ("id" = String, Path, description = "Model ID")
    ),
    responses(
        (status = 200, description = "Model details", body = UnifiedModelResponse),
        (status = 404, description = "Model not found"),
        (status = 500, description = "Backend error"),
    )
)]
async fn get_model(
    State(service): State<ModelService>,
    Path(params): Path<IdPath>,
) -> Result<Json<UnifiedModelResponse>, ServerError> {
    let params = validate(params)?;
    let model = service.get_model(&params.id).await?;
    Ok(Json(model_response(&service, model).await))
}

#[utoipa::path(
    get,
    path = "/v1/models/{id}/config-document",
    tag = "models",
    params(
        ("id" = String, Path, description = "Model ID")
    ),
    responses(
        (status = 200, description = "Model config document", body = ModelConfigDocumentResponse),
        (status = 404, description = "Model not found"),
        (status = 500, description = "Backend error"),
    )
)]
async fn get_model_config_document(
    State(service): State<ModelService>,
    Path(params): Path<IdPath>,
) -> Result<Json<ModelConfigDocumentResponse>, ServerError> {
    let params = validate(params)?;
    Ok(Json(service.get_model_config_document(&params.id).await?.into()))
}

#[utoipa::path(
    put,
    path = "/v1/models/{id}",
    tag = "models",
    request_body = UpdateModelRequest,
    params(
        ("id" = String, Path, description = "Model ID")
    ),
    responses(
        (status = 200, description = "Model updated", body = UnifiedModelResponse),
        (status = 400, description = "Bad request"),
        (status = 404, description = "Model not found"),
        (status = 500, description = "Backend error"),
    )
)]
async fn update_model(
    State(service): State<ModelService>,
    Path(params): Path<IdPath>,
    ValidatedJson(req): ValidatedJson<UpdateModelRequest>,
) -> Result<Json<UnifiedModelResponse>, ServerError> {
    let params = validate(params)?;
    let model = service.update_model(&params.id, req.into()).await?;
    Ok(Json(model_response(&service, model).await))
}

#[utoipa::path(
    put,
    path = "/v1/models/{id}/config-selection",
    tag = "models",
    request_body = UpdateModelConfigSelectionRequest,
    params(
        ("id" = String, Path, description = "Model ID")
    ),
    responses(
        (status = 200, description = "Model config selection updated", body = UnifiedModelResponse),
        (status = 400, description = "Bad request"),
        (status = 404, description = "Model not found"),
        (status = 500, description = "Backend error"),
    )
)]
async fn update_model_config_selection(
    State(service): State<ModelService>,
    Path(params): Path<IdPath>,
    ValidatedJson(req): ValidatedJson<UpdateModelConfigSelectionRequest>,
) -> Result<Json<UnifiedModelResponse>, ServerError> {
    let params = validate(params)?;
    let model = service.update_model_config_selection(&params.id, req.into()).await?;
    Ok(Json(model_response(&service, model).await))
}

#[utoipa::path(
    delete,
    path = "/v1/models/{id}",
    tag = "models",
    params(
        ("id" = String, Path, description = "Model ID")
    ),
    responses(
        (status = 200, description = "Model deleted", body = DeleteModelResponse),
        (status = 404, description = "Model not found"),
        (status = 500, description = "Backend error"),
    )
)]
async fn delete_model(
    State(service): State<ModelService>,
    Path(params): Path<IdPath>,
) -> Result<Json<DeleteModelResponse>, ServerError> {
    let params = validate(params)?;
    let view = service.delete_model(&params.id).await?;
    Ok(Json(view.into()))
}

#[utoipa::path(
    get,
    path = "/v1/models",
    tag = "models",
    params(ListModelsQuery),
    responses(
        (status = 200, description = "List all models (local and cloud)", body = [UnifiedModelResponse]),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn list_models(
    State(service): State<ModelService>,
    Query(query): Query<ListModelsQuery>,
) -> Result<Json<Vec<UnifiedModelResponse>>, ServerError> {
    let mut items = Vec::new();
    for model in service.list_models(query.into()).await? {
        items.push(model_response(&service, model).await);
    }
    Ok(Json(items))
}

async fn model_response(service: &ModelService, model: UnifiedModel) -> UnifiedModelResponse {
    let runtime_state = service.runtime_state_for_model(&model).await;
    UnifiedModelResponse::from_model(model, runtime_state)
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
    request_body = UnloadModelRequest,
    responses(
        (status = 200, description = "Model unloaded", body = ModelStatusResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn unload_model(
    State(service): State<ModelService>,
    ValidatedJson(req): ValidatedJson<UnloadModelRequest>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    Ok(Json(service.unload_model(req.into()).await?.into()))
}

#[utoipa::path(
    get,
    path = "/v1/models/available",
    tag = "models",
    params(ListAvailableQuery),
    responses(
        (status = 200, description = "List of available files in a HuggingFace repo", body = AvailableModelsResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn list_available_models(
    State(service): State<ModelService>,
    ValidatedQuery(query): ValidatedQuery<ListAvailableQuery>,
) -> Result<Json<AvailableModelsResponse>, ServerError> {
    let response = service.list_available_models(query.into()).await?;
    Ok(Json(response.into()))
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
        (status = 404, description = "Model not found"),
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

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode, header};
    use serde_json::json;

    use crate::api::test_support::{TEST_PROVIDER_ID, TestServer, response_json};

    #[tokio::test]
    async fn create_model_validates_request_body() {
        let server = TestServer::new().await;

        let response = server
            .post_json(
                "/v1/models",
                json!({
                    "display_name": " ",
                    "kind": "local",
                    "backend_id": "ggml.llama",
                    "spec": {}
                }),
            )
            .await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST);
        assert!(response.body["message"].as_str().unwrap_or_default().contains("display_name"));
    }

    #[tokio::test]
    async fn model_path_validation_rejects_blank_ids() {
        let server = TestServer::new().await;

        let response = server.get("/v1/models/%20").await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST);
        assert!(response.body["message"].as_str().unwrap_or_default().contains("id"));
    }

    #[tokio::test]
    async fn create_and_get_cloud_model_round_trip_over_http() {
        let server = TestServer::new().await;

        let created = server
            .post_json(
                "/v1/models",
                json!({
                    "display_name": "Cloud Chat",
                    "kind": "cloud",
                    "capabilities": ["chat_generation"],
                    "spec": {
                        "provider_id": TEST_PROVIDER_ID,
                        "remote_model_id": "gpt-4.1-mini"
                    }
                }),
            )
            .await;

        assert_eq!(created.status, StatusCode::OK);
        let id = created.body["id"].as_str().expect("created id");

        let fetched = server.get(&format!("/v1/models/{id}")).await;
        assert_eq!(fetched.status, StatusCode::OK);
        assert_eq!(fetched.body["id"], id);
        assert_eq!(fetched.body["display_name"], "Cloud Chat");
        assert_eq!(fetched.body["spec"]["provider_id"], TEST_PROVIDER_ID);
    }

    #[tokio::test]
    async fn list_available_models_validates_repo_id_query() {
        let server = TestServer::new().await;

        let response = server.get("/v1/models/available?repo_id=%20").await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST);
        assert!(response.body["message"].as_str().unwrap_or_default().contains("repo_id"));
    }

    #[tokio::test]
    async fn download_model_maps_missing_model_to_not_found() {
        let server = TestServer::new().await;

        let response =
            server.post_json("/v1/models/download", json!({ "model_id": "missing-model" })).await;

        assert_eq!(response.status, StatusCode::NOT_FOUND);
        assert_eq!(response.body["i18n"]["message"]["key"], "server.errors.notFound");
    }

    #[tokio::test]
    async fn import_model_pack_rejects_non_slab_extension_before_service_call() {
        let server = TestServer::new().await;
        let boundary = "slab-test-boundary";
        let body = concat!(
            "--slab-test-boundary\r\n",
            "Content-Disposition: form-data; name=\"file\"; filename=\"pack.txt\"\r\n",
            "Content-Type: application/octet-stream\r\n",
            "\r\n",
            "not a slab pack\r\n",
            "--slab-test-boundary--\r\n"
        );
        let request = Request::builder()
            .method(Method::POST)
            .uri("/v1/models/import-pack")
            .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
            .body(Body::from(body))
            .expect("multipart request");

        let response = response_json(server.raw(request).await).await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST);
        assert!(response.body["message"].as_str().unwrap_or_default().contains(".slab extension"));
    }
}
