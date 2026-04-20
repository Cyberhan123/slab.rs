//! Backend worker adapter for `ggml.llama`.
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
//! Uses a typed [`slab_llama::LlamaLoadConfig`] payload.
//!
//! ### `inference` / `inference.stream` options payload
//! Uses a typed [`slab_llama::LlamaInferenceParams`] payload. Grammar and chat
//! message normalization are resolved before the backend receives the request.
//!
//! Runtime and peer control hooks are also routed through typed extractor
//! arguments, but remain fire-and-forget because the control bus has no reply
//! channel.

use std::sync::Arc;

use serde_json::json;
use slab_llama::{LlamaInferenceParams, LlamaLoadConfig};
use tokio::sync::broadcast;

use crate::infra::backends::ggml::llama::adapter::{
    GGMLLlamaEngine, LlamaDispatchOutput, LlamaDispatchRequest,
};
use slab_runtime_core::backend::{
    CancelRx, ControlOpId, Input, Options, StreamHandle, WorkerCommand,
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
    fn from_params(params: LlamaInferenceParams) -> Self {
        Self {
            max_tokens: if params.max_tokens == 0 { 256 } else { params.max_tokens },
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
    async fn on_load_model(&mut self, config: Input<LlamaLoadConfig>) -> Result<(), anyhow::Error> {
        self.handle_load_model(config.0).await.map_err(anyhow::Error::msg)
    }

    #[on_event(UnloadModel)]
    async fn on_unload_model(&mut self) -> Result<(), anyhow::Error> {
        self.handle_unload_model().await.map_err(anyhow::Error::msg)
    }

    #[on_event(Inference)]
    async fn on_inference(
        &mut self,
        prompt: String,
        options: Options<LlamaInferenceParams>,
    ) -> Result<serde_json::Value, anyhow::Error> {
        let options = InferenceOptions::from_params(options.0);
        self.handle_inference(prompt, options).await.map_err(anyhow::Error::msg)
    }

    #[on_event(InferenceStream)]
    async fn on_inference_stream(
        &mut self,
        prompt: String,
        options: Options<LlamaInferenceParams>,
        cancel: CancelRx,
    ) -> Result<StreamHandle, anyhow::Error> {
        let options = InferenceOptions::from_params(options.0);
        self.handle_inference_stream(prompt, options, cancel).await.map_err(anyhow::Error::msg)
    }

    fn cleanup_runtime_state(&mut self) {
        if let Some(engine) = self.engine.as_ref() {
            let _ = engine.unload();
        }
    }

    #[on_runtime_control(GlobalUnload)]
    #[on_runtime_control(GlobalLoad)]
    async fn apply_runtime_control(&mut self, op_id: ControlOpId) -> Result<(), anyhow::Error> {
        tracing::debug!(op_id = op_id.0, "llama runtime control pre-cleanup");
        // Runtime-level GlobalLoad is treated as a pre-load cleanup signal.
        // The actual model.load request is still driven by the management path.
        self.cleanup_runtime_state();
        Ok(())
    }

    #[on_control_lagged]
    async fn on_control_lagged_cleanup(&mut self) -> Result<(), anyhow::Error> {
        self.cleanup_runtime_state();
        Ok(())
    }

    // ── model.load ────────────────────────────────────────────────────────────

    async fn handle_load_model(&mut self, config: LlamaLoadConfig) -> Result<(), String> {
        let engine = match self.engine.as_ref() {
            Some(e) => Arc::clone(e),
            None => {
                return Err("engine not initialized".to_owned());
            }
        };

        if config.num_workers == 0 {
            return Err("num_workers must be > 0".to_owned());
        }

        // Model loading is CPU/blocking; use block_in_place to avoid stalling
        // the async runtime without the Send constraint of spawn_blocking.
        let result = tokio::task::block_in_place(|| {
            use slab_llama::{LlamaContextParams, LlamaModelParams};
            let mut ctx_params = LlamaContextParams::default();
            // This backend continuously batches multiple seq_ids inside a worker
            // and callers treat `context_length` as the usable window per session.
            ctx_params.kv_unified = true;
            ctx_params.flash_attn = config.flash_attn;
            if let Some(context_length) = config.context_length {
                ctx_params.n_ctx = context_length;
                if ctx_params.n_batch > context_length {
                    ctx_params.n_batch = context_length;
                }
                if ctx_params.n_ubatch > context_length {
                    ctx_params.n_ubatch = context_length;
                }
            }

            engine.load_model_with_workers(
                &config.model_path,
                LlamaModelParams::default(),
                ctx_params,
                config.num_workers,
            )
        });

        result.map_err(|error| error.to_string())
    }

    // ── model.unload ──────────────────────────────────────────────────────────

    async fn handle_unload_model(&mut self) -> Result<(), String> {
        let engine = match self.engine.as_ref() {
            Some(e) => Arc::clone(e),
            None => {
                return Err("engine not initialized".to_owned());
            }
        };

        match engine.unload() {
            Ok(()) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    // ── inference ─────────────────────────────────────────────────────────────

    async fn handle_inference(
        &mut self,
        prompt: String,
        options: InferenceOptions,
    ) -> Result<serde_json::Value, String> {
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
        let engine =
            self.engine.as_ref().map(Arc::clone).ok_or_else(|| "model not loaded".to_owned())?;
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
        let LlamaDispatchOutput { text, usage, finish_reason, metadata } =
            engine.dispatch_inference(request).await.map_err(|error| error.to_string())?;
        let tokens_used = usage.as_ref().map(|usage| usage.completion_tokens);
        Ok(json!({
            "text": text,
            "tokens_used": tokens_used,
            "usage": usage,
            "finish_reason": finish_reason,
            "metadata": metadata,
        }))
    }

    // ── inference.stream ──────────────────────────────────────────────────────

    async fn handle_inference_stream(
        &mut self,
        prompt: String,
        options: InferenceOptions,
        cancel: CancelRx,
    ) -> Result<StreamHandle, String> {
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
        let engine =
            self.engine.as_ref().map(Arc::clone).ok_or_else(|| "model not loaded".to_owned())?;
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
        engine
            .dispatch_inference_stream(request, cancel.0)
            .await
            .map_err(|error: crate::infra::backends::ggml::EngineError| error.to_string())
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
    use super::{InferenceOptions, LlamaWorker};
    use slab_llama::LlamaInferenceParams;
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
        let options = InferenceOptions::from_params(LlamaInferenceParams {
            max_tokens: 32,
            ignore_eos: true,
            logit_bias: Some(serde_json::json!({ "42": false })),
            ..Default::default()
        });

        assert!(options.ignore_eos);
        assert_eq!(options.logit_bias, Some(serde_json::json!({ "42": false })));
    }
}
