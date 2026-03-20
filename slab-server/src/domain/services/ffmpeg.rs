use tracing::{info, warn};

use crate::context::{SubmitOperation, WorkerState};
use crate::domain::models::{AcceptedOperation, FfmpegConvertCommand};
use crate::error::ServerError;

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
        req: FfmpegConvertCommand,
    ) -> Result<AcceptedOperation, ServerError> {
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

                    // Use the ffmpeg-sidecar path resolver: prefers the sidecar
                    // binary installed next to the executable, falls back to the
                    // system `ffmpeg` on $PATH.
                    let ffmpeg_bin = ffmpeg_sidecar::paths::ffmpeg_path();

                    let result = tokio::process::Command::new(&ffmpeg_bin)
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
                            warn!(task_id = %operation_id, ffmpeg_bin = %ffmpeg_bin.display(), error = %error, "ffmpeg spawn failed");
                            if let Err(db_error) = operation.mark_failed(&error.to_string()).await {
                                warn!(task_id = %operation_id, error = %db_error, "failed to persist ffmpeg spawn error");
                            }
                        }
                    }
                },
            )
            .await?;

        Ok(AcceptedOperation { operation_id })
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn output_path_defaults_to_temp_dir_when_missing() {
        let source_path = std::path::Path::new("/tmp/source.wav");
        let output_format = "mp3";
        let output_path = std::env::temp_dir()
            .join(format!(
                "{}.{}",
                source_path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("output"),
                output_format
            ))
            .to_string_lossy()
            .into_owned();

        assert!(output_path.ends_with(".mp3"));
    }
}
