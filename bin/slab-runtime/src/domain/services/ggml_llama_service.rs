use futures::StreamExt;
use futures::stream::BoxStream;
use slab_llama::{LlamaInferenceParams, LlamaLoadConfig};
use slab_runtime_core::Payload;
use slab_types::{Capability, ModelFamily};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::application::dtos as dto;
use crate::domain::runtime::CoreError;

use super::ExecutionHub;
use super::driver_runtime::DriverRuntime;
use super::helpers::{
    decode_text_response, decode_text_stream_chunk, invalid_model, model_spec, required_path,
    required_string,
};

#[derive(Clone, Debug)]
pub(crate) struct GgmlLlamaService {
    runtime: DriverRuntime,
}

impl GgmlLlamaService {
    pub(crate) fn new(
        execution: ExecutionHub,
        request: dto::GgmlLlamaLoadRequest,
    ) -> Result<Self, CoreError> {
        let model_path = required_path("ggml_llama.model_path", request.model_path)?;
        let num_workers = request
            .num_workers
            .ok_or_else(|| invalid_model("ggml_llama.num_workers", "missing required value"))?;
        if num_workers == 0 {
            return Err(invalid_model("ggml_llama.num_workers", "must be > 0"));
        }
        let flash_attn = request
            .flash_attn
            .ok_or_else(|| invalid_model("ggml_llama.flash_attn", "missing required value"))?;

        let load_payload = Payload::typed(LlamaLoadConfig {
            model_path: model_path.clone(),
            num_workers: usize::try_from(num_workers)
                .map_err(|_| invalid_model("ggml_llama.num_workers", "exceeds usize range"))?,
            context_length: request.context_length,
            flash_attn,
            chat_template: request.chat_template,
            gbnf: request.gbnf,
        });

        Ok(Self {
            runtime: DriverRuntime::new(
                execution,
                model_spec(ModelFamily::Llama, Capability::TextGeneration, model_path),
                "ggml.llama",
                load_payload,
            ),
        })
    }

    pub(crate) async fn load(&self) -> Result<(), CoreError> {
        self.runtime.load().await
    }

    pub(crate) async fn unload(&self) -> Result<(), CoreError> {
        self.runtime.unload().await
    }

    pub(crate) async fn chat(
        &self,
        request: dto::GgmlLlamaChatRequest,
    ) -> Result<dto::LlamaChatResponse, CoreError> {
        let prompt = required_string("ggml_llama.prompt", request.prompt.clone())?;
        let payload = self
            .runtime
            .submit(
                Capability::TextGeneration,
                false,
                Payload::text(prompt),
                Vec::new(),
                Payload::typed(build_inference_params(request)?),
            )
            .await?
            .result()
            .await?;
        decode_text_response(payload, "ggml_llama")
    }

    pub(crate) async fn chat_stream(
        &self,
        request: dto::GgmlLlamaChatRequest,
    ) -> Result<BoxStream<'static, Result<dto::LlamaChatStreamChunk, CoreError>>, CoreError> {
        let prompt = required_string("ggml_llama.prompt", request.prompt.clone())?;
        let handle = self
            .runtime
            .submit(
                Capability::TextGeneration,
                true,
                Payload::text(prompt),
                Vec::new(),
                Payload::typed(build_inference_params(request)?),
            )
            .await?;
        let raw_stream = match handle.take_stream().await {
            Ok(stream) => stream,
            Err(error) => {
                handle.cancel_and_purge().await;
                return Err(error);
            }
        };

        let (tx, rx) = mpsc::channel::<Result<dto::LlamaChatStreamChunk, CoreError>>(32);
        tokio::spawn(async move {
            tokio::pin!(raw_stream);
            while let Some(chunk) = raw_stream.next().await {
                let next = match chunk {
                    Ok(chunk) => match decode_text_stream_chunk(chunk, "ggml_llama") {
                        Ok(Some(chunk)) => Some(Ok(chunk)),
                        Ok(None) => None,
                        Err(error) => Some(Err(error)),
                    },
                    Err(error) => Some(Err(error)),
                };

                if let Some(next) = next
                    && tx.send(next).await.is_err()
                {
                    handle.cancel_and_purge().await;
                    return;
                }
            }
            handle.purge().await;
        });

        Ok(ReceiverStream::new(rx).boxed())
    }
}

fn build_inference_params(
    request: dto::GgmlLlamaChatRequest,
) -> Result<LlamaInferenceParams, CoreError> {
    let logit_bias = match request.logit_bias_json {
        Some(bytes) => Some(serde_json::from_slice(&bytes).map_err(|error| {
            invalid_model("ggml_llama.logit_bias_json", format!("invalid JSON payload: {error}"))
        })?),
        None => None,
    };

    Ok(LlamaInferenceParams {
        max_tokens: request
            .max_tokens
            .map(usize::try_from)
            .transpose()
            .map_err(|_| invalid_model("ggml_llama.max_tokens", "exceeds usize range"))?
            .unwrap_or_default(),
        session_key: request.session_key,
        gbnf: request.gbnf,
        temperature: request.temperature,
        top_p: request.top_p,
        top_k: request.top_k,
        min_p: request.min_p,
        repetition_penalty: request.repetition_penalty,
        presence_penalty: request.presence_penalty,
        ignore_eos: request.ignore_eos.unwrap_or(false),
        logit_bias,
        stop_sequences: request.stop_sequences.unwrap_or_default(),
    })
}
