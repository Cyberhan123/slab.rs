//! `generate_image` agent tool — lets the model produce images via the existing
//! `ggml.diffusion` backend (`ImageService` → `GrpcGateway` → `slab-runtime`),
//! surfacing each result inline as a harness `TurnItem::ImageView`.
//!
//! Registered alongside [`super::code_tools::CodeLspStatusTool`] in
//! [`super::bootstrap`] because it needs an app-core service (`ImageService`),
//! which `slab-agent-tools` cannot depend on (app-core depends on it).

use std::time::Duration;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use slab_agent::{
    AgentError, ToolCallRender, ToolContext, ToolOutput, TypedTool, protocol::TurnItem,
    tool::default_tool_turn_item, typed_input_schema,
};

use crate::domain::models::{
    ImageGenerationCommand, ImageGenerationMode, ImageGenerationTaskView, ListModelsFilter,
    ModelLoadCommand, TaskStatus, UnifiedModelKind,
};
use crate::domain::services::{ImageService, ModelService};
use crate::error::AppCoreError;
use slab_types::Capability;

/// Poll cadence / ceiling for the fire-and-forget `ImageService` task.
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const MAX_POLL_ATTEMPTS: u32 = 240; // 240 × 500 ms = 120 s

pub(crate) struct GenerateImageTool {
    image_service: ImageService,
    /// Lazy-load seam: when present, the first image-generation call ensures
    /// the local diffusion model is downloaded + loaded BEFORE dispatching to
    /// the runtime (the old flow sent `model_id: None`, skipped the
    /// resident-model check, and failed with `runtime_model_not_loaded`).
    model_service: Option<ModelService>,
}

impl GenerateImageTool {
    pub(crate) fn new(image_service: ImageService) -> Self {
        Self { image_service, model_service: None }
    }

    pub(crate) fn with_model_service(mut self, model_service: ModelService) -> Self {
        self.model_service = Some(model_service);
        self
    }

    /// Ensure a local image-generation model is loaded (idempotent fast path
    /// when it already is). Best-effort: when no local image model exists in
    /// the catalog the generation proceeds and surfaces the runtime's own
    /// not-loaded error (its message quality is part of the contract).
    async fn ensure_image_model_loaded(&self) -> Result<(), AgentError> {
        let Some(model_service) = &self.model_service else {
            return Ok(());
        };
        let models = model_service
            .list_models(ListModelsFilter { capability: Some(Capability::ImageGeneration) })
            .await
            .map_err(to_tool_execution_error)?;
        let Some(model) = models.into_iter().find(|model| model.kind == UnifiedModelKind::Local)
        else {
            return Ok(());
        };

        // Drain the progress channel in the background — the tool surfaces no
        // load indicator (the turn-level marker is driven elsewhere); the
        // sends must simply never block.
        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel(16);
        tokio::spawn(async move { while progress_rx.recv().await.is_some() {} });
        match model_service
            .ensure_model_loaded_with_progress(
                ModelLoadCommand {
                    model_id: Some(model.id.clone()),
                    backend_id: None,
                    model_path: None,
                    num_workers: None,
                },
                progress_tx,
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(error) => Err(AgentError::ToolExecution(format!(
                "failed to prepare the image model {}: {error}",
                model.display_name
            ))),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GenerateImageArgs {
    /// Text description of the desired image.
    prompt: String,
    /// What to avoid in the generated image.
    negative_prompt: Option<String>,
    /// Image width in pixels (default 512).
    width: Option<u32>,
    /// Image height in pixels (default 512).
    height: Option<u32>,
    /// Number of images to generate (default 1).
    n: Option<u32>,
    /// Sampling steps.
    steps: Option<i32>,
    /// Reproducibility seed.
    seed: Option<i64>,
    /// Classifier-free guidance scale.
    cfg_scale: Option<f32>,
}

#[async_trait]
impl TypedTool for GenerateImageTool {
    type Input = GenerateImageArgs;
    fn name(&self) -> &str {
        "generate_image"
    }

    fn description(&self) -> &str {
        "Generate one or more images from a text prompt using the local diffusion \
         model and render them inline in the chat. The image model is loaded \
         automatically on first use (downloads when the weights are not local \
         yet). Returns the artifact URL(s); the image appears inline \
         automatically — do not describe it as text."
    }

    fn parameters_schema(&self) -> Value {
        typed_input_schema::<GenerateImageArgs>()
    }

    fn render_turn_item(&self, render: &ToolCallRender<'_>) -> TurnItem {
        // The image URL is only known once the tool completes (it is encoded in
        // the result `content`). While running, fall back to the generic
        // command-execution item so the call is still visible on the timeline.
        let Some(output) = render.output else {
            return default_tool_turn_item(render);
        };
        let Ok(value) = serde_json::from_str::<Value>(output) else {
            return default_tool_turn_item(render);
        };
        let Some(path) = value.get("image_url").and_then(Value::as_str) else {
            return default_tool_turn_item(render);
        };
        TurnItem::ImageView { id: render.call.id.clone(), path: path.to_owned() }
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        args: GenerateImageArgs,
    ) -> Result<ToolOutput, AgentError> {
        // Lazy-load: prepare the diffusion model BEFORE dispatching the
        // generation (no-op fast path when it is already resident).
        self.ensure_image_model_loaded().await?;

        // `model_id` left to the default loaded diffusion backend; `model` is the
        // informational model path (empty = use the pre-loaded default).
        let command = ImageGenerationCommand {
            model_id: None,
            model: String::new(),
            prompt: args.prompt,
            negative_prompt: args.negative_prompt,
            n: args.n.unwrap_or(1).clamp(1, 4),
            width: args.width.unwrap_or(512),
            height: args.height.unwrap_or(512),
            cfg_scale: args.cfg_scale,
            guidance: None,
            steps: args.steps,
            seed: args.seed,
            sample_method: None,
            scheduler: None,
            clip_skip: None,
            eta: None,
            strength: None,
            init_image: None,
            mode: ImageGenerationMode::Txt2Img,
        };

        let accepted =
            self.image_service.generate_images(command).await.map_err(to_tool_execution_error)?;
        let operation_id = accepted.operation_id;
        let view = poll_generation_task(&self.image_service, &operation_id).await?;

        if view.status != TaskStatus::Succeeded {
            return Err(AgentError::ToolExecution(format!(
                "image generation {} for task {operation_id}{}",
                view.status.as_str(),
                view.error_msg.as_deref().map(|message| format!(": {message}")).unwrap_or_default(),
            )));
        }

        let image_url = view
            .primary_image_url
            .clone()
            .or_else(|| view.image_urls.first().cloned())
            .unwrap_or_else(|| format!("/v1/images/generations/{operation_id}/artifacts/0"));

        Ok(ToolOutput {
            content: serde_json::json!({
                "status": "succeeded",
                "operation_id": operation_id,
                "image_url": image_url,
                "image_urls": view.image_urls,
            })
            .to_string(),
            metadata: None,
        })
    }
}

/// Poll `get_generation_task` until it reaches a terminal state or the budget is
/// exhausted. `ImageService::generate_images` is fire-and-forget (it spawns the
/// gRPC call and returns the operation id immediately), so the tool awaits
/// completion here.
async fn poll_generation_task(
    image_service: &ImageService,
    operation_id: &str,
) -> Result<ImageGenerationTaskView, AgentError> {
    for _ in 0..MAX_POLL_ATTEMPTS {
        let view = image_service
            .get_generation_task(operation_id)
            .await
            .map_err(to_tool_execution_error)?;
        if !matches!(view.status, TaskStatus::Pending | TaskStatus::Running) {
            return Ok(view);
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    Err(AgentError::ToolExecution(format!(
        "image generation timed out waiting for task {operation_id}"
    )))
}

fn to_tool_execution_error(error: AppCoreError) -> AgentError {
    // Preserve the stable machine codes at the tool boundary so the model
    // (and logs) can grep failures like `runtime_model_not_loaded` instead
    // of only prose. Other variants already render with English prefixes.
    let coded = match &error {
        AppCoreError::RuntimeFailure { message, data } => {
            format!("[runtime:{}] {message}", data.runtime_code().unwrap_or("unknown"))
        }
        AppCoreError::NotFound(message) => format!("[not_found] {message}"),
        _ => error.to_string(),
    };
    AgentError::ToolExecution(coded)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn prompt_field_is_required() {
        let schema = typed_input_schema::<GenerateImageArgs>();
        assert_eq!(schema["properties"]["prompt"]["type"], "string");
        assert_eq!(schema["required"], json!(["prompt"]));
    }

    #[test]
    fn args_decode_with_defaults() {
        let args: GenerateImageArgs =
            serde_json::from_value(json!({ "prompt": "a red cube" })).unwrap();
        assert_eq!(args.prompt, "a red cube");
        assert!(args.width.is_none() && args.height.is_none() && args.n.is_none());
    }

    /// P5 regression: the runtime's stable machine code (e.g.
    /// `runtime_model_not_loaded`) must survive the tool boundary instead of
    /// being flattened into prose.
    #[test]
    fn runtime_failure_error_keeps_runtime_code() {
        let data = Box::new(crate::error::AppCoreErrorData::runtime_failure(
            "runtime_model_not_loaded",
            serde_json::json!({"detail": "model is not loaded"}),
        ));
        let error = to_tool_execution_error(AppCoreError::RuntimeFailure {
            message: "model is not loaded".to_owned(),
            data,
        });
        let rendered = error.to_string();
        assert!(rendered.contains("[runtime:runtime_model_not_loaded]"), "{rendered}");
        assert!(rendered.contains("model is not loaded"), "{rendered}");
    }

    #[test]
    fn not_found_error_keeps_code_prefix() {
        let error =
            to_tool_execution_error(AppCoreError::NotFound("task abc does not exist".into()));
        assert!(error.to_string().contains("[not_found] task abc does not exist"), "{error}");
    }
}
