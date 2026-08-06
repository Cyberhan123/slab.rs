use futures::StreamExt;
use futures::stream::BoxStream;
use slab_runtime_core::backend::RequestRoute;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::application::dtos as dto;
use crate::domain::models::{
    GgmlLlamaLoadConfig, GgmlLlamaLoadMetadata, GgmlLlamaQuantizeInput, GgmlLlamaQuantizeOutput,
    TextGenerationImagePart, TextGenerationOptions,
};
use crate::domain::runtime::CoreError;

use super::ExecutionHub;
use super::driver_runtime::DriverRuntime;
use super::helpers::{
    decode_text_response, decode_text_stream_chunk, invalid_model, required_path, required_string,
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

        let load_payload = GgmlLlamaLoadConfig {
            model_path: model_path.clone(),
            engine_workers: usize::try_from(num_workers)
                .map_err(|_| invalid_model("ggml_llama.num_workers", "exceeds usize range"))?,
            context_length: request.context_length,
            free_vram_bytes: request.free_vram_bytes,
            flash_attn,
            chat_template: request.chat_template,
            gbnf: request.gbnf,
            mmproj_path: request.mmproj_path,
        };

        Ok(Self {
            runtime: DriverRuntime::new_typed(execution, "ggml.llama", "ggml.llama", load_payload),
        })
    }

    pub(crate) async fn load(&self) -> Result<Option<GgmlLlamaLoadMetadata>, CoreError> {
        self.runtime.load_with_result().await
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
            .submit_payload(
                RequestRoute::Inference,
                prompt,
                Vec::new(),
                build_inference_params(request)?,
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
            .submit_payload(
                RequestRoute::InferenceStream,
                prompt,
                Vec::new(),
                build_inference_params(request)?,
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

    pub(crate) async fn quantize(
        &self,
        request: dto::GgmlLlamaQuantizeRequest,
    ) -> Result<dto::GgmlLlamaQuantizeResult, CoreError> {
        let input = GgmlLlamaQuantizeInput {
            input_path: request.input_path,
            output_path: request.output_path,
            ftype: request.ftype,
            nthread: request.nthread,
            allow_requantize: request.allow_requantize.unwrap_or(false),
            // Mirrors `llama_model_quantize_default_params()`: quantize the
            // output tensor by default unless the caller explicitly disables it.
            quantize_output_tensor: request.quantize_output_tensor.unwrap_or(true),
            only_copy: request.only_copy.unwrap_or(false),
            pure: request.pure.unwrap_or(false),
            keep_split: request.keep_split.unwrap_or(false),
            dry_run: request.dry_run.unwrap_or(false),
        };
        let output = self
            .runtime
            .invoke_without_options::<GgmlLlamaQuantizeInput, GgmlLlamaQuantizeOutput>(
                RequestRoute::Quantize,
                input,
                Vec::new(),
            )
            .await?;
        Ok(dto::GgmlLlamaQuantizeResult {
            layers_processed: output.layers_processed,
            output_path: output.output_path,
        })
    }
}

fn build_inference_params(
    request: dto::GgmlLlamaChatRequest,
) -> Result<TextGenerationOptions, CoreError> {
    let logit_bias = match request.logit_bias_json {
        Some(bytes) => Some(serde_json::from_slice(&bytes).map_err(|error| {
            invalid_model("ggml_llama.logit_bias_json", format!("invalid JSON payload: {error}"))
        })?),
        None => None,
    };

    if let Some(max_tokens) = request.max_tokens
        && usize::try_from(max_tokens).is_err()
    {
        return Err(invalid_model("ggml_llama.max_tokens", "exceeds usize range"));
    }

    Ok(TextGenerationOptions {
        max_tokens: request.max_tokens,
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
        agent_trace: request.agent_trace,
        stream: false,
        image_parts: request
            .image_parts
            .into_iter()
            .map(|part| TextGenerationImagePart { data: part.data, mime_type: part.mime_type })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::build_inference_params;
    use crate::application::dtos::GgmlLlamaChatRequest;

    #[test]
    fn build_inference_params_preserves_logit_bias_and_stop_sequences() {
        let options = build_inference_params(GgmlLlamaChatRequest {
            prompt: Some("hello".to_owned()),
            max_tokens: Some(32),
            stop_sequences: Some(vec!["</think>".to_owned(), "###".to_owned()]),
            ignore_eos: Some(true),
            logit_bias_json: Some(br#"{"42":false,"hello":1.5}"#.to_vec()),
            ..Default::default()
        })
        .expect("request should map");

        assert_eq!(options.max_tokens, Some(32));
        assert!(options.ignore_eos);
        assert_eq!(options.stop_sequences, vec!["</think>".to_owned(), "###".to_owned()]);
        assert_eq!(options.logit_bias, Some(serde_json::json!({ "42": false, "hello": 1.5 })));
    }
}
