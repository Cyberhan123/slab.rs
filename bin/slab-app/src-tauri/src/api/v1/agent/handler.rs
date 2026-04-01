use std::sync::Arc;

use slab_app_core::context::AppState;
use slab_app_core::schemas::agent::{
    AgentInputRequest, AgentInputResponse, AgentShutdownResponse, AgentStatusResponse,
    SpawnAgentRequest, SpawnAgentResponse,
};

use crate::api::validation::{map_err, validate, validate_id};

#[tauri::command(async)]
pub async fn spawn_agent(
    state: tauri::State<'_, Arc<AppState>>,
    req: SpawnAgentRequest,
) -> Result<SpawnAgentResponse, String> {
    let req = validate(req)?;
    let messages = req.messages.into_iter().map(Into::into).collect();
    let thread_id = state
        .services
        .agent
        .spawn(req.session_id, req.config.into(), messages)
        .await
        .map_err(map_err)?;
    Ok(SpawnAgentResponse { thread_id })
}

#[tauri::command(async)]
pub async fn agent_input(
    _state: tauri::State<'_, Arc<AppState>>,
    id: String,
    req: AgentInputRequest,
) -> Result<AgentInputResponse, String> {
    validate_id(&id)?;
    let _content = req.content;
    Ok(AgentInputResponse {
        accepted: false,
        message: "send_input is not yet implemented; the agent runs autonomously once spawned"
            .to_owned(),
    })
}

#[tauri::command(async)]
pub async fn agent_status(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<AgentStatusResponse, String> {
    validate_id(&id)?;
    let status = state.services.agent.get_status(&id).await.map_err(map_err)?;
    Ok(AgentStatusResponse { thread_id: id, status: status.into() })
}

#[tauri::command(async)]
pub async fn agent_shutdown(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<AgentShutdownResponse, String> {
    validate_id(&id)?;
    state.services.agent.shutdown(&id).await.map_err(map_err)?;
    Ok(AgentShutdownResponse { thread_id: id, shutdown: true })
}
