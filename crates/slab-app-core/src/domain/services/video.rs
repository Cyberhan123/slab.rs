use std::sync::Arc;

use slab_proto::convert;
use slab_types::RuntimeBackendId;
use slab_types::diffusion::{
    DiffusionRequestCommon, DiffusionVideoBackend, DiffusionVideoRequest, GgmlDiffusionVideoParams,
};
use slab_types::media::RawImageInput;
use tracing::{debug, info, warn};

use crate::context::{SubmitOperation, WorkerState};
use crate::domain::models::{AcceptedOperation, TaskResult, VideoGenerationCommand};
use crate::error::AppCoreError;
use crate::infra::rpc::{self, pb};

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

        let fps = req.fps;
        let input_json = serde_json::json!({
            "model": req.model,
            "prompt": req.prompt,
            "negative_prompt": req.negative_prompt,
            "width": req.width,
            "height": req.height,
            "video_frames": req.video_frames,
            "fps": fps,
        })
        .to_string();

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
        let grpc_req = convert::encode_diffusion_video_request(req.model.clone(), &grpc_request);

        let model_auto_unload = Arc::clone(self.state.auto_unload());
        let operation_id = self
            .state
            .submit_operation(
                SubmitOperation::running("ggml.diffusion.video", None, Some(input_json)),
                move |operation| async move {
                    let operation_id = operation.id().to_owned();

                    let _usage_guard =
                        match model_auto_unload
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

                    let rpc_result =
                        rpc::client::generate_video(generate_image_channel, grpc_req).await;
                    if operation.is_cancelled().await {
                        return;
                    }

                    let response = match rpc_result {
                        Ok(payload) => payload,
                        Err(error) => {
                            if let Err(db_error) = operation.mark_failed(&error.to_string()).await {
                                warn!(task_id = %operation_id, error = %db_error, "failed to persist diffusion video error");
                            }
                            return;
                        }
                    };

                    let frames = match convert::decode_diffusion_video_response(
                        &pb::VideoResponse {
                            frames_json: response,
                        },
                    ) {
                        Ok(value) => value.frames,
                        Err(error) => {
                            let message =
                                format!("failed to decode frames from diffusion backend: {error}");
                            if let Err(db_error) = operation.mark_failed(&message).await {
                                warn!(task_id = %operation_id, error = %db_error, "failed to persist frame decode error");
                            }
                            return;
                        }
                    };

                    if frames.is_empty() {
                        if let Err(db_error) =
                            operation.mark_failed("diffusion returned no frames").await
                        {
                            warn!(task_id = %operation_id, error = %db_error, "failed to persist empty-frame error");
                        }
                        return;
                    }

                    let frame_dir = std::env::temp_dir().join(format!("slab-video-{operation_id}"));
                    if let Err(error) = tokio::fs::create_dir_all(&frame_dir).await {
                        let message = format!("failed to create frame dir: {error}");
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
                        if let Err(db_error) = operation.mark_failed("no valid frames written").await
                        {
                            warn!(task_id = %operation_id, error = %db_error, "failed to persist no-valid-frame error");
                        }
                        return;
                    }

                    let output_path =
                        std::env::temp_dir().join(format!("slab-video-{operation_id}.mp4"));
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
                            let result = TaskResult {
                                image: None,
                                images: None,
                                video_path: Some(video_path),
                                text: None,
                            };
                            let payload = serde_json::to_string(&result).unwrap_or_default();
                            if let Err(db_error) = operation.mark_succeeded(&payload).await {
                                warn!(task_id = %operation_id, error = %db_error, "failed to persist video task success");
                            }
                        }
                        Ok(output) => {
                            let error = String::from_utf8_lossy(&output.stderr).to_string();
                            warn!(task_id = %operation_id, ffmpeg_bin = %ffmpeg_bin.display(), error = %error, "ffmpeg failed");
                            if let Err(db_error) = operation.mark_failed(&error).await {
                                warn!(task_id = %operation_id, error = %db_error, "failed to persist ffmpeg failure");
                            }
                        }
                        Err(error) => {
                            warn!(task_id = %operation_id, ffmpeg_bin = %ffmpeg_bin.display(), error = %error, "ffmpeg spawn failed");
                            if let Err(db_error) = operation.mark_failed(&error.to_string()).await {
                                warn!(task_id = %operation_id, error = %db_error, "failed to persist ffmpeg spawn failure");
                            }
                        }
                    }
                },
            )
            .await?;

        Ok(AcceptedOperation { operation_id })
    }
}
