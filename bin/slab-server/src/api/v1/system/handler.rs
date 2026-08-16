use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::v1::system::schema::{
    AgentDiagnosticsResponse, AgentThreadStatResponse, FailedToolCallResponse, GpuDeviceStatus,
    GpuLedgerDeviceResponse, GpuLedgerEntryResponse, GpuLedgerGaugeResponse, GpuLedgerResponse,
    GpuStatusResponse, SystemDiagnosticPathResponse, SystemDiagnosticsResponse,
};
use crate::error::ServerError;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::SystemService;

#[derive(OpenApi)]
#[openapi(
    paths(gpu_status, gpu_ledger, system_diagnostics, agent_diagnostics),
    components(schemas(
        GpuStatusResponse,
        GpuDeviceStatus,
        GpuLedgerResponse,
        GpuLedgerDeviceResponse,
        GpuLedgerEntryResponse,
        GpuLedgerGaugeResponse,
        SystemDiagnosticsResponse,
        SystemDiagnosticPathResponse,
        AgentDiagnosticsResponse,
        AgentThreadStatResponse,
        FailedToolCallResponse
    ))
)]
pub struct SystemApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/system/gpu", get(gpu_status))
        .route("/system/gpu/ledger", get(gpu_ledger))
        .route("/system/diagnostics", get(system_diagnostics))
        .route("/system/diagnostics/agent-stats", get(agent_diagnostics))
}

#[utoipa::path(
    get,
    path = "/v1/system/gpu",
    tag = "system",
    responses(
        (status = 200, description = "Current GPU telemetry snapshot", body = GpuStatusResponse),
    )
)]
async fn gpu_status(State(service): State<SystemService>) -> Json<GpuStatusResponse> {
    Json(service.gpu_status().await.into())
}

#[utoipa::path(
    get,
    path = "/v1/system/gpu/ledger",
    tag = "system",
    responses(
        (status = 200, description = "Resident model memory ledger (diagnostics)", body = GpuLedgerResponse),
    )
)]
async fn gpu_ledger(State(service): State<SystemService>) -> Json<GpuLedgerResponse> {
    Json(service.gpu_ledger().await)
}

#[utoipa::path(
    get,
    path = "/v1/system/diagnostics",
    tag = "system",
    responses(
        (status = 200, description = "Read-only local diagnostics snapshot", body = SystemDiagnosticsResponse),
        (status = 500, description = "Backend error"),
    )
)]
async fn system_diagnostics(
    State(service): State<SystemService>,
) -> Result<Json<SystemDiagnosticsResponse>, ServerError> {
    Ok(Json(service.diagnostics().await?.into()))
}

#[utoipa::path(
    get,
    path = "/v1/system/diagnostics/agent-stats",
    tag = "system",
    responses(
        (status = 200, description = "Recent agent thread stats + failed tool calls", body = AgentDiagnosticsResponse),
        (status = 500, description = "Backend error"),
    )
)]
async fn agent_diagnostics(
    State(service): State<SystemService>,
) -> Result<Json<AgentDiagnosticsResponse>, ServerError> {
    Ok(Json(service.agent_diagnostics().await?))
}
