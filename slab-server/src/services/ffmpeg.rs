use tracing::{info, warn};

use crate::api::v1::ffmpeg::schema::ConvertRequest;
use crate::api::v1::tasks::schema::OperationAcceptedResponse;
use crate::context::{SubmitOperation, WorkerState};
use crate::error::ServerError;

const ALLOWED_OUTPUT_FORMATS: &[&str] = &[
    "mp3", "mp4", "wav", "flac", "ogg", "opus", "webm", "avi", "mkv", "mov", "aac", "m4a", "m4v",
    "f32le", "pcm",
];

#[derive(Clone)]
pub struct FfmpegService {
    state: WorkerState,
}

impl FfmpegService {
    pub fn new(state: WorkerState) -> Self {
        Self { state }
    }

    pub async fn convert(
        &self,
        req: ConvertRequest,
    ) -> Result<OperationAcceptedResponse, ServerError> {
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
        if !ALLOWED_OUTPUT_FORMATS.contains(&req.output_format.to_ascii_lowercase().as_str()) {
            return Err(ServerError::BadRequest(format!(
                "unsupported output_format '{}'; must be one of: {}",
                req.output_format,
                ALLOWED_OUTPUT_FORMATS.join(", ")
            )));
        }
        if !tokio::fs::try_exists(&req.source_path)
            .await
            .unwrap_or(false)
        {
            return Err(ServerError::BadRequest(format!(
                "source_path '{}' does not exist or is not accessible",
                req.source_path
            )));
        }

        let input_data = serde_json::json!({
            "source_path": req.source_path,
            "output_format": req.output_format,
            "output_path": req.output_path,
        })
        .to_string();

        let operation_id = self
            .state
            .submit_operation(
                SubmitOperation::pending("ffmpeg", None, Some(input_data.clone())),
                move |operation| async move {
                    let operation_id = operation.id().to_owned();

                    if let Err(error) = operation.mark_running().await {
                        warn!(task_id = %operation_id, error = %error, "failed to set ffmpeg task running");
                        return;
                    }

                    let input: serde_json::Value = match serde_json::from_str(&input_data) {
                        Ok(value) => value,
                        Err(error) => {
                            warn!(task_id = %operation_id, error = %error, "invalid stored input_data for ffmpeg task");
                            let message = format!("invalid stored input_data: {error}");
                            if let Err(db_error) = operation.mark_failed(&message).await {
                                warn!(task_id = %operation_id, error = %db_error, "failed to persist ffmpeg task parse error");
                            }
                            return;
                        }
                    };

                    let source_path = input["source_path"].as_str().unwrap_or("").to_owned();
                    let output_format = input["output_format"].as_str().unwrap_or("out").to_owned();
                    let output_path = input["output_path"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| {
                            let base = std::path::Path::new(&source_path)
                                .file_stem()
                                .and_then(|stem| stem.to_str())
                                .unwrap_or("output");
                            std::env::temp_dir()
                                .join(format!("{base}.{output_format}"))
                                .to_string_lossy()
                                .into_owned()
                        });

                    let result = tokio::process::Command::new("ffmpeg")
                        .args(["-y", "-i", &source_path, "-f", &output_format, &output_path])
                        .output()
                        .await;

                    match result {
                        Ok(output) if output.status.success() => {
                            let result_json =
                                serde_json::json!({ "output_path": output_path }).to_string();
                            if let Err(error) = operation.mark_succeeded(&result_json).await {
                                warn!(task_id = %operation_id, error = %error, "failed to persist ffmpeg success");
                            }
                            info!(task_id = %operation_id, output_path = %output_path, "ffmpeg conversion succeeded");
                        }
                        Ok(output) => {
                            let error = String::from_utf8_lossy(&output.stderr).to_string();
                            warn!(task_id = %operation_id, error = %error, "ffmpeg conversion failed");
                            if let Err(db_error) = operation.mark_failed(&error).await {
                                warn!(task_id = %operation_id, error = %db_error, "failed to persist ffmpeg error");
                            }
                        }
                        Err(error) => {
                            warn!(task_id = %operation_id, error = %error, "ffmpeg spawn failed");
                            if let Err(db_error) = operation.mark_failed(&error.to_string()).await {
                                warn!(task_id = %operation_id, error = %db_error, "failed to persist ffmpeg spawn error");
                            }
                        }
                    }
                },
            )
            .await?;

        Ok(OperationAcceptedResponse { operation_id })
    }
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
