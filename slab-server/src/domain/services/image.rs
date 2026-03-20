use std::sync::Arc;

use base64::Engine as _;
use slab_proto::convert;
use slab_types::diffusion::DiffusionImageRequest;
use slab_types::media::RawImageInput;
use tracing::{debug, warn};

use crate::context::{SubmitOperation, WorkerState};
use crate::domain::models::{
    AcceptedOperation, ImageGenerationCommand, ImageGenerationMode, TaskResult,
};
use crate::error::ServerError;
use crate::infra::rpc::{self, pb};

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
    ) -> Result<AcceptedOperation, ServerError> {
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

        let shared_request = DiffusionImageRequest {
            prompt: req.prompt.clone(),
            negative_prompt: req.negative_prompt.clone(),
            count: req.n,
            width: req.width,
            height: req.height,
            cfg_scale: req.cfg_scale,
            guidance: req.guidance,
            steps: req.steps,
            seed: req.seed,
            sample_method: req.sample_method.clone(),
            scheduler: req.scheduler.clone(),
            clip_skip: req.clip_skip,
            strength: effective_strength,
            eta: req.eta,
            init_image: effective_init_image.map(|image| RawImageInput {
                data: image.data,
                width: image.width,
                height: image.height,
                channels: image.channels.clamp(1, u8::MAX as u32) as u8,
            }),
            options: Default::default(),
        };
        let grpc_req = convert::encode_diffusion_image_request(req.model.clone(), &shared_request);

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
                            let payload = match convert::decode_diffusion_image_response(
                                &pb::ImageResponse { images_json },
                            ) {
                                Ok(value) => value,
                                Err(error) => {
                                    let message = format!(
                                        "invalid diffusion image response payload: {error}"
                                    );
                                    debug!(task_id = %operation_id, error = %error, "failed to decode image response from backend");
                                    if let Err(db_error) = operation.mark_failed(&message).await {
                                        warn!(task_id = %operation_id, error = %db_error, "failed to update task status after image decode error");
                                    }
                                    return;
                                }
                            };

                            let data_uris: Vec<String> = payload
                                .images
                                .into_iter()
                                .map(|image| {
                                    format!(
                                        "data:image/png;base64,{}",
                                        base64::engine::general_purpose::STANDARD.encode(
                                            image.bytes
                                        )
                                    )
                                })
                                .collect();

                            if data_uris.is_empty() {
                                let message = "diffusion returned no images".to_owned();
                                if let Err(db_error) = operation.mark_failed(&message).await {
                                    warn!(task_id = %operation_id, error = %db_error, "failed to update image result after empty payload");
                                }
                                return;
                            }

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
