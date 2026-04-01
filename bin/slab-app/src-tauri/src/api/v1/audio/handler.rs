use std::sync::Arc;

use slab_app_core::context::AppState;
use slab_app_core::schemas::audio::CompletionRequest;
use slab_app_core::schemas::tasks::OperationAcceptedResponse;

use crate::api::validation::{map_err, validate};

#[tauri::command(async)]
pub async fn transcribe(
    state: tauri::State<'_, Arc<AppState>>,
    req: CompletionRequest,
) -> Result<OperationAcceptedResponse, String> {
    let req = validate(req)?;
    Ok(state.services.audio.transcribe(req.into()).await.map_err(map_err)?.into())
}
