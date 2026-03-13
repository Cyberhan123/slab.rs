use std::sync::Arc;

use serde::Serialize;
use tracing::{debug, warn};

use crate::context::{SubmitOperation, WorkerState};
use crate::domain::models::{AcceptedOperation, TaskResult};
use crate::error::ServerError;
use crate::infra::rpc::{self, pb};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ImageGenerationMode {
    Txt2Img,
    Img2Img,
}

#[derive(Debug, Clone)]
pub struct DecodedImageInput {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub channels: u32,
}

#[derive(Debug, Clone)]
pub struct ImageGenerationCommand {
    pub model: String,
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub n: u32,
    pub width: u32,
    pub height: u32,
    pub cfg_scale: Option<f32>,
    pub guidance: Option<f32>,
    pub steps: Option<i32>,
    pub seed: Option<i64>,
    pub sample_method: Option<String>,
    pub scheduler: Option<String>,
    pub clip_skip: Option<i32>,
    pub eta: Option<f32>,
    pub strength: Option<f32>,
    pub init_image: Option<DecodedImageInput>,
    pub mode: ImageGenerationMode,
}

#[derive(Clone)]
pub struct ImagesService {
    state: WorkerState,
}

impl ImagesService {
    pub fn new(state: WorkerState) -> Self {
        Self { state }
    }

    pub async fn generate_images(
        &self,
        req: ImageGenerationCommand,
    ) -> Result<AcceptedOperation, ServerError> {
        let effective_init_image = if req.mode == ImageGenerationMode::Img2Img {
            req.init_image.clone()
        } else {
            None
        };
        let effective_strength = if req.mode == ImageGenerationMode::Img2Img {
            req.strength
        } else {
            None
        };

        let (init_image_bytes, init_image_width, init_image_height, init_image_channels) =
            if let Some(image) = effective_init_image {
                (image.data, image.width, image.height, image.channels)
            } else {
                (Vec::new(), 0u32, 0u32, 3u32)
            };

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
                ServerError::BackendNotReady("diffusion gRPC endpoint is not configured".into())
            })?;

        let input_json = serde_json::json!({
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
        })
        .to_string();

        let grpc_req = pb::ImageRequest {
            model: req.model.clone(),
            prompt: req.prompt.clone(),
            negative_prompt: req.negative_prompt.clone().unwrap_or_default(),
            n: req.n,
            width: req.width,
            height: req.height,
            cfg_scale: req.cfg_scale.unwrap_or(7.0),
            guidance: req.guidance.unwrap_or(3.5),
            sample_steps: req.steps.unwrap_or(20),
            seed: req.seed.unwrap_or(42),
            sample_method: req.sample_method.clone().unwrap_or_default(),
            scheduler: req.scheduler.clone().unwrap_or_default(),
            clip_skip: req.clip_skip.unwrap_or(0),
            strength: effective_strength.unwrap_or(0.75),
            eta: req.eta.unwrap_or(0.0),
            init_image_data: init_image_bytes,
            init_image_width,
            init_image_height,
            init_image_channels,
        };

        let model_auto_unload = Arc::clone(self.state.auto_unload());
        let generate_image_channel_for_spawn = generate_image_channel;
        let operation_id = self
            .state
            .submit_operation(
                SubmitOperation::running("ggml.diffusion", None, Some(input_json)),
                move |operation| async move {
                    let operation_id = operation.id().to_owned();
                    let _usage_guard =
                        match model_auto_unload.acquire_for_inference("ggml.diffusion").await {
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
                        return;
                    }

                    match rpc_result {
                        Ok(images_json) => {
                            let images: Vec<serde_json::Value> =
                                match serde_json::from_slice(&images_json) {
                                    Ok(value) => value,
                                    Err(error) => {
                                        let message =
                                            format!("invalid JSON from diffusion backend: {error}");
                                        debug!(task_id = %operation_id, error = %error, "failed to parse image JSON from backend");
                                        if let Err(db_error) = operation.mark_failed(&message).await {
                                            warn!(task_id = %operation_id, error = %db_error, "failed to update task status after JSON parse error");
                                        }
                                        return;
                                    }
                                };

                            let data_uris: Vec<String> = images
                                .iter()
                                .filter_map(|image| image["b64"].as_str())
                                .map(|b64| format!("data:image/png;base64,{b64}"))
                                .collect();

                            let result = TaskResult {
                                image: data_uris.first().cloned(),
                                images: Some(data_uris),
                                video_path: None,
                                text: None,
                            };
                            let payload_str = serde_json::to_string(&result).unwrap_or_default();
                            if let Err(error) = operation.mark_succeeded(&payload_str).await {
                                warn!(task_id = %operation_id, error = %error, "failed to update image result");
                            }
                        }
                        Err(error) => {
                            let message = error.to_string();
                            if let Err(db_error) = operation.mark_failed(&message).await {
                                warn!(task_id = %operation_id, error = %db_error, "failed to update image failure");
                            }
                        }
                    }
                },
            )
            .await?;

        Ok(AcceptedOperation { operation_id })
    }
}
