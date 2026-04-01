use std::sync::Arc;

use slab_app_core::context::AppState;
use slab_app_core::schemas::tasks::OperationAcceptedResponse;
use slab_app_core::schemas::video::VideoGenerationRequest;

use crate::api::validation::{map_err, validate};

#[tauri::command(async)]
pub async fn generate_video(
    state: tauri::State<'_, Arc<AppState>>,
    req: VideoGenerationRequest,
) -> Result<OperationAcceptedResponse, String> {
    let req = validate(req)?;
    let command = req.try_into().map_err(map_err)?;
    Ok(state.services.video.generate_video(command).await.map_err(map_err)?.into())
}
