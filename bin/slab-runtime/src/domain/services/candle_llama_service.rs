use futures::StreamExt;
use futures::stream::BoxStream;
use slab_runtime_core::backend::RequestRoute;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::application::dtos as dto;
use crate::domain::models::{CandleLlamaLoadConfig, TextGenerationOptions};
use crate::domain::runtime::CoreError;

use super::ExecutionHub;
use super::driver_runtime::DriverRuntime;
use super::helpers::{
    decode_text_response, decode_text_stream_chunk, invalid_model, required_path, required_string,
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
        let load_payload = CandleLlamaLoadConfig {
            model_path: model_path.clone(),
            tokenizer_path: request.tokenizer_path,
            seed,
        };

        Ok(Self {
            runtime: DriverRuntime::new_typed(
                execution,
                "candle.llama",
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
            .submit_payload(
                RequestRoute::Inference,
                prompt,
                Vec::new(),
                TextGenerationOptions {
                    max_tokens: request.max_tokens,
                    session_key: request.session_key,
                    stream: false,
                    ..Default::default()
                },
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
            .submit_payload(
                RequestRoute::InferenceStream,
                prompt,
                Vec::new(),
                TextGenerationOptions {
                    max_tokens: request.max_tokens,
                    session_key: request.session_key,
                    stream: true,
                    ..Default::default()
                },
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
