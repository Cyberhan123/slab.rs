use std::sync::Arc;

use slab_app_core::context::AppState;
use slab_app_core::schemas::images::ImageGenerationRequest;
use slab_app_core::schemas::tasks::OperationAcceptedResponse;

use crate::api::validation::{map_err, validate};

#[tauri::command(async)]
pub async fn generate_images(
    state: tauri::State<'_, Arc<AppState>>,
    req: ImageGenerationRequest,
) -> Result<OperationAcceptedResponse, String> {
    let req = validate(req)?;
    let command = req.try_into().map_err(map_err)?;
    Ok(state.services.image.generate_images(command).await.map_err(map_err)?.into())
}
