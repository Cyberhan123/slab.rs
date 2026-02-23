//! FFmpeg async conversion task endpoints.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;

use crate::db::{TaskRecord, TaskStore};
use crate::error::ServerError;
use crate::state::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/ffmpeg/convert", post(convert))
}

#[derive(Deserialize)]
pub struct ConvertRequest {
    /// Absolute path to the source file.
    pub source_path: String,
    /// Desired output format (e.g. `"mp3"`, `"wav"`, `"mp4"`).
    pub output_format: String,
    /// Optional output path; defaults to source path with new extension.
    pub output_path: Option<String>,
}

#[derive(Serialize)]
pub struct ConvertResponse {
    pub task_id: String,
}

pub async fn convert(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ConvertRequest>,
) -> Result<Json<ConvertResponse>, ServerError> {
    if req.source_path.is_empty() || !std::path::Path::new(&req.source_path).is_absolute() {
        return Err(ServerError::BadRequest(
            "source_path must be an absolute path".into(),
        ));
    }
    if req.source_path.contains("..") {
        return Err(ServerError::BadRequest(
            "source_path must not contain '..'".into(),
        ));
    }
    if req.output_format.is_empty() {
        return Err(ServerError::BadRequest(
            "output_format must not be empty".into(),
        ));
    }
    // Verify the source file exists and is readable before accepting the task.
    if !tokio::fs::try_exists(&req.source_path).await.unwrap_or(false) {
        return Err(ServerError::BadRequest(format!(
            "source_path '{}' does not exist or is not accessible",
            req.source_path
        )));
    }

    let task_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let input_data = serde_json::json!({
        "source_path": req.source_path,
        "output_format": req.output_format,
        "output_path": req.output_path,
    })
    .to_string();

    state
        .store
        .insert_task(TaskRecord {
            id: task_id.clone(),
            task_type: "ffmpeg".into(),
            status: "pending".into(),
            input_data: Some(input_data.clone()),
            result_data: None,
            error_msg: None,
            created_at: now,
            updated_at: now,
        })
        .await?;

    let store = Arc::clone(&state.store);
    let task_manager = Arc::clone(&state.task_manager);
    let tid = task_id.clone();

    let join = tokio::spawn(async move {
        store.update_task_status(&tid, "running", None, None).await.ok();

        let input: serde_json::Value = serde_json::from_str(&input_data).unwrap_or_default();

        let output_path = input["output_path"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| {
                let src = input["source_path"].as_str().unwrap_or("output");
                let fmt = input["output_format"].as_str().unwrap_or("out");
                let base = std::path::Path::new(src)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("output");
                std::env::temp_dir()
                    .join(format!("{base}.{fmt}"))
                    .to_string_lossy()
                    .into_owned()
            });

        let source_path = input["source_path"].as_str().unwrap_or("").to_owned();

        let result = tokio::process::Command::new("ffmpeg")
            .args(["-y", "-i", &source_path, &output_path])
            .output()
            .await;

        match result {
            Ok(output) if output.status.success() => {
                let result_json =
                    serde_json::json!({ "output_path": output_path }).to_string();
                store
                    .update_task_status(&tid, "succeeded", Some(&result_json), None)
                    .await
                    .ok();
                info!(task_id = %tid, output_path = %output_path, "ffmpeg conversion succeeded");
            }
            Ok(output) => {
                let err = String::from_utf8_lossy(&output.stderr).to_string();
                warn!(task_id = %tid, error = %err, "ffmpeg conversion failed");
                store
                    .update_task_status(&tid, "failed", None, Some(&err))
                    .await
                    .ok();
            }
            Err(e) => {
                warn!(task_id = %tid, error = %e, "ffmpeg spawn failed");
                store
                    .update_task_status(&tid, "failed", None, Some(&e.to_string()))
                    .await
                    .ok();
            }
        }
        task_manager.remove(&tid);
    });

    state.task_manager.insert(task_id.clone(), join.abort_handle());

    Ok(Json(ConvertResponse { task_id }))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn rejects_relative_path() {
        let path = "relative/path.mp4";
        assert!(!std::path::Path::new(path).is_absolute());
    }

    #[test]
    fn rejects_traversal_path() {
        let path = "/foo/../../../etc/passwd";
        assert!(path.contains(".."));
    }
}
