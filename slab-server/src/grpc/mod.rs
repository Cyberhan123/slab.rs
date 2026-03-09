pub mod client;

use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::info;

use slab_core::api::Backend;

use crate::routes::v1::audio::convert_to_pcm_f32le;

pub mod pb {
    tonic::include_proto!("slab.ipc.v1");
}

#[derive(Default)]
struct BackendServiceImpl;

#[tonic::async_trait]
impl pb::backend_service_server::BackendService for BackendServiceImpl {
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

    async fn transcribe(
        &self,
        request: Request<pb::TranscribeRequest>,
    ) -> Result<Response<pb::TranscribeResponse>, Status> {
        let req = request.into_inner();
        if req.path.is_empty() {
            return Err(Status::invalid_argument("audio file path is empty"));
        }

        let output = slab_core::api::backend(Backend::GGMLWhisper)
            .inference()
            .input(slab_core::Payload::Text(req.path.into()))
            .preprocess("ffmpeg.to_pcm_f32le", convert_to_pcm_f32le)
            .run_wait()
            .await
            .map_err(runtime_to_status)?;

        Ok(Response::new(pb::TranscribeResponse {
            text: String::from_utf8_lossy(&output).into_owned(),
        }))
    }

    async fn generate_image(
        &self,
        request: Request<pb::ImageRequest>,
    ) -> Result<Response<pb::ImageResponse>, Status> {
        let req = request.into_inner();
        if req.prompt.is_empty() {
            return Err(Status::invalid_argument("prompt must not be empty"));
        }

        let payload = serde_json::json!({
            "prompt": req.prompt,
            "n": req.n,
            "size": req.size,
            "model": req.model,
        });

        let output = slab_core::api::backend(Backend::GGMLDiffusion)
            .inference()
            .input(slab_core::Payload::Json(payload))
            .run_wait()
            .await
            .map_err(runtime_to_status)?;

        Ok(Response::new(pb::ImageResponse {
            image: output.to_vec(),
        }))
    }
}

fn runtime_to_status(err: slab_core::RuntimeError) -> Status {
    match err {
        slab_core::RuntimeError::NotInitialized => Status::failed_precondition(err.to_string()),
        other => Status::internal(other.to_string()),
    }
}

pub async fn serve(bind_address: String) -> anyhow::Result<()> {
    let addr = bind_address.parse()?;
    info!(%addr, "gRPC server listening");
    tonic::transport::Server::builder()
        .add_service(pb::backend_service_server::BackendServiceServer::new(
            BackendServiceImpl,
        ))
        .serve(addr)
        .await?;
    Ok(())
}
