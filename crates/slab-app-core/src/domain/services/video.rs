use std::path::{Path, PathBuf};
use std::sync::Arc;

use slab_types::RuntimeBackendId;
use slab_types::diffusion::{
    DiffusionRequestCommon, DiffusionVideoBackend, DiffusionVideoRequest, GgmlDiffusionVideoParams,
};
use slab_types::media::RawImageInput;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::config::default_output_dir_for_settings_path;
use crate::context::WorkerState;
use crate::domain::models::{
    AcceptedOperation, TaskResult, TaskStatus, VIDEO_GENERATION_TASK_TYPE, VideoGenerationCommand,
    VideoGenerationTaskView,
};
use crate::error::AppCoreError;
use crate::infra::db::{
    MediaTaskStore, NewVideoGenerationTaskRecord, TaskRecord, VideoGenerationTaskViewRecord,
};
use crate::infra::rpc::{self, codec};

const VIDEO_BACKEND_ID: &str = "ggml.diffusion";

#[derive(Clone)]
pub struct VideoService {
    state: WorkerState,
}

impl VideoService {
    pub fn new(state: WorkerState) -> Self {
        Self { state }
    }

    pub async fn generate_video(
        &self,
        req: VideoGenerationCommand,
    ) -> Result<AcceptedOperation, AppCoreError> {
        debug!(
            model = %req.model,
            prompt_len = req.prompt.len(),
            frames = req.video_frames,
            "video generation request"
        );

        let generate_image_channel =
            self.state.grpc().generate_image_channel().ok_or_else(|| {
                AppCoreError::BackendNotReady("diffusion gRPC endpoint is not configured".into())
            })?;

        let operation_id = Uuid::new_v4().to_string();
        let output_dir = video_task_dir(&self.output_root(), &operation_id);
        tokio::fs::create_dir_all(&output_dir).await.map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to create video output directory '{}': {error}",
                output_dir.display()
            ))
        })?;

        let reference_image_path = if let Some(image) = req.init_image.as_ref() {
            let path = output_dir.join("reference.png");
            save_rgb_png(&path, &image.data, image.width, image.height).await?;
            Some(path.to_string_lossy().into_owned())
        } else {
            None
        };

        let input_json = serde_json::json!({
            "model_id": req.model_id,
            "model": req.model,
            "prompt": req.prompt,
            "negative_prompt": req.negative_prompt,
            "width": req.width,
            "height": req.height,
            "video_frames": req.video_frames,
            "fps": req.fps,
            "cfg_scale": req.cfg_scale,
            "guidance": req.guidance,
            "steps": req.steps,
            "seed": req.seed,
            "sample_method": req.sample_method,
            "scheduler": req.scheduler,
            "strength": req.strength,
            "reference_image_path": reference_image_path,
        })
        .to_string();

        let fps = req.fps;
        let grpc_request = DiffusionVideoRequest {
            common: DiffusionRequestCommon {
                prompt: req.prompt.clone(),
                negative_prompt: req.negative_prompt.clone(),
                width: req.width,
                height: req.height,
                init_image: req.init_image.map(|image| RawImageInput {
                    data: image.data,
                    width: image.width,
                    height: image.height,
                    channels: image.channels.clamp(1, u8::MAX as u32) as u8,
                }),
                options: Default::default(),
            },
            backend: DiffusionVideoBackend::Ggml(GgmlDiffusionVideoParams {
                video_frames: Some(req.video_frames),
                fps: Some(req.fps),
                cfg_scale: req.cfg_scale,
                guidance: req.guidance,
                steps: req.steps,
                seed: req.seed,
                sample_method: req.sample_method.clone(),
                scheduler: req.scheduler.clone(),
                strength: req.strength,
            }),
        };
        let grpc_req = codec::encode_diffusion_video_request(req.model.clone(), &grpc_request);

        let now = chrono::Utc::now();
        let task_record = TaskRecord {
            id: operation_id.clone(),
            task_type: VIDEO_GENERATION_TASK_TYPE.to_owned(),
            status: TaskStatus::Running,
            model_id: req.model_id.clone(),
            input_data: Some(input_json.clone()),
            result_data: None,
            error_msg: None,
            core_task_id: None,
            created_at: now,
            updated_at: now,
        };
        let video_task_record = NewVideoGenerationTaskRecord {
            task_id: operation_id.clone(),
            backend_id: VIDEO_BACKEND_ID.to_owned(),
            model_id: req.model_id.clone(),
            model_path: req.model.clone(),
            prompt: req.prompt.clone(),
            negative_prompt: req.negative_prompt.clone(),
            width: req.width,
            height: req.height,
            frames: req.video_frames,
            fps: req.fps,
            reference_image_path: reference_image_path.clone(),
            request_data: input_json,
            created_at: now,
            updated_at: now,
        };
        if let Err(error) = self
            .state
            .store()
            .insert_video_generation_operation(task_record, video_task_record)
            .await
        {
            tokio::fs::remove_dir_all(&output_dir).await.ok();
            return Err(error.into());
        }

        let model_auto_unload = Arc::clone(self.state.auto_unload());
        let store = Arc::clone(self.state.store());
        let generate_image_channel_for_spawn = generate_image_channel;
        let output_root = self.output_root();
        self.state
            .clone()
            .spawn_existing_operation(operation_id.clone(), move |operation| async move {
                let operation_id = operation.id().to_owned();
                let task_output_dir = video_task_dir(&output_root, &operation_id);

                let _usage_guard = match model_auto_unload
                    .acquire_for_inference(RuntimeBackendId::GgmlDiffusion)
                    .await
                {
                    Ok(guard) => guard,
                    Err(error) => {
                        let message = format!("diffusion backend not ready: {error}");
                        if let Err(db_error) = operation.mark_failed(&message).await {
                            warn!(task_id = %operation_id, error = %db_error, "failed to persist backend-not-ready error");
                        }
                        return;
                    }
                };

                let rpc_result = rpc::client::generate_video(generate_image_channel_for_spawn, grpc_req).await;
                if operation.is_cancelled().await {
                    cleanup_dir(&task_output_dir).await;
                    return;
                }

                let response = match rpc_result {
                    Ok(payload) => payload,
                    Err(error) => {
                        cleanup_dir(&task_output_dir).await;
                        if let Err(db_error) = operation.mark_failed(&error.to_string()).await {
                            warn!(task_id = %operation_id, error = %db_error, "failed to persist diffusion video error");
                        }
                        return;
                    }
                };

                let frames = match codec::decode_diffusion_video_response(&response) {
                    Ok(value) => value.frames,
                    Err(error) => {
                        let message = format!("failed to decode frames from diffusion backend: {error}");
                        cleanup_dir(&task_output_dir).await;
                        if let Err(db_error) = operation.mark_failed(&message).await {
                            warn!(task_id = %operation_id, error = %db_error, "failed to persist frame decode error");
                        }
                        return;
                    }
                };

                if frames.is_empty() {
                    cleanup_dir(&task_output_dir).await;
                    if let Err(db_error) = operation.mark_failed("diffusion returned no frames").await {
                        warn!(task_id = %operation_id, error = %db_error, "failed to persist empty-frame error");
                    }
                    return;
                }

                let frame_dir = task_output_dir.join("frames");
                if let Err(error) = tokio::fs::create_dir_all(&frame_dir).await {
                    let message = format!("failed to create frame dir: {error}");
                    cleanup_dir(&task_output_dir).await;
                    if let Err(db_error) = operation.mark_failed(&message).await {
                        warn!(task_id = %operation_id, error = %db_error, "failed to persist frame-dir error");
                    }
                    return;
                }

                let mut written_index = 0usize;
                for (source_index, frame) in frames.iter().enumerate() {
                    let image = if frame.channels == 3 {
                        image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(
                            frame.width,
                            frame.height,
                            frame.data.clone(),
                        )
                        .map(image::DynamicImage::ImageRgb8)
                    } else {
                        image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
                            frame.width,
                            frame.height,
                            frame.data.clone(),
                        )
                        .map(image::DynamicImage::ImageRgba8)
                    };

                    let Some(image) = image else {
                        warn!(task_id = %operation_id, source_frame = source_index, written = written_index, "failed to construct image from raw pixels; skipping");
                        continue;
                    };

                    let frame_path = frame_dir.join(format!("frame_{written_index:05}.png"));
                    if let Err(error) = image.save(&frame_path) {
                        warn!(task_id = %operation_id, source_frame = source_index, written = written_index, error = %error, "failed to save frame PNG; skipping");
                        continue;
                    }
                    written_index += 1;
                }

                if written_index == 0 {
                    cleanup_dir(&task_output_dir).await;
                    if let Err(db_error) = operation.mark_failed("no valid frames written").await {
                        warn!(task_id = %operation_id, error = %db_error, "failed to persist no-valid-frame error");
                    }
                    return;
                }

                let output_path = task_output_dir.join("output.mp4");
                let frame_pattern = frame_dir.join("frame_%05d.png");
                let ffmpeg_bin = ffmpeg_sidecar::paths::ffmpeg_path();
                let ffmpeg_result = tokio::process::Command::new(&ffmpeg_bin)
                    .arg("-y")
                    .arg("-framerate")
                    .arg(fps.to_string())
                    .arg("-i")
                    .arg(&frame_pattern)
                    .arg("-c:v")
                    .arg("libx264")
                    .arg("-pix_fmt")
                    .arg("yuv420p")
                    .arg(&output_path)
                    .output()
                    .await;

                tokio::fs::remove_dir_all(&frame_dir).await.ok();

                match ffmpeg_result {
                    Ok(output) if output.status.success() => {
                        let video_path = output_path.to_string_lossy().into_owned();
                        info!(task_id = %operation_id, video_path = %video_path, "video generation succeeded");
                        let persisted_result =
                            serde_json::json!({ "video_path": video_path }).to_string();
                        let task_result = TaskResult {
                            image: None,
                            images: None,
                            video_path: Some(format!("/v1/video/generations/{operation_id}/artifact")),
                            output_path: None,
                            text: None,
                            segments: None,
                        };
                        let task_payload = serde_json::to_string(&task_result).unwrap_or_default();
                        if let Err(db_error) = store
                            .update_video_generation_result(
                                &operation_id,
                                Some(&video_path),
                                Some(&persisted_result),
                            )
                            .await
                        {
                            cleanup_dir(&task_output_dir).await;
                            let message =
                                format!("failed to persist video task metadata: {db_error}");
                            if let Err(mark_error) = operation.mark_failed(&message).await {
                                warn!(task_id = %operation_id, error = %mark_error, "failed to persist video metadata failure");
                            }
                            return;
                        }
                        if let Err(db_error) = operation.mark_succeeded(&task_payload).await {
                            warn!(task_id = %operation_id, error = %db_error, "failed to persist video task success");
                        }
                    }
                    Ok(output) => {
                        cleanup_dir(&task_output_dir).await;
                        let error = String::from_utf8_lossy(&output.stderr).to_string();
                        warn!(task_id = %operation_id, ffmpeg_bin = %ffmpeg_bin.display(), error = %error, "ffmpeg failed");
                        if let Err(db_error) = operation.mark_failed(&error).await {
                            warn!(task_id = %operation_id, error = %db_error, "failed to persist ffmpeg failure");
                        }
                    }
                    Err(error) => {
                        cleanup_dir(&task_output_dir).await;
                        warn!(task_id = %operation_id, ffmpeg_bin = %ffmpeg_bin.display(), error = %error, "ffmpeg spawn failed");
                        if let Err(db_error) = operation.mark_failed(&error.to_string()).await {
                            warn!(task_id = %operation_id, error = %db_error, "failed to persist ffmpeg spawn failure");
                        }
                    }
                }
            });

        Ok(AcceptedOperation { operation_id })
    }

    pub async fn list_generation_tasks(
        &self,
    ) -> Result<Vec<VideoGenerationTaskView>, AppCoreError> {
        let rows = self.state.store().list_video_generation_tasks().await?;
        Ok(rows.into_iter().map(map_video_view).collect())
    }

    pub async fn get_generation_task(
        &self,
        task_id: &str,
    ) -> Result<VideoGenerationTaskView, AppCoreError> {
        let row =
            self.state.store().get_video_generation_task(task_id).await?.ok_or_else(|| {
                AppCoreError::NotFound(format!("video generation task {task_id} not found"))
            })?;
        Ok(map_video_view(row))
    }

    pub async fn read_generated_video(&self, task_id: &str) -> Result<Vec<u8>, AppCoreError> {
        let row =
            self.state.store().get_video_generation_task(task_id).await?.ok_or_else(|| {
                AppCoreError::NotFound(format!("video generation task {task_id} not found"))
            })?;
        let path = row.task.video_path.ok_or_else(|| {
            AppCoreError::NotFound(format!("video artifact for task {task_id} not found"))
        })?;
        read_managed_file(&path, &self.output_root()).await
    }

    pub async fn read_reference_image(&self, task_id: &str) -> Result<Vec<u8>, AppCoreError> {
        let row =
            self.state.store().get_video_generation_task(task_id).await?.ok_or_else(|| {
                AppCoreError::NotFound(format!("video generation task {task_id} not found"))
            })?;
        let path = row.task.reference_image_path.ok_or_else(|| {
            AppCoreError::NotFound(format!("reference image for task {task_id} not found"))
        })?;
        read_managed_file(&path, &self.output_root()).await
    }

    fn output_root(&self) -> PathBuf {
        default_output_dir_for_settings_path(&self.state.config().settings_path)
    }
}

fn map_video_view(row: VideoGenerationTaskViewRecord) -> VideoGenerationTaskView {
    VideoGenerationTaskView {
        task_id: row.task.task_id.clone(),
        task_type: VIDEO_GENERATION_TASK_TYPE.to_owned(),
        status: row.state.status,
        progress: row.state.progress,
        error_msg: row.state.error_msg,
        backend_id: row.task.backend_id,
        model_id: row.task.model_id,
        model_path: row.task.model_path,
        prompt: row.task.prompt,
        negative_prompt: row.task.negative_prompt,
        width: row.task.width,
        height: row.task.height,
        frames: row.task.frames,
        fps: row.task.fps,
        reference_image_url: row
            .task
            .reference_image_path
            .as_ref()
            .map(|_| format!("/v1/video/generations/{}/reference", row.task.task_id)),
        video_url: row
            .task
            .video_path
            .as_ref()
            .map(|_| format!("/v1/video/generations/{}/artifact", row.task.task_id)),
        request_data: parse_json_value(&row.task.request_data),
        result_data: row.task.result_data.as_deref().map(parse_json_value),
        created_at: row.state.task_created_at.to_rfc3339(),
        updated_at: row.state.task_updated_at.to_rfc3339(),
    }
}

fn parse_json_value(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_owned()))
}

fn video_task_dir(output_root: &Path, task_id: &str) -> PathBuf {
    output_root.join("videos").join(task_id)
}

async fn save_rgb_png(
    path: &Path,
    data: &[u8],
    width: u32,
    height: u32,
) -> Result<(), AppCoreError> {
    let path = path.to_path_buf();
    let error_path = path.clone();
    let bytes = data.to_vec();
    tokio::task::spawn_blocking(move || {
        image::save_buffer_with_format(
            &path,
            &bytes,
            width,
            height,
            image::ColorType::Rgb8,
            image::ImageFormat::Png,
        )
    })
    .await
    .map_err(|error| AppCoreError::Internal(format!("reference image task panicked: {error}")))?
    .map_err(|error| {
        AppCoreError::Internal(format!("failed to save PNG '{}': {error}", error_path.display()))
    })
}

async fn read_managed_file(path: &str, output_root: &Path) -> Result<Vec<u8>, AppCoreError> {
    let candidate = PathBuf::from(path);
    if !candidate.starts_with(output_root) {
        return Err(AppCoreError::BadRequest("artifact path escapes output root".to_owned()));
    }
    tokio::fs::read(&candidate).await.map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => {
            AppCoreError::NotFound(format!("artifact '{}' not found", candidate.display()))
        }
        _ => AppCoreError::Internal(format!(
            "failed to read artifact '{}': {error}",
            candidate.display()
        )),
    })
}

async fn cleanup_dir(path: &Path) {
    tokio::fs::remove_dir_all(path).await.ok();
}
