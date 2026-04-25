use std::process::Stdio;

use tokio::io::AsyncReadExt;
use tracing::{info, warn};

use crate::context::worker_state::OperationContext;
use crate::context::{SubmitOperation, WorkerState};
use crate::domain::models::{AcceptedOperation, FfmpegConvertCommand, TaskProgress, TaskStatus};
use crate::error::AppCoreError;

const FFMPEG_PROGRESS_LOG_LIMIT: usize = 240;

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
                    let mut progress = FfmpegProgressState::new(output_path.clone());

                    if let Err(error) = publish_ffmpeg_progress(&operation, &progress).await {
                        warn!(task_id = %operation_id, error = %error, "failed to publish initial ffmpeg progress");
                    }

                    let mut command = tokio::process::Command::new(&ffmpeg_bin);
                    command
                        .args(["-y", "-i", &source_path, "-f", &output_format, &output_path])
                        .stdout(Stdio::null())
                        .stderr(Stdio::piped())
                        .kill_on_drop(true);

                    let spawn_result = command.spawn();

                    let mut child = match spawn_result {
                        Ok(child) => child,
                        Err(error) => {
                            progress.push_log(format!(
                                "failed to spawn ffmpeg '{}': {error}",
                                ffmpeg_bin.display()
                            ));
                            let payload = progress.to_payload();
                            warn!(task_id = %operation_id, ffmpeg_bin = %ffmpeg_bin.display(), error = %error, "ffmpeg spawn failed");
                            if let Err(db_error) = operation
                                .update_status(TaskStatus::Failed, Some(&payload), Some(&error.to_string()))
                                .await
                            {
                                warn!(task_id = %operation_id, error = %db_error, "failed to persist ffmpeg spawn error");
                            }
                            return;
                        }
                    };

                    let stderr = child.stderr.take();
                    if let Some(mut stderr) = stderr {
                        let mut buffer = Vec::new();
                        let mut chunk = [0_u8; 8192];
                        loop {
                            match stderr.read(&mut chunk).await {
                                Ok(0) => break,
                                Ok(bytes_read) => {
                                    for byte in &chunk[..bytes_read] {
                                        if *byte == b'\n' || *byte == b'\r' {
                                            publish_ffmpeg_log_line(
                                                &operation,
                                                &mut progress,
                                                &mut buffer,
                                            )
                                            .await;
                                        } else {
                                            buffer.push(*byte);
                                        }
                                    }
                                }
                                Err(error) => {
                                    progress.push_log(format!("failed to read ffmpeg stderr: {error}"));
                                    if let Err(db_error) =
                                        publish_ffmpeg_progress(&operation, &progress).await
                                    {
                                        warn!(task_id = %operation_id, error = %db_error, "failed to publish ffmpeg stderr read error");
                                    }
                                    break;
                                }
                            }
                        }
                        publish_ffmpeg_log_line(&operation, &mut progress, &mut buffer).await;
                    } else {
                        progress.push_log("ffmpeg stderr pipe was not available".to_owned());
                        if let Err(error) = publish_ffmpeg_progress(&operation, &progress).await {
                            warn!(task_id = %operation_id, error = %error, "failed to publish ffmpeg stderr pipe warning");
                        }
                    }

                    match child.wait().await {
                        Ok(status) if status.success() => {
                            progress.mark_complete();
                            progress.push_log(format!("ffmpeg exited successfully with status {status}"));
                            let result_json = progress.to_success_payload();
                            if let Err(error) = operation.mark_succeeded(&result_json).await {
                                warn!(task_id = %operation_id, error = %error, "failed to persist ffmpeg success");
                            }
                            info!(task_id = %operation_id, output_path = %output_path, "ffmpeg conversion succeeded");
                        }
                        Ok(status) => {
                            let error = progress.failure_message(status.to_string());
                            let payload = progress.to_payload();
                            warn!(task_id = %operation_id, error = %error, "ffmpeg conversion failed");
                            if let Err(db_error) = operation
                                .update_status(TaskStatus::Failed, Some(&payload), Some(&error))
                                .await
                            {
                                warn!(task_id = %operation_id, error = %db_error, "failed to persist ffmpeg error");
                            }
                        }
                        Err(error) => {
                            progress.push_log(format!("failed to wait for ffmpeg: {error}"));
                            let payload = progress.to_payload();
                            warn!(task_id = %operation_id, ffmpeg_bin = %ffmpeg_bin.display(), error = %error, "ffmpeg spawn failed");
                            if let Err(db_error) = operation
                                .update_status(TaskStatus::Failed, Some(&payload), Some(&error.to_string()))
                                .await
                            {
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

#[derive(Debug, Clone)]
struct FfmpegProgressState {
    output_path: String,
    current_ms: u64,
    duration_ms: Option<u64>,
    message: Option<String>,
    logs: Vec<String>,
}

impl FfmpegProgressState {
    fn new(output_path: String) -> Self {
        Self {
            output_path,
            current_ms: 0,
            duration_ms: None,
            message: Some("Starting FFmpeg".to_owned()),
            logs: Vec::new(),
        }
    }

    fn push_log(&mut self, line: String) {
        let line = line.trim().to_owned();
        if line.is_empty() {
            return;
        }

        if let Some(duration_ms) = parse_ffmpeg_duration_ms(&line) {
            self.duration_ms = Some(duration_ms);
        }
        if let Some(current_ms) = parse_ffmpeg_time_ms(&line) {
            self.current_ms = match self.duration_ms {
                Some(total) => current_ms.min(total),
                None => current_ms,
            };
        }

        self.message = Some(line.clone());
        self.logs.push(line);
        if self.logs.len() > FFMPEG_PROGRESS_LOG_LIMIT {
            let excess = self.logs.len() - FFMPEG_PROGRESS_LOG_LIMIT;
            self.logs.drain(0..excess);
        }
    }

    fn mark_complete(&mut self) {
        if let Some(duration_ms) = self.duration_ms {
            self.current_ms = duration_ms;
        } else {
            self.current_ms = self.current_ms.max(1);
            self.duration_ms = Some(self.current_ms);
        }
        self.message = Some("FFmpeg conversion completed".to_owned());
    }

    fn to_progress(&self) -> TaskProgress {
        TaskProgress {
            label: Some("FFmpeg audio extraction".to_owned()),
            message: self.message.clone(),
            current: self.current_ms,
            total: self.duration_ms,
            unit: Some("ms".to_owned()),
            step: Some(1),
            step_count: Some(1),
            logs: (!self.logs.is_empty()).then_some(self.logs.clone()),
        }
    }

    fn to_payload(&self) -> String {
        serde_json::json!({
            "progress": self.to_progress(),
        })
        .to_string()
    }

    fn to_success_payload(&self) -> String {
        serde_json::json!({
            "output_path": self.output_path.clone(),
            "progress": self.to_progress(),
        })
        .to_string()
    }

    fn failure_message(&self, status: String) -> String {
        let tail = self.logs.iter().rev().take(8).cloned().collect::<Vec<_>>();
        if tail.is_empty() {
            return format!("ffmpeg exited with status {status}");
        }

        let mut lines = tail;
        lines.reverse();
        format!("ffmpeg exited with status {status}: {}", lines.join("\n"))
    }
}

async fn publish_ffmpeg_log_line(
    operation: &OperationContext,
    progress: &mut FfmpegProgressState,
    buffer: &mut Vec<u8>,
) {
    if buffer.is_empty() {
        return;
    }

    let line = String::from_utf8_lossy(buffer).trim().to_owned();
    buffer.clear();
    if line.is_empty() {
        return;
    }

    progress.push_log(line);
    if let Err(error) = publish_ffmpeg_progress(operation, progress).await {
        warn!(task_id = %operation.id(), error = %error, "failed to publish ffmpeg progress");
    }
}

async fn publish_ffmpeg_progress(
    operation: &OperationContext,
    progress: &FfmpegProgressState,
) -> Result<(), AppCoreError> {
    let payload = progress.to_payload();
    operation.update_status(TaskStatus::Running, Some(&payload), None).await
}

fn parse_ffmpeg_duration_ms(line: &str) -> Option<u64> {
    let (_, after) = line.split_once("Duration:")?;
    let value = after.split(',').next()?.trim();
    parse_ffmpeg_timestamp_ms(value)
}

fn parse_ffmpeg_time_ms(line: &str) -> Option<u64> {
    let start = line.find("time=")? + "time=".len();
    let value = line[start..].trim_start().split_whitespace().next()?;
    parse_ffmpeg_timestamp_ms(value)
}

fn parse_ffmpeg_timestamp_ms(value: &str) -> Option<u64> {
    let mut parts = value.split(':');
    let hours = parts.next()?.parse::<u64>().ok()?;
    let minutes = parts.next()?.parse::<u64>().ok()?;
    let seconds = parts.next()?.parse::<f64>().ok()?;
    if parts.next().is_some() || !seconds.is_finite() || minutes >= 60 {
        return None;
    }

    Some(
        hours
            .saturating_mul(3_600_000)
            .saturating_add(minutes.saturating_mul(60_000))
            .saturating_add((seconds * 1_000.0).round().max(0.0) as u64),
    )
}

#[cfg(test)]
mod test {
    use super::{parse_ffmpeg_duration_ms, parse_ffmpeg_time_ms, parse_ffmpeg_timestamp_ms};

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

    #[test]
    fn parses_ffmpeg_timestamp() {
        assert_eq!(parse_ffmpeg_timestamp_ms("01:02:03.45"), Some(3_723_450));
    }

    #[test]
    fn parses_ffmpeg_duration_line() {
        assert_eq!(
            parse_ffmpeg_duration_ms("  Duration: 00:01:10.12, start: 0.000000, bitrate: 192 kb/s",),
            Some(70_120),
        );
    }

    #[test]
    fn parses_ffmpeg_time_progress_line() {
        assert_eq!(
            parse_ffmpeg_time_ms(
                "size=    256kB time=00:00:09.88 bitrate=212.4kbits/s speed=1.21x",
            ),
            Some(9_880),
        );
    }
}
