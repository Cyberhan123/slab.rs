use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, instrument, warn, Instrument};

use slab_core::api::TextGenerationRequest;
use slab_proto::slab::ipc::v1 as pb;

use super::{extract_request_id, runtime_to_status, BackendKind, GrpcServiceImpl};

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
        let response = pipeline
            .run_text_generation(TextGenerationRequest {
                prompt: req.prompt,
                system_prompt: None,
                max_tokens: Some(req.max_tokens),
                temperature: Some(req.temperature),
                top_p: None,
                session_key: (!req.session_key.is_empty()).then_some(req.session_key),
                stream: false,
                options: Default::default(),
            })
            .await
            .map_err(|error| {
                error!(error = %error, "llama text generation failed");
                runtime_to_status(error)
            })?;

        info!(output_len = response.text.len(), "llama chat completed");
        Ok(Response::new(pb::ChatResponse {
            text: response.text,
        }))
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
        let backend_stream = pipeline
            .stream_text_generation(TextGenerationRequest {
                prompt: req.prompt,
                system_prompt: None,
                max_tokens: Some(req.max_tokens),
                temperature: Some(req.temperature),
                top_p: None,
                session_key: (!req.session_key.is_empty()).then_some(req.session_key),
                stream: true,
                options: Default::default(),
            })
            .await
            .map_err(|error| {
                error!(error = %error, "llama text generation stream setup failed");
                runtime_to_status(error)
            })?;

        let (tx, rx) = mpsc::channel(32);
        tokio::spawn(
            async move {
                tokio::pin!(backend_stream);
                let mut token_count = 0usize;
                while let Some(chunk) = backend_stream.next().await {
                    let message = match chunk {
                        Ok(chunk) => {
                            token_count += 1;
                            pb::ChatStreamChunk {
                                token: chunk.delta,
                                error: String::new(),
                                done: false,
                            }
                        }
                        Err(error) => {
                            warn!(error = %error, "error in llama stream chunk");
                            pb::ChatStreamChunk {
                                token: String::new(),
                                error: error.to_string(),
                                done: false,
                            }
                        }
                    };

                    if tx.send(Ok(message)).await.is_err() {
                        debug!("llama stream receiver dropped; stopping relay");
                        return;
                    }
                }

                debug!(token_count, "llama chat_stream relay finished");
                let _ = tx
                    .send(Ok(pb::ChatStreamChunk {
                        token: String::new(),
                        error: String::new(),
                        done: true,
                    }))
                    .await;
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
        let status = self
            .load_model_for_backend(BackendKind::Llama, request.into_inner())
            .await?;
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

    #[instrument(skip_all, fields(request_id, backend = "ggml.llama"))]
    async fn reload_library(
        &self,
        request: Request<pb::ReloadLibraryRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        debug!("llama reload_library request received");
        let status = self
            .reload_library_for_backend(BackendKind::Llama, request.into_inner())
            .await?;
        Ok(Response::new(status))
    }
}
