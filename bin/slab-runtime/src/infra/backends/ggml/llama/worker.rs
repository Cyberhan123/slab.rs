//! Backend worker for `ggml.llama`.
//!
//! Provides [`spawn_backend`] and [`spawn_backend_with_path`] which start a
//! Tokio worker whose `#[backend_handler]` routes translate typed event and
//! control handlers into llama inference calls.
//!
//! # Supported ops
//!
//! | Op string            | Event variant    | Description                                    |
//! |----------------------|------------------|------------------------------------------------|
//! | `"model.load"`       | `LoadModel`      | Load a GGUF model from the engine.             |
//! | `"model.unload"`     | `UnloadModel`    | Drop the model handle; call model.load to restore. |
//! | `"inference"`        | `Inference`      | Unary text generation; input is UTF-8 prompt.  |
//! | `"inference.stream"` | `InferenceStream`| Streaming text generation.                     |
//!
//! ### `model.load` input payload
//! Uses a typed runtime-owned `GgmlLlamaLoadConfig` payload.
//!
//! ### `inference` / `inference.stream` options payload
//! Uses a typed runtime-owned `TextGenerationOptions` payload. Grammar and chat
//! message normalization are resolved before the backend receives the request.
//!
//! Runtime and peer control hooks are also routed through typed extractor
//! arguments, but remain fire-and-forget because the control bus has no reply
//! channel.

use std::sync::Arc;

use tokio::sync::broadcast;

use super::contract::{GgmlLlamaLoadConfig, TextGenerationOptions, TextGenerationResponse};
use super::engine::{GGMLLlamaEngine, LlamaDispatchOutput, LlamaDispatchRequest};
use super::error::GGMLLlamaWorkerError;
use slab_runtime_core::backend::{
    CancelRx, ControlOpId, Input, Options, StreamHandle, Typed, WorkerCommand,
};
use slab_runtime_core::backend::{SharedIngressRx, spawn_runtime_worker};
use slab_runtime_macros::backend_handler;

// ── Configurations ────────────────────────────────────────────────────────────

struct InferenceOptions {
    max_tokens: usize,
    session_key: Option<String>,
    gbnf: Option<String>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    top_k: Option<i32>,
    min_p: Option<f32>,
    repetition_penalty: Option<f32>,
    presence_penalty: Option<f32>,
    ignore_eos: bool,
    logit_bias: Option<serde_json::Value>,
    stop_sequences: Vec<String>,
}

impl InferenceOptions {
    fn from_options(params: TextGenerationOptions) -> Self {
        Self {
            max_tokens: params
                .max_tokens
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| *value > 0)
                .unwrap_or(256),
            session_key: params.session_key,
            gbnf: params.gbnf,
            temperature: params.temperature,
            top_p: params.top_p,
            top_k: params.top_k,
            min_p: params.min_p,
            repetition_penalty: params.repetition_penalty,
            presence_penalty: params.presence_penalty,
            ignore_eos: params.ignore_eos,
            logit_bias: params.logit_bias,
            stop_sequences: params.stop_sequences,
        }
    }
}

// ── Worker ────────────────────────────────────────────────────────────────────

struct LlamaWorker {
    /// The engine: wraps both the library handle and inference workers.
    /// - `None` → library not loaded.
    /// - `Some(e)` where `e.inference_engine` is None → lib loaded, no model.
    /// - `Some(e)` where `e.inference_engine` is Some → lib + model loaded.
    engine: Option<Arc<GGMLLlamaEngine>>,
}

#[backend_handler]
impl LlamaWorker {
    fn new(engine: Option<Arc<GGMLLlamaEngine>>) -> Self {
        Self { engine }
    }

    #[on_event(LoadModel)]
    async fn on_load_model(
        &mut self,
        config: Input<GgmlLlamaLoadConfig>,
    ) -> Result<(), GGMLLlamaWorkerError> {
        self.handle_load_model(config.0).await
    }

    #[on_event(UnloadModel)]
    async fn on_unload_model(&mut self) -> Result<(), GGMLLlamaWorkerError> {
        self.handle_unload_model().await
    }

    #[on_event(Inference)]
    async fn on_inference(
        &mut self,
        prompt: String,
        options: Options<TextGenerationOptions>,
    ) -> Result<Typed<TextGenerationResponse>, GGMLLlamaWorkerError> {
        let options = InferenceOptions::from_options(options.0);
        self.handle_inference(prompt, options).await
    }

    #[on_event(InferenceStream)]
    async fn on_inference_stream(
        &mut self,
        prompt: String,
        options: Options<TextGenerationOptions>,
        cancel: CancelRx,
    ) -> Result<StreamHandle, GGMLLlamaWorkerError> {
        let options = InferenceOptions::from_options(options.0);
        self.handle_inference_stream(prompt, options, cancel).await
    }

    fn cleanup_runtime_state(&mut self) {
        if let Some(engine) = self.engine.as_ref() {
            let _ = engine.unload();
        }
    }

    #[on_runtime_control(GlobalUnload)]
    #[on_runtime_control(GlobalLoad)]
    async fn apply_runtime_control(
        &mut self,
        op_id: ControlOpId,
    ) -> Result<(), GGMLLlamaWorkerError> {
        tracing::debug!(op_id = op_id.0, "llama runtime control pre-cleanup");
        // Runtime-level GlobalLoad is treated as a pre-load cleanup signal.
        // The actual model.load request is still driven by the management path.
        self.cleanup_runtime_state();
        Ok(())
    }

    #[on_control_lagged]
    async fn on_control_lagged_cleanup(&mut self) -> Result<(), GGMLLlamaWorkerError> {
        self.cleanup_runtime_state();
        Ok(())
    }

    // ── model.load ────────────────────────────────────────────────────────────

    async fn handle_load_model(
        &mut self,
        config: GgmlLlamaLoadConfig,
    ) -> Result<(), GGMLLlamaWorkerError> {
        let engine = match self.engine.as_ref() {
            Some(e) => Arc::clone(e),
            None => {
                return Err(GGMLLlamaWorkerError::load("engine not initialized"));
            }
        };

        if config.engine_workers == 0 {
            return Err(GGMLLlamaWorkerError::contract("engine_workers must be > 0"));
        }

        // Model loading is CPU/blocking; use block_in_place to avoid stalling
        // the async runtime without the Send constraint of spawn_blocking.
        let result = tokio::task::block_in_place(|| engine.load_model_from_config(&config));

        result.map_err(|error| GGMLLlamaWorkerError::load(error.to_string()))
    }

    // ── model.unload ──────────────────────────────────────────────────────────

    async fn handle_unload_model(&mut self) -> Result<(), GGMLLlamaWorkerError> {
        let engine = match self.engine.as_ref() {
            Some(e) => Arc::clone(e),
            None => {
                return Err(GGMLLlamaWorkerError::unload("engine not initialized"));
            }
        };

        engine.unload().map_err(|error| GGMLLlamaWorkerError::unload(error.to_string()))
    }

    // ── inference ─────────────────────────────────────────────────────────────

    async fn handle_inference(
        &mut self,
        prompt: String,
        options: InferenceOptions,
    ) -> Result<Typed<TextGenerationResponse>, GGMLLlamaWorkerError> {
        let InferenceOptions {
            max_tokens,
            session_key,
            gbnf,
            temperature,
            top_p,
            top_k,
            min_p,
            repetition_penalty,
            presence_penalty,
            ignore_eos,
            logit_bias,
            stop_sequences,
        } = options;
        let engine = self
            .engine
            .as_ref()
            .map(Arc::clone)
            .ok_or_else(|| GGMLLlamaWorkerError::inference("model not loaded"))?;
        let request = LlamaDispatchRequest {
            prompt,
            max_tokens,
            session_key,
            gbnf,
            temperature,
            top_p,
            top_k,
            min_p,
            repetition_penalty,
            presence_penalty,
            ignore_eos,
            logit_bias,
            stop_sequences,
        };
        let LlamaDispatchOutput { text, usage, finish_reason, metadata } = engine
            .dispatch_inference(request)
            .await
            .map_err(|error| GGMLLlamaWorkerError::inference(error.to_string()))?;
        Ok(Typed(TextGenerationResponse { text, finish_reason, usage, metadata }))
    }

    // ── inference.stream ──────────────────────────────────────────────────────

    async fn handle_inference_stream(
        &mut self,
        prompt: String,
        options: InferenceOptions,
        cancel: CancelRx,
    ) -> Result<StreamHandle, GGMLLlamaWorkerError> {
        let InferenceOptions {
            max_tokens,
            session_key,
            gbnf,
            temperature,
            top_p,
            top_k,
            min_p,
            repetition_penalty,
            presence_penalty,
            ignore_eos,
            logit_bias,
            stop_sequences,
        } = options;
        let engine = self
            .engine
            .as_ref()
            .map(Arc::clone)
            .ok_or_else(|| GGMLLlamaWorkerError::inference("model not loaded"))?;
        let request = LlamaDispatchRequest {
            prompt,
            max_tokens,
            session_key,
            gbnf,
            temperature,
            top_p,
            top_k,
            min_p,
            repetition_penalty,
            presence_penalty,
            ignore_eos,
            logit_bias,
            stop_sequences,
        };
        engine.dispatch_inference_stream(request, cancel.0).await.map_err(
            |error: crate::infra::backends::ggml::EngineError| {
                GGMLLlamaWorkerError::inference(error.to_string())
            },
        )
    }
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Spawn a llama backend worker with a pre-loaded engine handle.
///
/// Used by runtime construction to separate library loading (phase 1) from
/// worker spawning (phase 2) so that no tasks are started if any library fails.
pub fn spawn_backend_with_engine(
    shared_ingress_rx: SharedIngressRx,
    control_tx: broadcast::Sender<WorkerCommand>,
    engine: Option<Arc<GGMLLlamaEngine>>,
) {
    let worker = LlamaWorker::new(engine);
    spawn_runtime_worker(shared_ingress_rx, control_tx.subscribe(), 0, worker);
}

#[cfg(test)]
mod tests {
    use super::super::contract::TextGenerationOptions;
    use super::{InferenceOptions, LlamaWorker};
    use slab_runtime_core::backend::ControlOpId;

    // ── infer_add_assistant_prompt ────────────────────────────────────────────

    // ── LlamaWorker session management ────────────────────────────────────────

    #[tokio::test]
    async fn runtime_global_unload_is_safe_without_engine() {
        let mut worker = LlamaWorker::new(None);

        worker.apply_runtime_control(ControlOpId(1)).await.expect("control cleanup should succeed");

        assert!(worker.engine.is_none(), "global unload should remain safe without an engine");
    }

    #[tokio::test]
    async fn runtime_global_load_is_safe_without_engine() {
        let mut worker = LlamaWorker::new(None);

        worker.apply_runtime_control(ControlOpId(2)).await.expect("control cleanup should succeed");

        assert!(worker.engine.is_none(), "global load pre-cleanup should remain safe");
    }

    #[test]
    fn inference_options_preserve_ignore_eos_and_logit_bias() {
        let options = InferenceOptions::from_options(TextGenerationOptions {
            max_tokens: Some(32),
            ignore_eos: true,
            logit_bias: Some(serde_json::json!({ "42": false })),
            ..Default::default()
        });

        assert!(options.ignore_eos);
        assert_eq!(options.logit_bias, Some(serde_json::json!({ "42": false })));
    }
}
