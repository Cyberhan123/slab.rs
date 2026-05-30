use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::context::worker_state::OperationContext;
use crate::context::{SubmitOperation, WorkerState};
use crate::domain::models::{AcceptedOperation, FfmpegConvertCommand, TaskProgress, TaskStatus};
use crate::domain::services::ffmpeg_next_audio::{supports_output_format, transcode_audio};
use crate::domain::services::ffmpeg_next_remux::{
    remux_media, supports_output_format as supports_remux_output_format,
};
use crate::domain::services::ffmpeg_runtime::ensure_dynamic_runtime_ready;
use crate::error::AppCoreError;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FfmpegConvertInputData {
    source_path: String,
    output_format: String,
    output_path: Option<String>,
}

#[derive(Serialize)]
struct FfmpegProgressPayload {
    progress: TaskProgress,
}

#[derive(Serialize)]
struct FfmpegSuccessPayload {
    output_path: String,
    progress: TaskProgress,
}

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
    ) -> Result<AcceptedOperation, AppCoreError> {
        let input = FfmpegConvertInputData {
            source_path: req.source_path,
            output_format: req.output_format,
            output_path: req.output_path,
        };
        let input_data = serde_json::to_string(&input).map_err(|error| {
            AppCoreError::Internal(format!("failed to serialize ffmpeg input data: {error}"))
        })?;

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

                    let input: FfmpegConvertInputData = match serde_json::from_str(&input_data) {
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

                    let source_path = input.source_path;
                    let output_format = input.output_format;
                    let output_path = input.output_path.unwrap_or_else(|| {
                        let base = std::path::Path::new(&source_path)
                            .file_stem()
                            .and_then(|stem| stem.to_str())
                            .unwrap_or("output");
                        std::env::temp_dir()
                            .join(format!("{base}.{output_format}"))
                            .to_string_lossy()
                            .into_owned()
                    });

                    let mut progress = FfmpegProgressState::new(output_path.clone());

                    if let Err(error) = ensure_dynamic_runtime_ready() {
                        progress.push_log(error.clone());
                        let payload = progress.to_payload();
                        if let Err(db_error) = operation
                            .update_status(TaskStatus::Failed, Some(&payload), Some(&error))
                            .await
                        {
                            warn!(task_id = %operation_id, error = %db_error, "failed to persist ffmpeg runtime initialization error");
                        }
                        return;
                    }

                    if let Err(error) = publish_ffmpeg_progress(&operation, &progress).await {
                        warn!(task_id = %operation_id, error = %error, "failed to publish initial ffmpeg progress");
                    }

                    let supported_format = supports_output_format(&output_format)
                        || supports_remux_output_format(&output_format);
                    if !supported_format {
                        progress.push_log(format!(
                            "unsupported output format '{output_format}' in static ffmpeg-next mode"
                        ));
                        let error = format!(
                            "unsupported output format '{output_format}' for ffmpeg-next static mode"
                        );
                        let payload = progress.to_payload();
                        if let Err(db_error) = operation
                            .update_status(TaskStatus::Failed, Some(&payload), Some(&error))
                            .await
                        {
                            warn!(task_id = %operation_id, error = %db_error, "failed to persist unsupported-format error");
                        }
                        return;
                    }

                    if supports_output_format(&output_format) {
                        let source_path_for_worker = source_path.clone();
                        let output_path_for_worker = output_path.clone();
                        let transcode_result = tokio::task::spawn_blocking(move || {
                            transcode_audio(&source_path_for_worker, &output_path_for_worker)
                        })
                        .await;

                        match transcode_result {
                            Ok(Ok(())) => {
                                progress.mark_complete();
                                progress.push_log("ffmpeg-next audio transcoding completed".to_owned());
                                let result_json = progress.to_success_payload();
                                if let Err(error) = operation.mark_succeeded(&result_json).await {
                                    warn!(task_id = %operation_id, error = %error, "failed to persist ffmpeg-next conversion success");
                                }
                                info!(task_id = %operation_id, output_path = %output_path, "ffmpeg-next audio conversion succeeded");
                                return;
                            }
                            Ok(Err(error)) => {
                                progress.push_log(format!("ffmpeg-next audio conversion failed: {error}"));
                                let payload = progress.to_payload();
                                let error_text = error.to_string();
                                if let Err(db_error) = operation
                                    .update_status(
                                        TaskStatus::Failed,
                                        Some(&payload),
                                        Some(&error_text),
                                    )
                                    .await
                                {
                                    warn!(task_id = %operation_id, error = %db_error, "failed to persist ffmpeg-next conversion error");
                                }
                            }
                            Err(error) => {
                                progress.push_log(format!("ffmpeg-next audio conversion worker failed: {error}"));
                                let payload = progress.to_payload();
                                let error_text = error.to_string();
                                if let Err(db_error) = operation
                                    .update_status(
                                        TaskStatus::Failed,
                                        Some(&payload),
                                        Some(&error_text),
                                    )
                                    .await
                                {
                                    warn!(task_id = %operation_id, error = %db_error, "failed to persist ffmpeg-next worker error");
                                }
                            }
                        }

                        return;
                    }

                    if supports_remux_output_format(&output_format) {
                        let source_path_for_worker = source_path.clone();
                        let output_path_for_worker = output_path.clone();
                        let remux_result = tokio::task::spawn_blocking(move || {
                            remux_media(&source_path_for_worker, &output_path_for_worker)
                        })
                        .await;

                        match remux_result {
                            Ok(Ok(())) => {
                                progress.mark_complete();
                                progress.push_log("ffmpeg-next remux completed".to_owned());
                                let result_json = progress.to_success_payload();
                                if let Err(error) = operation.mark_succeeded(&result_json).await {
                                    warn!(task_id = %operation_id, error = %error, "failed to persist ffmpeg-next remux success");
                                }
                                info!(task_id = %operation_id, output_path = %output_path, "ffmpeg-next remux succeeded");
                                return;
                            }
                            Ok(Err(error)) => {
                                progress.push_log(format!("ffmpeg-next remux failed: {error}"));
                                let payload = progress.to_payload();
                                let error_text = error.to_string();
                                if let Err(db_error) = operation
                                    .update_status(
                                        TaskStatus::Failed,
                                        Some(&payload),
                                        Some(&error_text),
                                    )
                                    .await
                                {
                                    warn!(task_id = %operation_id, error = %db_error, "failed to persist ffmpeg-next remux error");
                                }
                            }
                            Err(error) => {
                                progress.push_log(format!("ffmpeg-next remux worker failed: {error}"));
                                let payload = progress.to_payload();
                                let error_text = error.to_string();
                                if let Err(db_error) = operation
                                    .update_status(
                                        TaskStatus::Failed,
                                        Some(&payload),
                                        Some(&error_text),
                                    )
                                    .await
                                {
                                    warn!(task_id = %operation_id, error = %db_error, "failed to persist ffmpeg-next remux worker error");
                                }
                            }
                        }

                        return;
                    }
                },
            )
            .await?;

        Ok(AcceptedOperation { operation_id })
    }
}

#[derive(Debug, Clone)]
struct FfmpegProgressState {
    output_path: String,
    current_ms: u64,
    message: Option<String>,
    logs: Vec<String>,
}

impl FfmpegProgressState {
    fn new(output_path: String) -> Self {
        Self {
            output_path,
            current_ms: 0,
            message: Some("Starting FFmpeg".to_owned()),
            logs: Vec::new(),
        }
    }

    fn push_log(&mut self, line: String) {
        let line = line.trim().to_owned();
        if line.is_empty() {
            return;
        }

        self.message = Some(line.clone());
        self.logs.push(line);
        if self.logs.len() > 64 {
            let excess = self.logs.len() - 64;
            self.logs.drain(0..excess);
        }
    }

    fn mark_complete(&mut self) {
        self.current_ms = 1;
        self.message = Some("FFmpeg conversion completed".to_owned());
    }

    fn to_progress(&self) -> TaskProgress {
        TaskProgress {
            label: Some("FFmpeg audio extraction".to_owned()),
            message: self.message.clone(),
            current: self.current_ms,
            total: Some(1),
            unit: Some("ms".to_owned()),
            step: Some(1),
            step_count: Some(1),
            logs: (!self.logs.is_empty()).then_some(self.logs.clone()),
        }
    }

    fn to_payload(&self) -> String {
        serde_json::to_string(&FfmpegProgressPayload { progress: self.to_progress() })
            .unwrap_or_default()
    }

    fn to_success_payload(&self) -> String {
        serde_json::to_string(&FfmpegSuccessPayload {
            output_path: self.output_path.clone(),
            progress: self.to_progress(),
        })
        .unwrap_or_default()
    }
}

async fn publish_ffmpeg_progress(
    operation: &OperationContext,
    progress: &FfmpegProgressState,
) -> Result<(), AppCoreError> {
    let payload = progress.to_payload();
    operation.update_status(TaskStatus::Running, Some(&payload), None).await
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
