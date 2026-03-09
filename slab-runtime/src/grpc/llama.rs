use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use slab_core::api::Backend;
use slab_proto::slab::ipc::v1 as pb;

use super::{
    load_model_for_backend, reload_library_for_backend, runtime_to_status,
    unload_model_for_backend, GrpcServiceImpl,
};

#[tonic::async_trait]
impl pb::llama_service_server::LlamaService for GrpcServiceImpl {
    async fn chat(
        &self,
        request: Request<pb::ChatRequest>,
    ) -> Result<Response<pb::ChatResponse>, Status> {
        let req = request.into_inner();
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
            .map_err(runtime_to_status)?;

        let text = String::from_utf8(output.to_vec())
            .map_err(|e| Status::internal(format!("backend returned invalid UTF-8: {e}")))?;
        Ok(Response::new(pb::ChatResponse { text }))
    }

    type ChatStreamStream = ReceiverStream<Result<pb::ChatStreamChunk, Status>>;

    async fn chat_stream(
        &self,
        request: Request<pb::ChatRequest>,
    ) -> Result<Response<Self::ChatStreamStream>, Status> {
        let req = request.into_inner();
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
            .map_err(runtime_to_status)?;

        let (tx, rx) = mpsc::channel(32);
        tokio::spawn(async move {
            tokio::pin!(backend_stream);
            while let Some(chunk) = backend_stream.next().await {
                let msg = match chunk {
                    Ok(bytes) => pb::ChatStreamChunk {
                        token: String::from_utf8_lossy(&bytes).into_owned(),
                        error: String::new(),
                        done: false,
                    },
                    Err(e) => pb::ChatStreamChunk {
                        token: String::new(),
                        error: e.to_string(),
                        done: false,
                    },
                };
                if tx.send(Ok(msg)).await.is_err() {
                    return;
                }
            }
            let _ = tx
                .send(Ok(pb::ChatStreamChunk {
                    token: String::new(),
                    error: String::new(),
                    done: true,
                }))
                .await;
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn load_model(
        &self,
        request: Request<pb::ModelLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let status = load_model_for_backend(Backend::GGMLLlama, request.into_inner()).await?;
        Ok(Response::new(status))
    }

    async fn unload_model(
        &self,
        _request: Request<pb::ModelUnloadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let status = unload_model_for_backend(Backend::GGMLLlama).await?;
        Ok(Response::new(status))
    }

    async fn reload_library(
        &self,
        request: Request<pb::ReloadLibraryRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let status = reload_library_for_backend(Backend::GGMLLlama, request.into_inner()).await?;
        Ok(Response::new(status))
    }
}
