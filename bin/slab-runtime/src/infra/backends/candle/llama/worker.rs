//! Backend worker for `candle.llama`.

use std::sync::{Arc, Mutex};

use slab_candle::CandleRuntimeEngine;
use slab_candle::llm::{
    CandleLlmEngine, CandleLlmLoadConfig as SlabCandleLlmLoadConfig, LlmModelKind, LlmWeightSource,
    PromptFormat, SamplingConfig, TextGenerationRequest, TextGenerationStreamChunk,
};
use slab_runtime_core::backend::{
    ControlOpId, Input, Options, StreamChunk, StreamHandle, Typed, WorkerCommand,
};
use slab_runtime_core::backend::{SharedIngressRx, spawn_runtime_worker};
use slab_runtime_macros::backend_handler;
use tokio::sync::{broadcast, mpsc};

use super::error::CandleLlamaWorkerError;
use crate::domain::models::{
    CandleLlamaLoadConfig, TextGenerationOptions,
    TextGenerationResponse as RuntimeTextGenerationResponse, TextGenerationStreamEvent,
    TextGenerationUsage as RuntimeTextGenerationUsage,
};

type SharedEngine = Arc<Mutex<CandleLlmEngine>>;

struct CandleLlamaWorker {
    engine: Option<SharedEngine>,
}

#[backend_handler]
impl CandleLlamaWorker {
    fn new(engine: Option<SharedEngine>) -> Self {
        Self { engine }
    }

    #[on_event(LoadModel)]
    async fn on_load_model(
        &mut self,
        config: Input<CandleLlamaLoadConfig>,
    ) -> Result<(), CandleLlamaWorkerError> {
        self.handle_load_model(config.0).await
    }

    #[on_event(UnloadModel)]
    async fn on_unload_model(&mut self) -> Result<(), CandleLlamaWorkerError> {
        self.handle_unload_model().await
    }

    #[on_event(Inference)]
    async fn on_inference(
        &mut self,
        prompt: String,
        options: Options<TextGenerationOptions>,
    ) -> Result<Typed<RuntimeTextGenerationResponse>, CandleLlamaWorkerError> {
        self.handle_inference(prompt, options.0).await
    }

    #[on_event(InferenceStream)]
    async fn on_inference_stream(
        &mut self,
        prompt: String,
        options: Options<TextGenerationOptions>,
    ) -> Result<StreamHandle, CandleLlamaWorkerError> {
        self.handle_inference_stream(prompt, options.0).await
    }

    fn cleanup_runtime_state(&mut self) {
        if let Some(engine) = self.engine.as_ref()
            && let Ok(mut engine) = engine.lock()
        {
            engine.unload_model();
        }
        self.engine = None;
    }

    #[on_runtime_control(GlobalUnload)]
    #[on_runtime_control(GlobalLoad)]
    async fn apply_runtime_control(
        &mut self,
        op_id: ControlOpId,
    ) -> Result<(), CandleLlamaWorkerError> {
        tracing::debug!(op_id = op_id.0, "candle.llama runtime control pre-cleanup");
        self.cleanup_runtime_state();
        Ok(())
    }

    #[on_control_lagged]
    async fn on_control_lagged_cleanup(&mut self) -> Result<(), CandleLlamaWorkerError> {
        self.cleanup_runtime_state();
        Ok(())
    }

    async fn handle_load_model(
        &mut self,
        config: CandleLlamaLoadConfig,
    ) -> Result<(), CandleLlamaWorkerError> {
        let load_config = SlabCandleLlmLoadConfig {
            model_path: config.model_path,
            tokenizer_path: config.tokenizer_path,
            device: config.device,
            config_path: None,
            extra_weight_paths: Vec::new(),
            model_kind: LlmModelKind::Llama,
            weight_source: LlmWeightSource::QuantizedGguf,
            prompt_format: PromptFormat::Raw,
            seed: config.seed,
        };

        let result = tokio::task::block_in_place(move || {
            let mut engine = CandleLlmEngine::new();
            engine.load_model(load_config)?;
            Ok::<_, slab_candle::llm::CandleLlmError>(Arc::new(Mutex::new(engine)))
        });

        match result {
            Ok(engine) => {
                self.engine = Some(engine);
                Ok(())
            }
            Err(error) => Err(CandleLlamaWorkerError::load(error.to_string())),
        }
    }

    async fn handle_unload_model(&mut self) -> Result<(), CandleLlamaWorkerError> {
        match self.engine.take() {
            Some(engine) => {
                let mut engine = engine.lock().map_err(|_| {
                    CandleLlamaWorkerError::unload("candle.llama engine lock poisoned")
                })?;
                engine.unload_model();
                Ok(())
            }
            None => Err(CandleLlamaWorkerError::unload("model not loaded")),
        }
    }

    async fn handle_inference(
        &mut self,
        prompt: String,
        options: TextGenerationOptions,
    ) -> Result<Typed<RuntimeTextGenerationResponse>, CandleLlamaWorkerError> {
        let engine = self.loaded_engine()?;
        let request = build_text_request(prompt, options);
        let result = tokio::task::block_in_place(move || {
            let mut engine = engine.lock().map_err(|_| {
                CandleLlamaWorkerError::inference("candle.llama engine lock poisoned")
            })?;
            engine
                .infer(request)
                .map(map_text_response)
                .map_err(|error| CandleLlamaWorkerError::inference(error.to_string()))
        });

        result.map(Typed)
    }

    async fn handle_inference_stream(
        &mut self,
        prompt: String,
        options: TextGenerationOptions,
    ) -> Result<StreamHandle, CandleLlamaWorkerError> {
        let engine = self.loaded_engine()?;
        let request = build_text_request(prompt, options);
        let (proto_tx, proto_rx) = mpsc::channel::<StreamChunk>(64);

        tokio::task::spawn_blocking(move || {
            let result = {
                let mut engine = match engine.lock() {
                    Ok(engine) => engine,
                    Err(_) => {
                        let _ = proto_tx.blocking_send(StreamChunk::Error(
                            "candle.llama engine lock poisoned".to_owned(),
                        ));
                        let _ = proto_tx.blocking_send(StreamChunk::Done);
                        return;
                    }
                };

                engine.infer_stream(request, |chunk| match chunk {
                    TextGenerationStreamChunk::Token(token) => {
                        proto_tx.blocking_send(StreamChunk::Token(token)).is_ok()
                    }
                    TextGenerationStreamChunk::Done(response) => {
                        let event = TextGenerationStreamEvent {
                            done: Some(true),
                            finish_reason: response.finish_reason,
                            usage: response.usage.map(map_usage),
                            ..Default::default()
                        };
                        proto_tx
                            .blocking_send(StreamChunk::Json(
                                serde_json::to_value(event)
                                    .expect("candle llama terminal stream event should serialize"),
                            ))
                            .is_ok()
                    }
                })
            };

            if let Err(error) = result {
                let _ = proto_tx.blocking_send(StreamChunk::Error(error.to_string()));
            }
            let _ = proto_tx.blocking_send(StreamChunk::Done);
        });

        Ok(proto_rx)
    }

    fn loaded_engine(&self) -> Result<SharedEngine, CandleLlamaWorkerError> {
        self.engine
            .as_ref()
            .cloned()
            .ok_or_else(|| CandleLlamaWorkerError::inference("model not loaded"))
    }
}

pub fn spawn_backend_with_engine(
    shared_ingress_rx: SharedIngressRx,
    control_tx: broadcast::Sender<WorkerCommand>,
    engine: Option<SharedEngine>,
) {
    let worker = CandleLlamaWorker::new(engine);
    spawn_runtime_worker(shared_ingress_rx, control_tx.subscribe(), 0, worker);
}

fn build_text_request(prompt: String, options: TextGenerationOptions) -> TextGenerationRequest {
    let top_k =
        options.top_k.and_then(|value| usize::try_from(value).ok()).filter(|value| *value > 0);
    let repetition_penalty =
        options.repetition_penalty.unwrap_or_else(|| SamplingConfig::default().repeat_penalty);

    TextGenerationRequest {
        prompt,
        max_tokens: options.max_tokens.and_then(|value| usize::try_from(value).ok()).unwrap_or(256),
        sampling: SamplingConfig {
            temperature: options.temperature.map(f64::from),
            top_p: options.top_p.map(f64::from),
            top_k,
            repeat_penalty: repetition_penalty,
            ..SamplingConfig::default()
        },
        stop_sequences: options.stop_sequences,
        ignore_eos: options.ignore_eos,
    }
}

fn map_text_response(
    response: slab_candle::llm::TextGenerationResponse,
) -> RuntimeTextGenerationResponse {
    RuntimeTextGenerationResponse {
        text: response.text,
        finish_reason: response.finish_reason,
        usage: response.usage.map(map_usage),
        ..Default::default()
    }
}

fn map_usage(usage: slab_candle::llm::TextGenerationUsage) -> RuntimeTextGenerationUsage {
    RuntimeTextGenerationUsage {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        estimated: false,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::CandleLlamaWorker;
    use slab_runtime_core::backend::ControlOpId;

    #[tokio::test]
    async fn runtime_global_unload_clears_engine() {
        let mut worker = CandleLlamaWorker::new(None);
        worker.apply_runtime_control(ControlOpId(1)).await.expect("control cleanup should succeed");
        assert!(worker.engine.is_none(), "global unload should leave engine cleared");
    }

    #[tokio::test]
    async fn runtime_global_load_runs_pre_cleanup() {
        let mut worker = CandleLlamaWorker::new(None);
        worker.apply_runtime_control(ControlOpId(2)).await.expect("control cleanup should succeed");
        assert!(worker.engine.is_none());
    }
}
