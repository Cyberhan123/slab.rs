//! FFmpeg async conversion task endpoints.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use chrono::Utc;
use tracing::{info, warn};
use uuid::Uuid;
use utoipa::OpenApi;

use crate::entities::{TaskRecord, TaskStore};
use crate::error::ServerError;
use crate::state::AppState;
use crate::schemas::v1::ffmpeg::{ConvertRequest, ConvertResponse};

#[derive(OpenApi)]
#[openapi(
    paths(convert),
    components(schemas(
        ConvertRequest, 
        ConvertResponse, 
    )),
)]
pub struct FfmpegApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/ffmpeg/convert", post(convert))
}

/// Allowlisted output formats for ffmpeg conversions.
/// Only these formats are accepted to prevent command injection through format strings.
const ALLOWED_OUTPUT_FORMATS: &[&str] = &[
    "mp3", "mp4", "wav", "flac", "ogg", "opus", "webm",
    "avi", "mkv", "mov", "aac", "m4a", "m4v", "f32le", "pcm",
];

#[utoipa::path(
    post,
    path = "/v1/ffmpeg/convert",
    tag = "ffmpeg",
    request_body = ConvertRequest,
    responses(
        (status = 200, description = "Conversion task created", body = ConvertResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
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
    // Validate output format against allowlist to prevent command injection.
    if !ALLOWED_OUTPUT_FORMATS.contains(&req.output_format.to_ascii_lowercase().as_str()) {
        return Err(ServerError::BadRequest(format!(
            "unsupported output_format '{}'; must be one of: {}",
            req.output_format,
            ALLOWED_OUTPUT_FORMATS.join(", ")
        )));
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
            core_task_id: None,
            created_at: now,
            updated_at: now,
        })
        .await?;

    let store = Arc::clone(&state.store);
    let task_manager = Arc::clone(&state.task_manager);
    let tid = task_id.clone();

    let join = tokio::spawn(async move {
        store.update_task_status(&tid, "running", None, None).await.ok();

        let input: serde_json::Value = match serde_json::from_str(&input_data) {
            Ok(v) => v,
            Err(e) => {
                warn!(task_id = %tid, error = %e, "invalid stored input_data for ffmpeg task");
                store.update_task_status(&tid, "failed", None, Some(&format!("invalid stored input_data: {e}"))).await.ok();
                task_manager.remove(&tid);
                return;
            }
        };

        let source_path    = input["source_path"].as_str().unwrap_or("").to_owned();
        let output_format  = input["output_format"].as_str().unwrap_or("out").to_owned();
        let output_path = input["output_path"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| {
                let base = std::path::Path::new(&source_path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("output");
                std::env::temp_dir()
                    .join(format!("{base}.{output_format}"))
                    .to_string_lossy()
                    .into_owned()
            });

        // Pass `-f {output_format}` explicitly so ffmpeg uses the validated
        // format regardless of the output filename extension.
        let result = tokio::process::Command::new("ffmpeg")
            .args(["-y", "-i", &source_path, "-f", &output_format, &output_path])
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

    #[test]
    fn accepts_allowed_formats() {
        for fmt in &["mp3", "mp4", "wav", "flac", "ogg"] {
            assert!(
                ALLOWED_OUTPUT_FORMATS.contains(fmt),
                "{fmt} should be allowed"
            );
        }
    }

    #[test]
    fn rejects_unknown_format() {
        assert!(!ALLOWED_OUTPUT_FORMATS.contains(&"exe"));
        assert!(!ALLOWED_OUTPUT_FORMATS.contains(&"sh"));
    }
}
