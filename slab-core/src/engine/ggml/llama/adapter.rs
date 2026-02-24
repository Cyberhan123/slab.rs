use crate::engine;
use slab_llama::{Llama, LlamaContextParams, LlamaModelParams};
use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::info;

use super::engine::LlamaInferenceEngine;
use super::{GGMLLlamaEngineError, SessionId, StreamChunk, StreamHandle};

#[derive(Debug)]
pub struct GGMLLlamaEngine {
    instance: Arc<Llama>,
    ineference_engine: Arc<Mutex<Option<LlamaInferenceEngine>>>,
}

// SAFETY: GGMLLlamaEngine is only accessed through Arc<Mutex<...>> for mutable state.
// The `instance: Arc<Llama>` field wraps a dynamically loaded library handle which is
// immutable after creation. Mutable lifecycle state (loaded engine handle)
// is guarded by the `engine: Arc<Mutex<...>>` field.
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
                source: source.into(),
            }
        })?;

        llama.backend_init();

        Ok(Self {
            instance: Arc::new(llama),
            ineference_engine: Arc::new(Mutex::new(None)),
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

        let mut engine_lock =
            self.ineference_engine
                .lock()
                .map_err(|_| GGMLLlamaEngineError::LockPoisoned {
                    operation: "lock llama engine state",
                })?;
        *engine_lock = None;

        let path = path_to_model
            .as_ref()
            .to_str()
            .ok_or(GGMLLlamaEngineError::InvalidModelPathUtf8)?;

        let model = Arc::new(
            self.instance
                .load_model_from_file(path, model_params)
                .map_err(|source| GGMLLlamaEngineError::LoadModel {
                    model_path: path.to_string(),
                    source: source.into(),
                })?,
        );

        let engine = LlamaInferenceEngine::start(num_workers, Arc::clone(&model), ctx_params)?;

        *engine_lock = Some(engine);
        Ok(())
    }

    fn require_engine(&self) -> Result<LlamaInferenceEngine, engine::EngineError> {
        let engine_lock: std::sync::MutexGuard<'_, Option<LlamaInferenceEngine>> = self
            .ineference_engine
            .lock()
            .map_err(|_| GGMLLlamaEngineError::LockPoisoned {
                operation: "lock llama engine state",
            })?;
        let engine = engine_lock
            .as_ref()
            .ok_or(GGMLLlamaEngineError::ModelNotLoaded)?;
        Ok(engine.clone())
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
    pub async fn cancel_generate(&self, session_id: SessionId) -> Result<(), engine::EngineError> {
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
                    stream_error = Some(GGMLLlamaEngineError::InferenceStreamError {
                        source: anyhow::anyhow!("stream error in session {sid}: {message}"),
                        message,
                    });
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
}
