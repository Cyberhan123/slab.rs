use crate::internal::engine;
use slab_llama::{ChatMessage, LlamaBatch, LlamaContextParams, LlamaModel, LlamaModelParams, LlamaToken};
use slab_llama::Llama;
use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::info;

use crate::base::error::CoreError;
use crate::internal::engine::tensor::Tensor;
use crate::internal::engine::traits::{CausalLM, ModelLoadConfig, ModelLoader};

use super::engine::LlamaInferenceEngine;
use super::{GGMLLlamaEngineError, SessionId, StreamChunk, StreamHandle};

#[derive(Debug)]
pub struct GGMLLlamaEngine {
    instance: Arc<Llama>,
    inference_engine: RwLock<Option<LlamaInferenceEngine>>,
    loaded_model: RwLock<Option<Arc<LlamaModel>>>,
}

// SAFETY: GGMLLlamaEngine is always owned through Arc<GGMLLlamaEngine> by backend workers.
// The `instance: Arc<Llama>` field wraps a dynamically loaded library handle which is
// immutable after creation. Mutable lifecycle state (loaded engine handle)
// is guarded by the `inference_engine: RwLock<...>` field.
unsafe impl Send for GGMLLlamaEngine {}
unsafe impl Sync for GGMLLlamaEngine {}

impl GGMLLlamaEngine {
    /// Resolve the final shared-library path and canonicalize it.
    ///
    /// Accepts either a directory containing the llama library or a direct path
    /// to the library file itself.
    fn resolve_lib_path<P: AsRef<Path>>(path: P) -> Result<PathBuf, engine::EngineError> {
        let llama_lib_name = format!("{}llama{}", DLL_PREFIX, DLL_SUFFIX);

        let mut lib_path = path.as_ref().to_path_buf();
        if lib_path.file_name() != Some(OsStr::new(&llama_lib_name)) {
            lib_path.push(&llama_lib_name);
        }

        std::fs::canonicalize(&lib_path).map_err(|source| {
            GGMLLlamaEngineError::CanonicalizeLibraryPath {
                path: lib_path,
                source,
            }
            .into()
        })
    }

    fn build_engine(normalized_path: &Path) -> Result<Self, engine::EngineError> {
        info!("current llama path is: {}", normalized_path.display());
        let llama = Llama::new(normalized_path).map_err(|source| {
            GGMLLlamaEngineError::InitializeDynamicLibrary {
                path: normalized_path.to_path_buf(),
                source,
            }
        })?;

        llama.backend_init();

        Ok(Self {
            instance: Arc::new(llama),
            inference_engine: RwLock::new(None),
            loaded_model: RwLock::new(None),
        })
    }

    /// Create a new engine from the library at `path` **without** registering
    /// any process-wide singleton.
    ///
    /// Call [`load_model_with_workers`] afterwards to load a model.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Arc<Self>, engine::EngineError> {
        let normalized = Self::resolve_lib_path(path)?;
        let engine = Self::build_engine(&normalized)?;
        Ok(Arc::new(engine))
    }

    /// Load a model and start a multi-worker inference engine.
    ///
    /// Any previously loaded model/engine are replaced.
    pub fn load_model_with_workers<P: AsRef<Path>>(
        &self,
        path_to_model: P,
        model_params: LlamaModelParams,
        ctx_params: LlamaContextParams,
        num_workers: usize,
    ) -> Result<(), engine::EngineError> {
        if num_workers == 0 {
            return Err(GGMLLlamaEngineError::InvalidWorkerCount { num_workers }.into());
        }

        let mut write_lock =
            self.inference_engine
                .write()
                .map_err(|_| GGMLLlamaEngineError::LockPoisoned {
                    operation: "lock llama engine state",
                })?;
        *write_lock = None;
        let mut model_write_lock =
            self.loaded_model
                .write()
                .map_err(|_| GGMLLlamaEngineError::LockPoisoned {
                    operation: "lock loaded llama model state",
                })?;
        *model_write_lock = None;

        let path = path_to_model
            .as_ref()
            .to_str()
            .ok_or(GGMLLlamaEngineError::InvalidModelPathUtf8)?;

        let model = Arc::new(
            self.instance
                .load_model_from_file(path, model_params)
                .map_err(|source| GGMLLlamaEngineError::LoadModel {
                    model_path: path.to_string(),
                    source,
                })?,
        );

        let engine = LlamaInferenceEngine::start(num_workers, Arc::clone(&model), ctx_params)?;

        *write_lock = Some(engine);
        *model_write_lock = Some(model);
        Ok(())
    }

    fn require_engine(&self) -> Result<LlamaInferenceEngine, engine::EngineError> {
        let read_lock: std::sync::RwLockReadGuard<'_, Option<LlamaInferenceEngine>> = self
            .inference_engine
            .read()
            .map_err(|_| GGMLLlamaEngineError::LockPoisoned {
                operation: "lock llama engine state",
            })?;
        let engine = read_lock
            .as_ref()
            .ok_or(GGMLLlamaEngineError::ModelNotLoaded)?;
        Ok(engine.clone())
    }

    fn require_model(&self) -> Result<Arc<LlamaModel>, engine::EngineError> {
        let read_lock =
            self.loaded_model
                .read()
                .map_err(|_| GGMLLlamaEngineError::LockPoisoned {
                    operation: "read loaded llama model state",
                })?;
        let model = read_lock
            .as_ref()
            .ok_or(GGMLLlamaEngineError::ModelNotLoaded)?;
        Ok(Arc::clone(model))
    }

    /// Apply the current model chat template to structured chat messages.
    pub fn apply_chat_template(
        &self,
        messages: &[ChatMessage],
        add_assistant_prompt: bool,
    ) -> Result<String, engine::EngineError> {
        let model = self.require_model()?;
        model
            .apply_chat_template(None, messages, add_assistant_prompt)
            .map_err(|source| GGMLLlamaEngineError::ApplyChatTemplate { source }.into())
    }

    /// Create a new session on the underlying inference engine.
    pub async fn create_session(&self) -> Result<SessionId, engine::EngineError> {
        let engine = self.require_engine()?;
        engine.create_session().await.map_err(Into::into)
    }

    /// Append text delta to an existing session.
    pub async fn append_input(
        &self,
        session_id: SessionId,
        text_delta: String,
    ) -> Result<(), engine::EngineError> {
        let engine = self.require_engine()?;
        engine
            .append_input(session_id, text_delta)
            .await
            .map_err(Into::into)
    }

    /// Start streaming generation for a session.
    pub async fn generate_stream(
        &self,
        session_id: SessionId,
        max_new_tokens: usize,
    ) -> Result<StreamHandle, engine::EngineError> {
        let engine = self.require_engine()?;
        engine
            .generate_stream(session_id, max_new_tokens)
            .await
            .map_err(Into::into)
    }

    /// End a session and release its KV entries.
    pub async fn end_session(&self, session_id: SessionId) -> Result<(), engine::EngineError> {
        let engine = self.require_engine()?;
        engine.end_session(session_id).await.map_err(Into::into)
    }

    /// Cancel active generation while keeping session KV state.
    ///
    /// Called from tests and available for future API callers via the backend dispatch path.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) async fn cancel_generate(
        &self,
        session_id: SessionId,
    ) -> Result<(), engine::EngineError> {
        let engine = self.require_engine()?;
        engine.cancel_generate(session_id).await.map_err(Into::into)
    }

    /// Generate text from a prompt by delegating to `LlamaInferenceEngine`.
    ///
    /// If `session_id` is `None`, creates a temporary session, appends the full prompt,
    /// consumes stream chunks until `Done`, and then ends the session.
    ///
    /// If `session_id` is `Some(sid)`, appends to the existing session and returns
    /// the output without ending the session (caller is responsible for cleanup).
    pub async fn inference(
        &self,
        prompt: &str,
        max_tokens: usize,
        session_id: Option<SessionId>,
    ) -> Result<String, engine::EngineError> {
        let sid = match session_id {
            Some(sid) => sid,
            None => self.create_session().await?,
        };
        let should_end = session_id.is_none();

        if let Err(error) = self.append_input(sid, prompt.to_string()).await {
            if should_end {
                let _ = self.end_session(sid).await;
            }
            return Err(error);
        }

        let mut stream = match self.generate_stream(sid, max_tokens).await {
            Ok(stream) => stream,
            Err(error) => {
                if should_end {
                    let _ = self.end_session(sid).await;
                }
                return Err(error);
            }
        };
        let mut output = String::new();
        let mut stream_error: Option<GGMLLlamaEngineError> = None;

        while let Some(chunk) = stream.recv().await {
            match chunk {
                StreamChunk::Token(piece) => output.push_str(&piece),
                StreamChunk::Done => break,
                StreamChunk::Error(message) => {
                    stream_error = Some(GGMLLlamaEngineError::InferenceStreamError { message });
                    break;
                }
            }
        }

        if should_end {
            let end_result = self.end_session(sid).await;
            if let Some(error) = stream_error {
                let _ = end_result;
                return Err(error.into());
            }
            end_result?;
        } else if let Some(error) = stream_error {
            return Err(error.into());
        }

        Ok(output)
    }

    /// Generate text from a prompt as an async stream.
    ///
    /// If `session_id` is `None`, creates a new temporary session and returns both
    /// the stream handle and the session ID (caller must end the session when done).
    ///
    /// If `session_id` is `Some(sid)`, appends to the existing session and returns
    /// the stream handle (caller is responsible for session management).
    pub async fn inference_stream(
        &self,
        prompt: &str,
        max_tokens: usize,
        session_id: Option<SessionId>,
    ) -> Result<(StreamHandle, SessionId), engine::EngineError> {
        let sid = match session_id {
            Some(sid) => sid,
            None => self.create_session().await?,
        };

        if let Err(error) = self.append_input(sid, prompt.to_string()).await {
            if session_id.is_none() {
                let _ = self.end_session(sid).await;
            }
            return Err(error);
        }

        let stream = match self.generate_stream(sid, max_tokens).await {
            Ok(stream) => stream,
            Err(error) => {
                if session_id.is_none() {
                    let _ = self.end_session(sid).await;
                }
                return Err(error);
            }
        };

        Ok((stream, sid))
    }

    /// Shared unload logic used by both the inherent method and the
    /// [`ModelLoader`] trait implementation.
    fn do_unload(&self) -> Result<(), GGMLLlamaEngineError> {
        let mut write_lock =
            self.inference_engine
                .write()
                .map_err(|_| GGMLLlamaEngineError::LockPoisoned {
                    operation: "lock llama engine state",
                })?;
        *write_lock = None;
        let mut model_write_lock =
            self.loaded_model
                .write()
                .map_err(|_| GGMLLlamaEngineError::LockPoisoned {
                    operation: "lock loaded llama model state",
                })?;
        *model_write_lock = None;
        Ok(())
    }

    /// Unload the current model and stop all inference workers.
    pub fn unload(&self) -> Result<(), engine::EngineError> {
        Ok(self.do_unload()?)
    }
}

// ── ModelLoader / CausalLM ────────────────────────────────────────────────────

/// Load-time configuration for the GGML LLaMA engine.
///
/// Used as `<GGMLLlamaEngine as ModelLoader>::LoadConfig`.
pub(crate) struct GgmlLlamaLoadConfig {
    /// Path to the GGUF model file.
    pub(crate) model_path: String,
    /// Number of parallel inference workers (≥ 1).
    ///
    /// Values below 1 are clamped to 1 during [`ModelLoader::load`].
    pub(crate) num_workers: usize,
    /// KV-cache context window in tokens; `0` means use the backend default.
    pub(crate) context_length: u32,
}

impl Default for GgmlLlamaLoadConfig {
    fn default() -> Self {
        Self {
            model_path: String::new(),
            num_workers: 1,
            context_length: 0,
        }
    }
}

impl ModelLoadConfig for GgmlLlamaLoadConfig {}

impl ModelLoader for GGMLLlamaEngine {
    type LoadConfig = GgmlLlamaLoadConfig;

    /// Load the GGML LLaMA model.
    ///
    /// Applies `context_length` to `n_ctx`, `n_batch`, and `n_ubatch` when
    /// non-zero, matching the behaviour of the backend worker's `model.load`
    /// handler.  `num_workers` is clamped to a minimum of 1.
    ///
    /// Replacement: `load_model_with_workers` clears both the inference engine
    /// and the loaded model handle before activating the new model, so any
    /// previously loaded model is safely replaced.
    fn load(&self, config: GgmlLlamaLoadConfig) -> Result<(), CoreError> {
        let mut ctx_params = LlamaContextParams::default();
        if config.context_length > 0 {
            let ctx_len = config.context_length;
            ctx_params.n_ctx = ctx_len;
            if ctx_params.n_batch > ctx_len {
                ctx_params.n_batch = ctx_len;
            }
            if ctx_params.n_ubatch > ctx_len {
                ctx_params.n_ubatch = ctx_len;
            }
        }
        let num_workers = config.num_workers.max(1);
        self.load_model_with_workers(
            &config.model_path,
            LlamaModelParams::default(),
            ctx_params,
            num_workers,
        )
    }

    /// Delegates to the shared `do_unload` helper to avoid code duplication
    /// with the inherent `unload` method.
    fn unload(&self) -> Result<(), CoreError> {
        Ok(self.do_unload()?)
    }

    fn is_loaded(&self) -> bool {
        self.loaded_model
            .read()
            .map(|g| g.is_some())
            .unwrap_or(false)
    }
}

impl CausalLM for GGMLLlamaEngine {
    /// Run a single stateless forward pass through the GGML LLaMA model.
    ///
    /// Creates a minimal temporary [`slab_llama::LlamaContext`] (KV cache sized
    /// just large enough for `input_ids`), decodes the batch, reads the logits
    /// for the last token via `get_logits_ith`, then drops the context.  This
    /// is deliberately stateless: no session or KV cache state persists between
    /// calls.
    fn forward(&self, input_ids: &Tensor) -> Result<Tensor, CoreError> {
        let token_ids = input_ids
            .as_token_ids()
            .ok_or_else(|| CoreError::UnsupportedOperation {
                backend: "ggml.llama".into(),
                op: "forward: expected token-ID (U32) tensor".into(),
            })?;

        if token_ids.is_empty() {
            return Err(CoreError::InvalidModelSpec {
                message: "forward requires at least one input token".into(),
            });
        }

        let model = self.require_model()?;

        let n = token_ids.len();

        // Create a minimal temporary context: KV cache just large enough to
        // hold the input tokens plus one generation slot.
        let ctx_len = (n as u32).saturating_add(1).max(8);
        let mut ctx_params = LlamaContextParams::default();
        ctx_params.n_ctx = ctx_len;
        if ctx_params.n_batch > ctx_len {
            ctx_params.n_batch = ctx_len;
        }
        if ctx_params.n_ubatch > ctx_len {
            ctx_params.n_ubatch = ctx_len;
        }

        let mut ctx = model
            .new_context(ctx_params)
            .map_err(|source| GGMLLlamaEngineError::CreateContext { source })?;

        // Sequence ID 0: this context is ephemeral, so any ID works.
        let mut batch = LlamaBatch::new(n);
        for (i, &tid) in token_ids.iter().enumerate() {
            batch
                .add(tid as LlamaToken, i as i32, &[0], i == n - 1)
                .expect("batch was created with capacity n and we add exactly n tokens");
        }

        ctx.decode(&mut batch)
            .map_err(|source| GGMLLlamaEngineError::TokenizeFailed { source })?;

        // Return logits for the last token in the batch.
        let logits = ctx.get_logits_ith((n - 1) as i32).to_vec();
        Ok(Tensor::from_logits(logits))
    }
}
