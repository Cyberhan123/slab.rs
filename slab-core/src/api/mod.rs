//! Public-facing API facade for slab-core.
//!
//! All user code should only `use slab_core::api;` – the underlying
//! [`Orchestrator`], [`ResourceManager`] and channel types remain private.
//!
//! # Quick-start
//!
//! ```rust,no_run
//! use slab_core::api;
//! use bytes::Bytes;
//!
//! # #[tokio::main]
//! # async fn main() {
//! // 1. Initialize once at process start (no model loaded yet).
//! api::init(api::Config::default()).unwrap();
//!
//! // 2. Load a model (example: llama).
//! let cfg = serde_json::json!({
//!     "lib_path": "/usr/local/lib/libllama.so",
//!     "model_path": "/models/qwen.gguf",
//!     "num_workers": 1
//! });
//! api::backend("ggml.llama")
//!     .op("model.load")
//!     .input(Bytes::from(cfg.to_string()))
//!     .run_wait()
//!     .await
//!     .unwrap();
//!
//! // 3. Whisper unary transcription.
//! let audio_bytes: Bytes = Bytes::new(); // load PCM f32 bytes
//! let result = api::backend("ggml.whisper")
//!     .op("transcribe")
//!     .input(audio_bytes)
//!     .run_wait()
//!     .await
//!     .unwrap();
//! println!("{}", String::from_utf8_lossy(&result));
//!
//! // 4. Llama streaming generation.
//! use futures::StreamExt;
//! let mut stream = api::backend("ggml.llama")
//!     .op("generate.stream")
//!     .input(Bytes::from("Hello, world!"))
//!     .stream()
//!     .await
//!     .unwrap();
//! while let Some(chunk) = stream.next().await {
//!     let bytes = chunk.unwrap();
//!     print!("{}", String::from_utf8_lossy(&bytes));
//! }
//! # }
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures::Stream;
use tokio::sync::mpsc;

use crate::runtime::backend::admission::ResourceManager;
use crate::runtime::backend::protocol::{BackendOp, BackendRequest, StreamChunk};
use crate::runtime::orchestrator::Orchestrator;
use crate::runtime::pipeline::PipelineBuilder;
use crate::runtime::types::{Payload, RuntimeError, TaskId, TaskStatus};

// ── Global runtime ─────────────────────────────────────────────────────────────

/// Holds the live runtime state after [`init`] is called.
pub(crate) struct ApiRuntime {
    pub(crate) orchestrator: Orchestrator,
    pub(crate) backends: HashMap<String, mpsc::Sender<BackendRequest>>,
}

static RUNTIME: std::sync::OnceLock<ApiRuntime> = std::sync::OnceLock::new();

// ── Configuration ──────────────────────────────────────────────────────────────

/// Configuration passed to [`init`].
///
/// All fields have sensible defaults via [`Default`].
#[derive(Debug, Clone)]
pub struct Config {
    /// Capacity of the orchestrator submission queue.  Defaults to `64`.
    pub queue_capacity: usize,
    /// Maximum concurrent in-flight requests per backend.  Defaults to `4`.
    pub backend_capacity: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            queue_capacity: 64,
            backend_capacity: 4,
        }
    }
}

// ── Initialization ─────────────────────────────────────────────────────────────

/// Initialize the API runtime.
///
/// Registers the three ggml backends (`ggml.llama`, `ggml.whisper`,
/// `ggml.diffusion`) and starts their worker tasks.  No model is loaded at
/// this point; call `model.load` via [`backend`] to load one.
///
/// Must be called inside a Tokio runtime.  Calling it a second time is a
/// no-op — the first configuration wins.
///
/// # Errors
///
/// Returns [`RuntimeError::NotInitialized`] only when the internal
/// `OnceLock` has already been set with an incompatible value (shouldn't
/// happen in normal usage).
pub fn init(config: Config) -> Result<(), RuntimeError> {
    let rm = ResourceManager::new();
    rm.register_backend("ggml.llama", config.backend_capacity);
    rm.register_backend("ggml.whisper", config.backend_capacity);
    rm.register_backend("ggml.diffusion", config.backend_capacity);

    let orchestrator = Orchestrator::start(rm, config.queue_capacity);

    let llama_tx = crate::engine::ggml::llama::spawn_backend(128);
    let whisper_tx = crate::engine::ggml::whisper::spawn_backend(128);
    let diffusion_tx = crate::engine::ggml::diffusion::spawn_backend(128);

    let mut backends = HashMap::new();
    backends.insert("ggml.llama".to_owned(), llama_tx);
    backends.insert("ggml.whisper".to_owned(), whisper_tx);
    backends.insert("ggml.diffusion".to_owned(), diffusion_tx);

    // set() is a no-op if already initialized — idempotent.
    let _ = RUNTIME.set(ApiRuntime {
        orchestrator,
        backends,
    });
    Ok(())
}

// ── Builder entry point ────────────────────────────────────────────────────────

/// Start building a call to the named backend.
///
/// Known backend ids: `"ggml.llama"`, `"ggml.whisper"`, `"ggml.diffusion"`.
///
/// Errors surface at the terminal step (`.run()`, `.run_wait()`, `.stream()`).
pub fn backend(id: &str) -> BackendBuilder {
    BackendBuilder { id: id.to_owned() }
}

// ── BackendBuilder ─────────────────────────────────────────────────────────────

/// Selects a backend; produced by [`backend`].
pub struct BackendBuilder {
    id: String,
}

impl BackendBuilder {
    /// Select the operation to invoke on the backend.
    ///
    /// Standard op names:
    /// - `"model.load"` – load (or reload) a model; params in `input: Bytes`
    /// - `"generate"` – unary text generation (llama)
    /// - `"generate.stream"` – streaming text generation (llama)
    /// - `"transcribe"` – speech-to-text (whisper); input is raw PCM `f32` bytes
    /// - `"generate_image"` – image generation (diffusion); input is JSON bytes
    pub fn op(self, name: &str) -> CallBuilder {
        CallBuilder {
            backend_id: self.id,
            op_name: name.to_owned(),
            op_options: serde_json::Value::Object(Default::default()),
            input: Bytes::new(),
        }
    }
}

// ── CallBuilder ────────────────────────────────────────────────────────────────

/// Configures and submits a single-stage backend call.
///
/// Produced by [`BackendBuilder::op`].  All terminal methods consume `self`.
pub struct CallBuilder {
    backend_id: String,
    op_name: String,
    op_options: serde_json::Value,
    input: Bytes,
}

impl CallBuilder {
    /// Attach the input payload (replaces any previous `input` call).
    pub fn input(mut self, data: Bytes) -> Self {
        self.input = data;
        self
    }

    /// Attach JSON options for the op (replaces any previous `options` call).
    ///
    /// Most parameters should travel via [`input`]; `options` is for
    /// small structural hints and is kept as `{}` by default.
    pub fn options(mut self, opts: serde_json::Value) -> Self {
        self.op_options = opts;
        self
    }

    // ── helpers ──────────────────────────────────────────────────────────────

    fn runtime() -> Result<&'static ApiRuntime, RuntimeError> {
        RUNTIME.get().ok_or(RuntimeError::NotInitialized)
    }

    fn ingress_tx(rt: &ApiRuntime, backend_id: &str) -> Result<mpsc::Sender<BackendRequest>, RuntimeError> {
        rt.backends
            .get(backend_id)
            .cloned()
            .ok_or_else(|| RuntimeError::Busy {
                backend_id: backend_id.to_owned(),
            })
    }

    // ── terminal methods ─────────────────────────────────────────────────────

    /// Submit the call and return a [`TaskId`] without waiting for completion.
    ///
    /// Use the orchestrator or [`run_wait`](Self::run_wait) to obtain the result.
    pub async fn run(self) -> Result<TaskId, RuntimeError> {
        let rt = Self::runtime()?;
        let ingress_tx = Self::ingress_tx(rt, &self.backend_id)?;
        let payload = Payload::Bytes(Arc::from(self.input.as_ref()));
        let op = BackendOp {
            name: self.op_name.clone(),
            options: self.op_options,
        };
        PipelineBuilder::new(rt.orchestrator.clone(), payload)
            .gpu(self.op_name, self.backend_id, op, ingress_tx)
            .run()
            .await
    }

    /// Submit the call and block until the result is available.
    ///
    /// Default timeout is 300 seconds; use
    /// [`run_wait_timeout`](Self::run_wait_timeout) for a custom deadline.
    pub async fn run_wait(self) -> Result<Bytes, RuntimeError> {
        self.run_wait_timeout(Duration::from_secs(300)).await
    }

    /// Submit the call and block until the result is available or `timeout`
    /// elapses.
    ///
    /// Returns [`RuntimeError::Timeout`] on deadline expiry.
    pub async fn run_wait_timeout(self, timeout: Duration) -> Result<Bytes, RuntimeError> {
        let rt = Self::runtime()?;
        let ingress_tx = Self::ingress_tx(rt, &self.backend_id)?;
        let payload = Payload::Bytes(Arc::from(self.input.as_ref()));
        let op = BackendOp {
            name: self.op_name.clone(),
            options: self.op_options,
        };

        let task_id = PipelineBuilder::new(rt.orchestrator.clone(), payload)
            .gpu(self.op_name, self.backend_id, op, ingress_tx)
            .run()
            .await?;

        // Poll until the task reaches a terminal state.
        tokio::time::timeout(timeout, async {
            loop {
                match rt.orchestrator.get_status(task_id).await {
                    Err(e) => return Err(e),
                    Ok(view) => match view.status {
                        TaskStatus::Succeeded { .. } => return Ok(()),
                        TaskStatus::Failed { error } => return Err(error),
                        TaskStatus::Cancelled => return Err(RuntimeError::BackendShutdown),
                        _ => tokio::time::sleep(Duration::from_millis(5)).await,
                    },
                }
            }
        })
        .await
        .map_err(|_| RuntimeError::Timeout)??;

        // Extract result bytes.
        let result = rt
            .orchestrator
            .get_result(task_id)
            .await
            .ok_or(RuntimeError::TaskNotFound { task_id })?;

        payload_to_bytes(result)
    }

    /// Submit the call as a **streaming** terminal stage and return a [`Stream`].
    ///
    /// This is always the last step in the call chain — it consumes `self` and
    /// the returned stream yields `Result<Bytes, RuntimeError>` items as the
    /// backend produces them.
    ///
    /// Awaiting this method submits the pipeline and waits (up to 30 s) for
    /// the backend to open the stream, then returns the live stream handle.
    pub async fn stream(
        self,
    ) -> Result<impl Stream<Item = Result<Bytes, RuntimeError>>, RuntimeError> {
        let rt = Self::runtime()?;
        let ingress_tx = Self::ingress_tx(rt, &self.backend_id)?;
        let payload = Payload::Bytes(Arc::from(self.input.as_ref()));
        let op = BackendOp {
            name: self.op_name.clone(),
            options: self.op_options,
        };

        let task_id = PipelineBuilder::new(rt.orchestrator.clone(), payload)
            .gpu_stream(self.op_name, self.backend_id, op, ingress_tx)
            .run_stream()
            .await?;

        // Wait for the backend to open the stream (task → SucceededStreaming).
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                match rt.orchestrator.get_status(task_id).await {
                    Err(e) => return Err(e),
                    Ok(view) => match view.status {
                        TaskStatus::SucceededStreaming => return Ok(()),
                        TaskStatus::Failed { error } => return Err(error),
                        _ => tokio::time::sleep(Duration::from_millis(5)).await,
                    },
                }
            }
        })
        .await
        .map_err(|_| RuntimeError::Timeout)??;

        let handle = rt
            .orchestrator
            .take_stream(task_id)
            .await
            .ok_or(RuntimeError::TaskNotFound { task_id })?;

        // Convert the mpsc::Receiver<StreamChunk> into a Stream<Item=Result<Bytes>>.
        let stream = futures::stream::unfold(handle, |mut rx| async move {
            match rx.recv().await {
                None | Some(StreamChunk::Done) => None,
                Some(StreamChunk::Token(t)) => Some((Ok(Bytes::from(t)), rx)),
                Some(StreamChunk::Error(msg)) => Some((
                    Err(RuntimeError::GpuStageFailed {
                        stage_name: "stream".into(),
                        message: msg,
                    }),
                    rx,
                )),
            }
        });

        Ok(stream)
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn payload_to_bytes(p: Payload) -> Result<Bytes, RuntimeError> {
    match p {
        Payload::Bytes(b) => Ok(Bytes::copy_from_slice(&b)),
        Payload::Text(s) => Ok(Bytes::from(s.as_bytes().to_vec())),
        Payload::F32(v) => {
            let byte_len = v.len() * std::mem::size_of::<f32>();
            let src = unsafe {
                // SAFETY: f32 is valid for any bit pattern; we copy to owned vec.
                std::slice::from_raw_parts(v.as_ptr() as *const u8, byte_len)
            };
            Ok(Bytes::copy_from_slice(src))
        }
        _ => Err(RuntimeError::GpuStageFailed {
            stage_name: "result".into(),
            message: "unsupported payload type for Bytes conversion".into(),
        }),
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::backend::protocol::BackendReply;
    use tokio::sync::mpsc;

    // ── helpers ───────────────────────────────────────────────────────────────

    /// Spawn a simple echo backend: returns the input payload unchanged.
    fn spawn_echo_backend(capacity: usize) -> mpsc::Sender<BackendRequest> {
        let (tx, mut rx) = mpsc::channel::<BackendRequest>(capacity);
        tokio::spawn(async move {
            while let Some(req) = rx.recv().await {
                let _ = req.reply_tx.send(BackendReply::Value(req.input));
            }
        });
        tx
    }

    /// Spawn a mock streaming backend that emits the given token strings then Done.
    fn spawn_stream_backend(
        capacity: usize,
        tokens: Vec<&'static str>,
    ) -> mpsc::Sender<BackendRequest> {
        let (tx, mut rx) = mpsc::channel::<BackendRequest>(capacity);
        tokio::spawn(async move {
            while let Some(req) = rx.recv().await {
                let (stream_tx, stream_rx) = mpsc::channel::<StreamChunk>(16);
                let _ = req.reply_tx.send(BackendReply::Stream(stream_rx));
                for t in &tokens {
                    let _ = stream_tx
                        .send(StreamChunk::Token(t.to_string()))
                        .await;
                }
                let _ = stream_tx.send(StreamChunk::Done).await;
            }
        });
        tx
    }

    /// Build an isolated orchestrator + backend pair without the global RUNTIME.
    fn make_orchestrator_with_backend(
        backend_id: &str,
        ingress_tx: mpsc::Sender<BackendRequest>,
    ) -> Orchestrator {
        let rm = ResourceManager::new();
        rm.register_backend(backend_id, 4);
        let orch = Orchestrator::start(rm, 64);
        // Store ingress_tx in an ApiRuntime-like structure; use directly below.
        let _ = ingress_tx; // returned to caller; passed into PipelineBuilder
        orch
    }

    // ── Tests: mock unary backend ─────────────────────────────────────────────

    /// Verify that a mock echo backend receives the request and returns the
    /// input bytes through the orchestrator pipeline.
    #[tokio::test]
    async fn mock_unary_backend_echo() {
        let echo_tx = spawn_echo_backend(16);

        let rm = ResourceManager::new();
        rm.register_backend("test.echo", 4);
        let orchestrator = Orchestrator::start(rm, 64);

        let op = BackendOp {
            name: "echo".to_owned(),
            options: serde_json::Value::Null,
        };

        let task_id = PipelineBuilder::new(
            orchestrator.clone(),
            Payload::Bytes(Arc::from(b"hello" as &[u8])),
        )
        .gpu("echo", "test.echo", op, echo_tx)
        .run()
        .await
        .expect("submit should succeed");

        // Poll until completion.
        let status = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let view = orchestrator
                    .get_status(task_id)
                    .await
                    .expect("task should exist");
                match view.status {
                    TaskStatus::Succeeded { .. } | TaskStatus::Failed { .. } => {
                        break view.status
                    }
                    _ => tokio::time::sleep(std::time::Duration::from_millis(10)).await,
                }
            }
        })
        .await
        .expect("should complete within timeout");

        assert!(matches!(status, TaskStatus::Succeeded { .. }));

        let payload = orchestrator
            .get_result(task_id)
            .await
            .expect("result should be present");
        if let Payload::Bytes(b) = payload {
            assert_eq!(&*b, b"hello");
        } else {
            panic!("unexpected payload variant");
        }
    }

    // ── Tests: mock stream backend ────────────────────────────────────────────

    /// Verify that a mock streaming backend emits the expected tokens through
    /// the orchestrator, and that collecting them yields the full string.
    #[tokio::test]
    async fn mock_stream_backend_collects_tokens() {
        let stream_tx = spawn_stream_backend(16, vec!["foo", " ", "bar"]);

        let rm = ResourceManager::new();
        rm.register_backend("test.stream", 4);
        let orchestrator = Orchestrator::start(rm, 64);

        let op = BackendOp {
            name: "gen".to_owned(),
            options: serde_json::Value::Null,
        };

        let task_id = PipelineBuilder::new(
            orchestrator.clone(),
            Payload::Bytes(Arc::from(b"prompt" as &[u8])),
        )
        .gpu_stream("gen", "test.stream", op, stream_tx)
        .run_stream()
        .await
        .expect("submit should succeed");

        // Wait for SucceededStreaming.
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let view = orchestrator.get_status(task_id).await.unwrap();
                if matches!(view.status, TaskStatus::SucceededStreaming) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("task should reach SucceededStreaming");

        let mut handle = orchestrator.take_stream(task_id).await.unwrap();
        let mut tokens = String::new();
        while let Some(chunk) = handle.recv().await {
            match chunk {
                StreamChunk::Token(t) => tokens.push_str(&t),
                StreamChunk::Done => break,
                StreamChunk::Error(e) => panic!("unexpected error: {e}"),
            }
        }
        assert_eq!(tokens, "foo bar");
    }

    // ── Tests: model not loaded ───────────────────────────────────────────────

    /// Verify that a backend that always returns "model not loaded" causes the
    /// task to transition to Failed.
    #[tokio::test]
    async fn backend_error_model_not_loaded() {
        let (ingress_tx, mut rx) = mpsc::channel::<BackendRequest>(16);
        tokio::spawn(async move {
            while let Some(req) = rx.recv().await {
                let _ = req
                    .reply_tx
                    .send(BackendReply::Error("model not loaded".to_owned()));
            }
        });

        let rm = ResourceManager::new();
        rm.register_backend("test.notloaded", 4);
        let orchestrator = Orchestrator::start(rm, 64);

        let op = BackendOp {
            name: "generate".to_owned(),
            options: serde_json::Value::Null,
        };

        let task_id = PipelineBuilder::new(
            orchestrator.clone(),
            Payload::Bytes(Arc::from(b"test" as &[u8])),
        )
        .gpu("generate", "test.notloaded", op, ingress_tx)
        .run()
        .await
        .expect("submit ok");

        let status = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let view = orchestrator.get_status(task_id).await.unwrap();
                match view.status {
                    TaskStatus::Failed { .. } | TaskStatus::Succeeded { .. } => {
                        break view.status
                    }
                    _ => tokio::time::sleep(std::time::Duration::from_millis(10)).await,
                }
            }
        })
        .await
        .expect("should fail quickly");

        assert!(
            matches!(status, TaskStatus::Failed { .. }),
            "expected Failed status when model not loaded, got {status:?}"
        );
    }

    // ── Tests: payload_to_bytes ───────────────────────────────────────────────

    #[test]
    fn payload_bytes_roundtrip() {
        let data = b"hello world";
        let p = Payload::Bytes(Arc::from(data as &[u8]));
        let b = payload_to_bytes(p).unwrap();
        assert_eq!(&b[..], data);
    }

    #[test]
    fn payload_text_to_bytes() {
        let p = Payload::Text(Arc::from("hello"));
        let b = payload_to_bytes(p).unwrap();
        assert_eq!(&b[..], b"hello");
    }
}
