use futures::StreamExt;
use futures::stream::BoxStream;
use slab_runtime_core::{CoreError, Payload};
use slab_types::{CandleLlamaLoadConfig, Capability, ModelFamily, TextGenerationOpOptions};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use slab_proto::convert::dto;

use super::ExecutionHub;
use super::driver_runtime::DriverRuntime;
use super::helpers::{
    decode_text_response, decode_text_stream_chunk, invalid_model, model_spec, required_path,
    required_string,
};

#[derive(Clone, Debug)]
pub(crate) struct CandleLlamaService {
    runtime: DriverRuntime,
}

impl CandleLlamaService {
    pub(crate) fn new(
        execution: ExecutionHub,
        request: dto::CandleLlamaLoadRequest,
    ) -> Result<Self, CoreError> {
        let model_path = required_path("candle_llama.model_path", request.model_path)?;
        let seed = request
            .seed
            .ok_or_else(|| invalid_model("candle_llama.seed", "missing required value"))?;
        let load_payload = Payload::typed(CandleLlamaLoadConfig {
            model_path: model_path.clone(),
            tokenizer_path: request.tokenizer_path,
            seed,
        });

        Ok(Self {
            runtime: DriverRuntime::new(
                execution,
                model_spec(ModelFamily::Llama, Capability::TextGeneration, model_path),
                "candle.llama",
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
        request: dto::CandleChatRequest,
    ) -> Result<dto::LlamaChatResponse, CoreError> {
        let prompt = required_string("candle_llama.prompt", request.prompt)?;
        let payload = self
            .runtime
            .submit(
                Capability::TextGeneration,
                false,
                Payload::text(prompt),
                Vec::new(),
                Payload::typed(TextGenerationOpOptions {
                    max_tokens: request.max_tokens,
                    session_key: request.session_key,
                    stream: false,
                    ..Default::default()
                }),
            )
            .await?
            .result()
            .await?;
        decode_text_response(payload, "candle_llama")
    }

    pub(crate) async fn chat_stream(
        &self,
        request: dto::CandleChatRequest,
    ) -> Result<BoxStream<'static, Result<dto::LlamaChatStreamChunk, CoreError>>, CoreError> {
        let prompt = required_string("candle_llama.prompt", request.prompt)?;
        let handle = self
            .runtime
            .submit(
                Capability::TextGeneration,
                true,
                Payload::text(prompt),
                Vec::new(),
                Payload::typed(TextGenerationOpOptions {
                    max_tokens: request.max_tokens,
                    session_key: request.session_key,
                    stream: true,
                    ..Default::default()
                }),
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
                    Ok(chunk) => match decode_text_stream_chunk(chunk, "candle_llama") {
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
