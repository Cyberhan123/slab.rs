use std::sync::Arc;

use slab_app_core::context::AppState;
use slab_app_core::error::AppCoreError;
use slab_app_core::schemas::ffmpeg::ConvertRequest;
use slab_app_core::schemas::tasks::OperationAcceptedResponse;

use crate::api::validation::{map_err, validate};

#[tauri::command(async)]
pub async fn convert(
    state: tauri::State<'_, Arc<AppState>>,
    req: ConvertRequest,
) -> Result<OperationAcceptedResponse, String> {
    let req = validate(req)?;
    if !tokio::fs::try_exists(&req.source_path).await.unwrap_or(false) {
        return Err(map_err(AppCoreError::BadRequest(format!(
            "source_path '{}' does not exist or is not accessible",
            req.source_path
        ))));
    }

    Ok(state.services.ffmpeg.convert(req.into()).await.map_err(map_err)?.into())
}
