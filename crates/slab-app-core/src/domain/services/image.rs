use std::path::{Path, PathBuf};
use std::sync::Arc;

use slab_types::RuntimeBackendId;
use slab_types::diffusion::{
    DiffusionImageBackend, DiffusionImageRequest, DiffusionRequestCommon, GgmlDiffusionImageParams,
};
use slab_types::media::RawImageInput;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::config::default_output_dir_for_settings_path;
use crate::context::WorkerState;
use crate::domain::models::{
    AcceptedOperation, IMAGE_GENERATION_TASK_TYPE, ImageGenerationCommand, ImageGenerationMode,
    ImageGenerationTaskView, TaskResult, TaskStatus,
};
use crate::error::AppCoreError;
use crate::infra::db::{
    ImageGenerationTaskViewRecord, MediaTaskStore, NewImageGenerationTaskRecord, TaskRecord,
};
use crate::infra::rpc::{self, codec};

const IMAGE_BACKEND_ID: &str = "ggml.diffusion";

#[derive(Clone)]
pub struct ImageService {
    state: WorkerState,
}

impl ImageService {
    pub fn new(state: WorkerState) -> Self {
        Self { state }
    }

    pub async fn generate_images(
        &self,
        req: ImageGenerationCommand,
    ) -> Result<AcceptedOperation, AppCoreError> {
        let effective_init_image =
            if req.mode == ImageGenerationMode::Img2Img { req.init_image.clone() } else { None };
        let effective_strength =
            if req.mode == ImageGenerationMode::Img2Img { req.strength } else { None };

        debug!(
            model = %req.model,
            prompt_len = req.prompt.len(),
            n = req.n,
            width = req.width,
            height = req.height,
            mode = ?req.mode,
            "image generation request"
        );

        let generate_image_channel =
            self.state.grpc().generate_image_channel().ok_or_else(|| {
                AppCoreError::BackendNotReady("diffusion gRPC endpoint is not configured".into())
            })?;

        let operation_id = Uuid::new_v4().to_string();
        let output_dir = image_task_dir(&self.output_root(), &operation_id);
        tokio::fs::create_dir_all(&output_dir).await.map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to create image output directory '{}': {error}",
                output_dir.display()
            ))
        })?;

        let reference_image_path = if let Some(image) = effective_init_image.as_ref() {
            let path = output_dir.join("reference.png");
            save_rgb_png(&path, &image.data, image.width, image.height).await?;
            Some(path.to_string_lossy().into_owned())
        } else {
            None
        };

        let input_json = serde_json::json!({
            "model_id": req.model_id,
            "prompt": req.prompt,
            "negative_prompt": req.negative_prompt,
            "n": req.n,
            "width": req.width,
            "height": req.height,
            "model": req.model,
            "mode": req.mode,
            "cfg_scale": req.cfg_scale,
            "guidance": req.guidance,
            "steps": req.steps,
            "seed": req.seed,
            "sample_method": req.sample_method,
            "scheduler": req.scheduler,
            "clip_skip": req.clip_skip,
            "strength": req.strength,
            "eta": req.eta,
            "reference_image_path": reference_image_path,
        })
        .to_string();

        let shared_request = DiffusionImageRequest {
            common: DiffusionRequestCommon {
                prompt: req.prompt.clone(),
                negative_prompt: req.negative_prompt.clone(),
                width: req.width,
                height: req.height,
                init_image: effective_init_image.map(|image| RawImageInput {
                    data: image.data,
                    width: image.width,
                    height: image.height,
                    channels: image.channels.clamp(1, u8::MAX as u32) as u8,
                }),
                options: Default::default(),
            },
            backend: DiffusionImageBackend::Ggml(GgmlDiffusionImageParams {
                count: Some(req.n),
                cfg_scale: req.cfg_scale,
                guidance: req.guidance,
                steps: req.steps,
                seed: req.seed,
                sample_method: req.sample_method.clone(),
                scheduler: req.scheduler.clone(),
                clip_skip: req.clip_skip,
                strength: effective_strength,
                eta: req.eta,
            }),
        };
        let grpc_req = codec::encode_diffusion_image_request(req.model.clone(), &shared_request);

        let now = chrono::Utc::now();
        let task_record = TaskRecord {
            id: operation_id.clone(),
            task_type: IMAGE_GENERATION_TASK_TYPE.to_owned(),
            status: TaskStatus::Running,
            model_id: req.model_id.clone(),
            input_data: Some(input_json.clone()),
            result_data: None,
            error_msg: None,
            core_task_id: None,
            created_at: now,
            updated_at: now,
        };
        let image_task_record = NewImageGenerationTaskRecord {
            task_id: operation_id.clone(),
            backend_id: IMAGE_BACKEND_ID.to_owned(),
            model_id: req.model_id.clone(),
            model_path: req.model.clone(),
            prompt: req.prompt.clone(),
            negative_prompt: req.negative_prompt.clone(),
            mode: match req.mode {
                ImageGenerationMode::Txt2Img => "txt2img".to_owned(),
                ImageGenerationMode::Img2Img => "img2img".to_owned(),
            },
            width: req.width,
            height: req.height,
            requested_count: req.n,
            reference_image_path: reference_image_path.clone(),
            request_data: input_json,
            created_at: now,
            updated_at: now,
        };
        if let Err(error) = self
            .state
            .store()
            .insert_image_generation_operation(task_record, image_task_record)
            .await
        {
            tokio::fs::remove_dir_all(&output_dir).await.ok();
            return Err(error.into());
        }

        let worker_state = self.state.clone();
        let model_auto_unload = Arc::clone(self.state.auto_unload());
        let runtime_status = Arc::clone(self.state.runtime_status());
        let store = Arc::clone(self.state.store());
        let generate_image_channel_for_spawn = generate_image_channel;
        let output_root = self.output_root();
        worker_state.spawn_existing_operation(operation_id.clone(), move |operation| async move {
            let operation_id = operation.id().to_owned();
            let task_output_dir = image_task_dir(&output_root, &operation_id);
            let _usage_guard = match model_auto_unload
                .acquire_for_inference(RuntimeBackendId::GgmlDiffusion)
                .await
            {
                Ok(guard) => guard,
                Err(error) => {
                    let message = format!("diffusion backend not ready: {error}");
                    if let Err(db_error) = operation.mark_failed(&message).await {
                        warn!(task_id = %operation_id, error = %db_error, "failed to update auto-reload failure");
                    }
                    return;
                }
            };

            let rpc_result =
                rpc::client::generate_image(generate_image_channel_for_spawn, grpc_req).await;
            if operation.is_cancelled().await {
                cleanup_dir(&task_output_dir).await;
                return;
            }

            match rpc_result {
                Ok(response) => {
                    let payload = match codec::decode_diffusion_image_response(&response) {
                        Ok(value) => value,
                        Err(error) => {
                            let message =
                                format!("invalid diffusion image response payload: {error}");
                            debug!(task_id = %operation_id, error = %error, "failed to decode image response from backend");
                            cleanup_dir(&task_output_dir).await;
                            if let Err(db_error) = operation.mark_failed(&message).await {
                                warn!(task_id = %operation_id, error = %db_error, "failed to update task status after image decode error");
                            }
                            return;
                        }
                    };

                    if payload.images.is_empty() {
                        let message = "diffusion returned no images".to_owned();
                        cleanup_dir(&task_output_dir).await;
                        if let Err(db_error) = operation.mark_failed(&message).await {
                            warn!(task_id = %operation_id, error = %db_error, "failed to update image result after empty payload");
                        }
                        return;
                    }

                    let mut artifact_paths = Vec::with_capacity(payload.images.len());
                    for (index, image) in payload.images.iter().enumerate() {
                        let path = task_output_dir.join(format!("image_{index:03}.png"));
                        if let Err(error) = tokio::fs::write(&path, &image.bytes).await {
                            let message = format!("failed to write generated image artifact: {error}");
                            cleanup_dir(&task_output_dir).await;
                            if let Err(db_error) = operation.mark_failed(&message).await {
                                warn!(task_id = %operation_id, error = %db_error, "failed to persist image artifact write failure");
                            }
                            return;
                        }
                        artifact_paths.push(path.to_string_lossy().into_owned());
                    }

                    let primary_image_path = artifact_paths.first().cloned();
                    let persisted_result = serde_json::json!({
                        "primary_image_path": primary_image_path,
                        "artifact_paths": artifact_paths,
                    })
                    .to_string();
                    let task_result = TaskResult {
                        image: Some(format!("/v1/images/generations/{operation_id}/artifacts/0")),
                        images: Some(
                            (0..artifact_paths.len())
                                .map(|index| format!("/v1/images/generations/{operation_id}/artifacts/{index}"))
                                .collect(),
                        ),
                        video_path: None,
                        output_path: None,
                        text: None,
                        segments: None,
                    };
                    let task_payload = serde_json::to_string(&task_result).unwrap_or_default();

                    if let Err(error) = store
                        .update_image_generation_result(
                            &operation_id,
                            &artifact_paths,
                            primary_image_path.as_deref(),
                            Some(&persisted_result),
                        )
                        .await
                    {
                        cleanup_dir(&task_output_dir).await;
                        let message = format!("failed to persist image generation metadata: {error}");
                        if let Err(db_error) = operation.mark_failed(&message).await {
                            warn!(task_id = %operation_id, error = %db_error, "failed to persist image metadata failure");
                        }
                        return;
                    }

                    if let Err(error) = operation.mark_succeeded(&task_payload).await {
                        warn!(task_id = %operation_id, error = %error, "failed to update image result");
                    }
                }
                Err(error) => {
                    let runtime_snapshot = runtime_status.snapshot(RuntimeBackendId::GgmlDiffusion);
                    let runtime_status = runtime_snapshot.compact_summary();
                    let transport_disconnect = rpc::client::transient_runtime_detail(&error);
                    let message = format!("{error:#}\nruntime_status: {runtime_status}");
                    cleanup_dir(&task_output_dir).await;
                    warn!(
                        task_id = %operation_id,
                        error = %message,
                        runtime_status = %runtime_status,
                        transport_disconnect = transport_disconnect.is_some(),
                        transport_detail = %transport_disconnect.as_deref().unwrap_or(""),
                        "image generation task failed"
                    );
                    if let Err(db_error) = operation.mark_failed(&message).await {
                        warn!(task_id = %operation_id, error = %db_error, "failed to update image failure");
                    }
                }
            }
        });

        Ok(AcceptedOperation { operation_id })
    }

    pub async fn list_generation_tasks(
        &self,
    ) -> Result<Vec<ImageGenerationTaskView>, AppCoreError> {
        let rows = self.state.store().list_image_generation_tasks().await?;
        Ok(rows.into_iter().map(map_image_view).collect())
    }

    pub async fn get_generation_task(
        &self,
        task_id: &str,
    ) -> Result<ImageGenerationTaskView, AppCoreError> {
        let row =
            self.state.store().get_image_generation_task(task_id).await?.ok_or_else(|| {
                AppCoreError::NotFound(format!("image generation task {task_id} not found"))
            })?;
        Ok(map_image_view(row))
    }

    pub async fn read_generated_artifact(
        &self,
        task_id: &str,
        index: usize,
    ) -> Result<Vec<u8>, AppCoreError> {
        let row =
            self.state.store().get_image_generation_task(task_id).await?.ok_or_else(|| {
                AppCoreError::NotFound(format!("image generation task {task_id} not found"))
            })?;
        let Some(path) = row.task.artifact_paths.get(index) else {
            return Err(AppCoreError::NotFound(format!(
                "image artifact {index} for task {task_id} not found"
            )));
        };
        read_managed_file(path, &self.output_root()).await
    }

    pub async fn read_reference_image(&self, task_id: &str) -> Result<Vec<u8>, AppCoreError> {
        let row =
            self.state.store().get_image_generation_task(task_id).await?.ok_or_else(|| {
                AppCoreError::NotFound(format!("image generation task {task_id} not found"))
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

fn map_image_view(row: ImageGenerationTaskViewRecord) -> ImageGenerationTaskView {
    let primary_image_url = row
        .task
        .primary_image_path
        .as_ref()
        .and_then(|primary| {
            row.task.artifact_paths.iter().position(|path| path == primary).map(|index| {
                format!("/v1/images/generations/{}/artifacts/{index}", row.task.task_id)
            })
        })
        .or_else(|| {
            (!row.task.artifact_paths.is_empty())
                .then(|| format!("/v1/images/generations/{}/artifacts/0", row.task.task_id))
        });

    ImageGenerationTaskView {
        task_id: row.task.task_id.clone(),
        task_type: IMAGE_GENERATION_TASK_TYPE.to_owned(),
        status: row.state.status,
        progress: row.state.progress,
        error_msg: row.state.error_msg,
        backend_id: row.task.backend_id,
        model_id: row.task.model_id,
        model_path: row.task.model_path,
        prompt: row.task.prompt,
        negative_prompt: row.task.negative_prompt,
        mode: row.task.mode,
        width: row.task.width,
        height: row.task.height,
        requested_count: row.task.requested_count,
        reference_image_url: row
            .task
            .reference_image_path
            .as_ref()
            .map(|_| format!("/v1/images/generations/{}/reference", row.task.task_id)),
        primary_image_url,
        image_urls: row
            .task
            .artifact_paths
            .iter()
            .enumerate()
            .map(|(index, _)| {
                format!("/v1/images/generations/{}/artifacts/{index}", row.task.task_id)
            })
            .collect(),
        request_data: parse_json_value(&row.task.request_data),
        result_data: row.task.result_data.as_deref().map(parse_json_value),
        created_at: row.state.task_created_at.to_rfc3339(),
        updated_at: row.state.task_updated_at.to_rfc3339(),
    }
}

fn parse_json_value(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_owned()))
}

fn image_task_dir(output_root: &Path, task_id: &str) -> PathBuf {
    output_root.join("images").join(task_id)
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
