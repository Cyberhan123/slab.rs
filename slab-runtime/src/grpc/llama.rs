use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, instrument, warn, Instrument};

use slab_core::api::Backend;
use slab_proto::slab::ipc::v1 as pb;

use super::{
    extract_request_id, load_model_for_backend, reload_library_for_backend, runtime_to_status,
    unload_model_for_backend, GrpcServiceImpl,
};

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
        debug!(prompt_len = req.prompt.len(), max_tokens = req.max_tokens, "llama chat request received");

        let options = serde_json::json!({
            "max_tokens": req.max_tokens,
            "temperature": req.temperature,
            "session_key": if req.session_key.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::Value::String(req.session_key)
            }
        });

        let output = slab_core::api::backend(Backend::GGMLLlama)
            .inference()
            .input(slab_core::Payload::Text(req.prompt.into()))
            .options(slab_core::Payload::Json(options))
            .run_wait()
            .await
            .map_err(|e| {
                error!(error = %e, "llama inference failed");
                runtime_to_status(e)
            })?;

        let text = String::from_utf8(output.to_vec()).map_err(|e| {
            error!(error = %e, "backend returned invalid UTF-8");
            Status::internal(format!("backend returned invalid UTF-8: {e}"))
        })?;

        info!(output_len = text.len(), "llama chat completed");
        Ok(Response::new(pb::ChatResponse { text }))
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
        debug!(prompt_len = req.prompt.len(), max_tokens = req.max_tokens, "llama chat_stream request received");

        let options = serde_json::json!({
            "max_tokens": req.max_tokens,
            "temperature": req.temperature,
            "session_key": if req.session_key.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::Value::String(req.session_key)
            }
        });

        let backend_stream = slab_core::api::backend(Backend::GGMLLlama)
            .inference_stream()
            .input(slab_core::Payload::Text(req.prompt.into()))
            .options(slab_core::Payload::Json(options))
            .stream()
            .await
            .map_err(|e| {
                error!(error = %e, "llama inference_stream setup failed");
                runtime_to_status(e)
            })?;

        info!("llama chat_stream started; spawning token relay task");

        let (tx, rx) = mpsc::channel(32);
        tokio::spawn(
            async move {
                tokio::pin!(backend_stream);
                let mut token_count = 0usize;
                while let Some(chunk) = backend_stream.next().await {
                    let msg = match chunk {
                        Ok(bytes) => {
                            token_count += 1;
                            pb::ChatStreamChunk {
                                token: String::from_utf8_lossy(&bytes).into_owned(),
                                error: String::new(),
                                done: false,
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "error in llama stream chunk");
                            pb::ChatStreamChunk {
                                token: String::new(),
                                error: e.to_string(),
                                done: false,
                            }
                        }
                    };
                    if tx.send(Ok(msg)).await.is_err() {
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
        let status = load_model_for_backend(Backend::GGMLLlama, request.into_inner()).await?;
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
        let status = unload_model_for_backend(Backend::GGMLLlama).await?;
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
        let status = reload_library_for_backend(Backend::GGMLLlama, request.into_inner()).await?;
        Ok(Response::new(status))
    }
}
