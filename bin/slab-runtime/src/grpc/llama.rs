use futures::StreamExt;
use slab_types::inference::{TextGenerationUsage, TextPromptTokensDetails};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{Instrument, debug, error, info, instrument, warn};

use slab_proto::{convert, slab::ipc::v1 as pb};
use slab_runtime_core::api::TextGenerationChunk;

use super::{BackendKind, GrpcServiceImpl, extract_request_id, proto_to_status, runtime_to_status};

#[tonic::async_trait]
impl pb::llama_service_server::LlamaService for GrpcServiceImpl {
    #[instrument(skip_all, fields(request_id, backend = "ggml.llama"))]
    async fn chat(
        &self,
        request: Request<pb::ChatRequest>,
    ) -> Result<Response<pb::ChatResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let req = request.into_inner();
        debug!(
            prompt_len = req.prompt.len(),
            max_tokens = req.max_tokens,
            "llama chat request received"
        );

        let pipeline = self.pipeline_for_backend(BackendKind::Llama).await?;
        let request = convert::decode_chat_request(&req, false).map_err(proto_to_status)?;
        let response = pipeline.run_text_generation(request).await.map_err(|error| {
            error!(error = %error, "llama text generation failed");
            runtime_to_status(error)
        })?;

        info!(output_len = response.text.len(), "llama chat completed");
        Ok(Response::new(convert::encode_chat_response(&response)))
    }

    type ChatStreamStream = ReceiverStream<Result<pb::ChatStreamChunk, Status>>;

    #[instrument(skip_all, fields(request_id, backend = "ggml.llama"))]
    async fn chat_stream(
        &self,
        request: Request<pb::ChatRequest>,
    ) -> Result<Response<Self::ChatStreamStream>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let req = request.into_inner();
        debug!(
            prompt_len = req.prompt.len(),
            max_tokens = req.max_tokens,
            "llama chat_stream request received"
        );

        let pipeline = self.pipeline_for_backend(BackendKind::Llama).await?;
        let max_tokens = req.max_tokens;
        let prompt_for_usage = req.prompt.clone();
        let request = convert::decode_chat_request(&req, true).map_err(proto_to_status)?;
        let stream_handle = pipeline.submit_text_generation(request).await.map_err(|error| {
            error!(error = %error, "llama text generation stream setup failed");
            runtime_to_status(error)
        })?;
        let backend_stream = match stream_handle.take_stream().await {
            Ok(stream) => stream,
            Err(error) => {
                stream_handle.cancel_and_purge().await;
                error!(error = %error, "llama text generation stream handle failed");
                return Err(runtime_to_status(error));
            }
        };

        let (tx, rx) = mpsc::channel::<Result<pb::ChatStreamChunk, Status>>(32);
        tokio::spawn(
            async move {
                tokio::pin!(backend_stream);
                let mut token_count = 0usize;
                while let Some(chunk) = backend_stream.next().await {
                    let message = match chunk {
                        Ok(chunk) => {
                            token_count += 1;
                            convert::encode_chat_stream_chunk(&chunk)
                        }
                        Err(error) => {
                            warn!(error = %error, "error in llama stream chunk");
                            pb::ChatStreamChunk {
                                token: String::new(),
                                error: error.to_string(),
                                done: false,
                                finish_reason: String::new(),
                                usage: None,
                            }
                        }
                    };

                    if tx.send(Ok(message)).await.is_err() {
                        debug!("llama stream receiver dropped; cancelling runtime task");
                        stream_handle.cancel_and_purge().await;
                        return;
                    }
                }

                debug!(token_count, "llama chat_stream relay finished");
                let completion_tokens = u32::try_from(token_count).unwrap_or(u32::MAX);
                let finish_reason = finish_reason_from_token_budget(completion_tokens, max_tokens);
                let usage = build_estimated_usage(&prompt_for_usage, completion_tokens);
                let _ = tx
                    .send(Ok(convert::encode_chat_stream_chunk(&TextGenerationChunk {
                        delta: String::new(),
                        done: true,
                        finish_reason: Some(finish_reason),
                        usage: Some(usage),
                        metadata: Default::default(),
                    })))
                    .await;
                stream_handle.purge().await;
            }
            .instrument(tracing::Span::current()),
        );

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.llama"))]
    async fn load_model(
        &self,
        request: Request<pb::ModelLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        debug!("llama load_model request received");
        let status = self.load_model_for_backend(BackendKind::Llama, request.into_inner()).await?;
        Ok(Response::new(status))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.llama"))]
    async fn unload_model(
        &self,
        request: Request<pb::ModelUnloadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        debug!("llama unload_model request received");
        let _ = request.into_inner();
        let status = self.unload_model_for_backend(BackendKind::Llama).await?;
        Ok(Response::new(status))
    }
}

fn estimate_token_count(text: &str) -> u32 {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0;
    }

    let bytes = trimmed.len() as u32;
    let whitespace_groups = trimmed.split_whitespace().count() as u32;
    let byte_estimate = bytes.div_ceil(4);
    byte_estimate.max(whitespace_groups).max(1)
}

fn finish_reason_from_token_budget(completion_tokens: u32, max_tokens: u32) -> String {
    if completion_tokens >= max_tokens && max_tokens > 0 {
        "length".to_owned()
    } else {
        "stop".to_owned()
    }
}

fn build_estimated_usage(prompt: &str, completion_tokens: u32) -> TextGenerationUsage {
    let prompt_tokens = estimate_token_count(prompt);

    TextGenerationUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens.saturating_add(completion_tokens),
        prompt_tokens_details: TextPromptTokensDetails::default(),
        estimated: true,
    }
}
