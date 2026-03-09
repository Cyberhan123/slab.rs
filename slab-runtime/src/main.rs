use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::Context;
use bytemuck::cast_slice;
use clap::Parser;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{info, warn};

use slab_core::api::Backend;
use slab_proto::slab::ipc::v1 as pb;

#[derive(Parser, Debug, Clone)]
#[command(name = "slab-runtime", version, about = "Slab gRPC runtime worker")]
struct Cli {
    #[arg(long = "grpc-bind", default_value = "127.0.0.1:50051")]
    grpc_bind: String,
    #[arg(long = "log")]
    log_level: Option<String>,
    #[arg(long = "log-json", action = clap::ArgAction::SetTrue)]
    log_json: bool,
    #[arg(long = "queue-capacity")]
    queue_capacity: Option<usize>,
    #[arg(long = "backend-capacity")]
    backend_capacity: Option<usize>,
    #[arg(long = "lib-dir")]
    lib_dir: Option<PathBuf>,
    #[arg(long = "enabled-backends")]
    enabled_backends: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct EnabledBackends {
    llama: bool,
    whisper: bool,
    diffusion: bool,
}

impl EnabledBackends {
    fn all() -> Self {
        Self {
            llama: true,
            whisper: true,
            diffusion: true,
        }
    }
}

fn parse_enabled_backends(raw: Option<&str>) -> anyhow::Result<EnabledBackends> {
    let Some(raw) = raw.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(EnabledBackends::all());
    };

    let mut enabled = EnabledBackends {
        llama: false,
        whisper: false,
        diffusion: false,
    };
    let mut unknown = Vec::new();

    for token in raw.split(',').map(str::trim).filter(|v| !v.is_empty()) {
        match token.to_ascii_lowercase().as_str() {
            "llama" | "ggml.llama" => enabled.llama = true,
            "whisper" | "ggml.whisper" => enabled.whisper = true,
            "diffusion" | "ggml.diffusion" => enabled.diffusion = true,
            other => unknown.push(other.to_string()),
        }
    }

    if !unknown.is_empty() {
        anyhow::bail!(
            "invalid enabled backends: {}. Supported: llama, whisper, diffusion",
            unknown.join(", ")
        );
    }
    if !enabled.llama && !enabled.whisper && !enabled.diffusion {
        anyhow::bail!("at least one backend must be enabled");
    }

    Ok(enabled)
}

fn init_tracing(log_level: &str, log_json: bool) {
    let env_filter = match tracing_subscriber::EnvFilter::try_from_default_env() {
        Ok(f) => f,
        Err(_) => match log_level.parse::<tracing_subscriber::EnvFilter>() {
            Ok(f) => f,
            Err(e) => {
                eprintln!(
                    "WARN: log level '{}' is invalid ({}); fallback to info",
                    log_level, e
                );
                tracing_subscriber::EnvFilter::new("info")
            }
        },
    };

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_thread_ids(true);
    if log_json {
        subscriber.json().init();
    } else {
        subscriber.init();
    }
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

    async fn load_model(
        &self,
        request: Request<pb::ModelLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let req = request.into_inner();
        if req.model_path.is_empty() {
            return Err(Status::invalid_argument("model_path must not be empty"));
        }
        if req.num_workers == 0 {
            return Err(Status::invalid_argument("num_workers must be at least 1"));
        }
        let backend = parse_backend(&req.backend_id)?;
        slab_core::api::backend(backend)
            .load_model()
            .input(slab_core::Payload::Json(serde_json::json!({
                "model_path": req.model_path,
                "num_workers": req.num_workers,
            })))
            .run()
            .await
            .map_err(runtime_to_status)?;

        Ok(Response::new(pb::ModelStatusResponse {
            backend: backend.to_string(),
            status: "loaded".to_string(),
        }))
    }

    async fn unload_model(
        &self,
        request: Request<pb::ModelUnloadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let req = request.into_inner();
        let backend = parse_backend(&req.backend_id)?;

        slab_core::api::backend(backend)
            .unload_model()
            .input(slab_core::Payload::default())
            .run()
            .await
            .map_err(runtime_to_status)?;

        Ok(Response::new(pb::ModelStatusResponse {
            backend: backend.to_string(),
            status: "unloaded".to_string(),
        }))
    }

    async fn reload_library(
        &self,
        request: Request<pb::ReloadLibraryRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let req = request.into_inner();
        if req.lib_path.is_empty() {
            return Err(Status::invalid_argument("lib_path must not be empty"));
        }
        if req.model_path.is_empty() {
            return Err(Status::invalid_argument("model_path must not be empty"));
        }
        if req.num_workers == 0 {
            return Err(Status::invalid_argument("num_workers must be at least 1"));
        }
        let backend = parse_backend(&req.backend_id)?;

        slab_core::api::reload_library(backend, &req.lib_path)
            .await
            .map_err(runtime_to_status)?;

        slab_core::api::backend(backend)
            .load_model()
            .input(slab_core::Payload::Json(serde_json::json!({
                "model_path": req.model_path,
                "num_workers": req.num_workers,
            })))
            .run()
            .await
            .map_err(runtime_to_status)?;

        Ok(Response::new(pb::ModelStatusResponse {
            backend: backend.to_string(),
            status: "loaded".to_string(),
        }))
    }
}

fn runtime_to_status(err: slab_core::RuntimeError) -> Status {
    match err {
        slab_core::RuntimeError::NotInitialized => Status::failed_precondition(err.to_string()),
        other => Status::internal(other.to_string()),
    }
}

fn parse_backend(raw: &str) -> Result<Backend, Status> {
    if raw.trim().is_empty() {
        return Err(Status::invalid_argument("backend_id must not be empty"));
    }
    Backend::from_str(raw)
        .map_err(|_| Status::invalid_argument(format!("unknown backend_id: {raw}")))
}

fn convert_to_pcm_f32le(payload: slab_core::Payload) -> Result<slab_core::Payload, String> {
    let path = payload
        .to_str()
        .map_err(|e| format!("invalid payload for preprocess: {e}"))?;
    let output = std::process::Command::new("ffmpeg")
        .arg("-i")
        .arg(path)
        .args([
            "-vn",
            "-f",
            "f32le",
            "-acodec",
            "pcm_f32le",
            "-ar",
            "16000",
            "-ac",
            "1",
            "-",
        ])
        .output()
        .map_err(|e| format!("ffmpeg start failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ffmpeg failed with status {}: {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }

    let pcm_bytes = output.stdout;
    if pcm_bytes.len() % std::mem::size_of::<f32>() != 0 {
        return Err(format!("PCM not aligned: {} bytes", pcm_bytes.len()));
    }

    let samples: Vec<f32> = cast_slice::<u8, f32>(&pcm_bytes).to_vec();
    Ok(slab_core::Payload::F32(std::sync::Arc::from(
        samples.as_slice(),
    )))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            warn!(error = %e, "failed to install CTRL+C signal handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(e) => warn!(error = %e, "failed to install SIGTERM handler"),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let log_level = cli.log_level.clone().unwrap_or_else(|| "info".to_owned());
    init_tracing(&log_level, cli.log_json);

    let enabled = parse_enabled_backends(cli.enabled_backends.as_deref())?;
    let base_lib_path = cli
        .lib_dir
        .as_deref()
        .unwrap_or(&Path::new("./resources/libs"));
    let llama_lib_dir = enabled.llama.then(|| base_lib_path.join("llama"));
    let whisper_lib_dir = enabled.whisper.then(|| base_lib_path.join("whisper"));
    let diffusion_lib_dir = enabled.diffusion.then(|| base_lib_path.join("diffusion"));

    slab_core::api::init(slab_core::api::Config {
        queue_capacity: cli.queue_capacity.unwrap_or(64),
        backend_capacity: cli.backend_capacity.unwrap_or(4),
        llama_lib_dir,
        whisper_lib_dir,
        diffusion_lib_dir,
    })
    .context("failed to initialize slab-core runtime")?;

    let addr = cli.grpc_bind.parse()?;
    info!(%addr, "slab-runtime gRPC listening");
    tonic::transport::Server::builder()
        .add_service(pb::backend_service_server::BackendServiceServer::new(
            BackendServiceImpl,
        ))
        .serve_with_shutdown(addr, shutdown_signal())
        .await?;
    Ok(())
}
