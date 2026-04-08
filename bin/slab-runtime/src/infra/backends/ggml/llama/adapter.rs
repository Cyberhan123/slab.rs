use slab_runtime_core::backend::{StreamChunk as BaseStreamChunk, StreamHandle as BaseStreamHandle};
use crate::infra::backends::ggml;
use slab_llama::Llama;
use slab_llama::{
    ChatMessage, LlamaContextParams, LlamaModel, LlamaModelParams, LlamaRuntime,
    LlamaSessionSnapshot,
};
use slab_types::inference::{TextGenerationUsage, TextPromptTokensDetails};
use slab_utils::loader::load_library_from_dir;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use tokio::sync::{Mutex, mpsc, watch};
use tracing::{info, warn};

use super::{GGMLLlamaEngineError, SessionId, StreamChunk, StreamHandle};

#[derive(Debug, Clone)]
pub(crate) struct LlamaDispatchRequest {
    pub prompt: String,
    pub max_tokens: usize,
    pub session_key: Option<String>,
    pub apply_chat_template: bool,
    pub chat_messages: Vec<ChatMessage>,
    pub grammar: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct LlamaDispatchOutput {
    pub text: String,
    pub usage: Option<TextGenerationUsage>,
}

#[derive(Debug, Clone)]
enum SessionBinding {
    Ready { snapshot: LlamaSessionSnapshot, cached_prompt: String, grammar: Option<String> },
    Busy,
}

#[derive(Debug, Clone)]
enum SessionReusePlan {
    CreateFresh { delta_prompt: String, cached_tokens: u32 },
    RestoreSnapshot { snapshot: LlamaSessionSnapshot, delta_prompt: String, cached_tokens: u32 },
}

#[derive(Debug)]
struct PreparedSession {
    key: Option<String>,
    sid: Option<SessionId>,
    delta_prompt: String,
    full_prompt: String,
    cached_tokens: u32,
}

struct ParsedChatPrompt {
    messages: Vec<ChatMessage>,
    add_assistant_prompt: bool,
}

fn parse_role_prefixed_chat_prompt(prompt: &str) -> Option<ParsedChatPrompt> {
    if !(prompt.contains("User:") || prompt.contains("Assistant:") || prompt.contains("System:")) {
        return None;
    }

    let mut messages: Vec<ChatMessage> = Vec::new();
    for raw_line in prompt.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            continue;
        }
        let (raw_role, raw_content) = line.split_once(':')?;
        let role = raw_role.trim().to_ascii_lowercase();
        if !matches!(role.as_str(), "system" | "user" | "assistant") {
            return None;
        }
        messages.push(ChatMessage { role, content: raw_content.trim_start().to_owned() });
    }

    if messages.is_empty() {
        return None;
    }

    let mut add_assistant_prompt = false;
    if let Some(last) = messages.last() {
        if last.role == "assistant" && last.content.is_empty() {
            let _ = messages.pop();
            add_assistant_prompt = true;
        } else if last.role != "assistant" {
            add_assistant_prompt = true;
        }
    }

    if messages.is_empty() && !add_assistant_prompt {
        return None;
    }

    Some(ParsedChatPrompt { messages, add_assistant_prompt })
}

pub(super) fn infer_add_assistant_prompt(messages: &[ChatMessage]) -> bool {
    messages.last().map(|message| message.role != "assistant").unwrap_or(true)
}

fn plan_session_reuse(
    key: &str,
    existing: Option<&SessionBinding>,
    full_prompt: &str,
    grammar: Option<&str>,
) -> Result<SessionReusePlan, GGMLLlamaEngineError> {
    match existing {
        None => Ok(SessionReusePlan::CreateFresh {
            delta_prompt: full_prompt.to_owned(),
            cached_tokens: 0,
        }),
        Some(SessionBinding::Busy) => {
            Err(GGMLLlamaEngineError::SessionKeyBusy { key: key.to_owned() })
        }
        Some(SessionBinding::Ready { snapshot, cached_prompt, grammar: cached_grammar }) => {
            if cached_grammar.as_deref() != grammar {
                return Ok(SessionReusePlan::CreateFresh {
                    delta_prompt: full_prompt.to_owned(),
                    cached_tokens: 0,
                });
            }

            match full_prompt.strip_prefix(cached_prompt) {
                Some("") | None => Ok(SessionReusePlan::CreateFresh {
                    delta_prompt: full_prompt.to_owned(),
                    cached_tokens: 0,
                }),
                Some(delta_prompt) => Ok(SessionReusePlan::RestoreSnapshot {
                    snapshot: snapshot.clone(),
                    delta_prompt: delta_prompt.to_owned(),
                    cached_tokens: snapshot.n_past.max(0) as u32,
                }),
            }
        }
    }
}

#[derive(Debug)]
pub struct GGMLLlamaEngine {
    instance: Arc<Llama>,
    inference_engine: RwLock<Option<LlamaRuntime>>,
    loaded_model: RwLock<Option<Arc<LlamaModel>>>,
    explicit_chat_template: RwLock<Option<String>>,
    session_bindings: Mutex<HashMap<String, SessionBinding>>,
}

// SAFETY: GGMLLlamaEngine is always owned through Arc<GGMLLlamaEngine> by backend workers.
// The `instance: Arc<Llama>` field wraps a dynamically loaded library handle which is
// immutable after creation. Mutable lifecycle state (loaded engine handle)
// is guarded by the `inference_engine: RwLock<...>` field.
unsafe impl Send for GGMLLlamaEngine {}
unsafe impl Sync for GGMLLlamaEngine {}

impl GGMLLlamaEngine {
    /// Create a new engine from the shared runtime library directory at `path`
    /// **without** registering any process-wide singleton.
    ///
    /// Call [`load_model_with_workers`] afterwards to load a model.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Arc<Self>, ggml::EngineError> {
        load_library_from_dir(path, "llama", |lib_dir, llama_path| {
            info!("current llama path is: {}", llama_path.display());
            let llama = Llama::new(lib_dir).map_err(|source| {
                GGMLLlamaEngineError::InitializeDynamicLibrary {
                    path: llama_path.to_path_buf(),
                    source,
                }
            })?;

            llama.backend_init();

            // SAFETY: `Llama` wraps `Arc<slab_llama_sys::LlamaLib>` — a dlopen2-generated
            // handle that holds a read-only table of function pointers loaded once at startup.
            // After `Llama::new` returns the function pointer table is never mutated, making
            // concurrent reads from multiple threads safe. No other mutable state is stored
            // directly on `Llama`; all mutable engine state (`inference_engine`, `loaded_model`)
            // is guarded by `RwLock` on the enclosing `GGMLLlamaEngine`. The `GGMLLlamaEngine`
            // struct therefore satisfies the `Send + Sync` contract, which is asserted explicitly
            // via the `unsafe impl` declarations above this block.
            #[allow(clippy::arc_with_non_send_sync)]
            Ok(Arc::new(Self {
                instance: Arc::new(llama),
                inference_engine: RwLock::new(None),
                loaded_model: RwLock::new(None),
                explicit_chat_template: RwLock::new(None),
                session_bindings: Mutex::new(HashMap::new()),
            }))
        })
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
        chat_template: Option<String>,
    ) -> Result<(), ggml::EngineError> {
        if num_workers == 0 {
            return Err(GGMLLlamaEngineError::InvalidWorkerCount { num_workers }.into());
        }

        let mut write_lock = self.inference_engine.write().map_err(|_| {
            GGMLLlamaEngineError::LockPoisoned { operation: "lock llama engine state" }
        })?;
        *write_lock = None;
        let mut model_write_lock = self.loaded_model.write().map_err(|_| {
            GGMLLlamaEngineError::LockPoisoned { operation: "lock loaded llama model state" }
        })?;
        *model_write_lock = None;
        let mut template_write_lock = self.explicit_chat_template.write().map_err(|_| {
            GGMLLlamaEngineError::LockPoisoned {
                operation: "lock explicit llama chat template state",
            }
        })?;
        *template_write_lock = None;
        self.session_bindings.blocking_lock().clear();

        let path =
            path_to_model.as_ref().to_str().ok_or(GGMLLlamaEngineError::InvalidModelPathUtf8)?;

        let model =
            Arc::new(self.instance.load_model_from_file(path, model_params).map_err(|source| {
                GGMLLlamaEngineError::LoadModel { model_path: path.to_string(), source }
            })?);

        let engine = LlamaRuntime::start(num_workers, Arc::clone(&model), ctx_params)
            .map_err(GGMLLlamaEngineError::from)?;

        *write_lock = Some(engine);
        *model_write_lock = Some(model);
        *template_write_lock = chat_template.filter(|value| !value.trim().is_empty());
        Ok(())
    }

    fn explicit_chat_template(&self) -> Result<Option<String>, ggml::EngineError> {
        let read_lock =
            self.explicit_chat_template.read().map_err(|_| GGMLLlamaEngineError::LockPoisoned {
                operation: "read explicit llama chat template state",
            })?;
        Ok(read_lock.clone())
    }

    fn require_engine(&self) -> Result<LlamaRuntime, ggml::EngineError> {
        let read_lock: std::sync::RwLockReadGuard<'_, Option<LlamaRuntime>> =
            self.inference_engine.read().map_err(|_| GGMLLlamaEngineError::LockPoisoned {
                operation: "lock llama engine state",
            })?;
        let engine = read_lock.as_ref().ok_or(GGMLLlamaEngineError::ModelNotLoaded)?;
        Ok(engine.clone())
    }

    fn require_model(&self) -> Result<Arc<LlamaModel>, ggml::EngineError> {
        let read_lock = self.loaded_model.read().map_err(|_| {
            GGMLLlamaEngineError::LockPoisoned { operation: "read loaded llama model state" }
        })?;
        let model = read_lock.as_ref().ok_or(GGMLLlamaEngineError::ModelNotLoaded)?;
        Ok(Arc::clone(model))
    }

    /// Apply the current model chat template to structured chat messages.
    pub fn apply_chat_template(
        &self,
        messages: &[ChatMessage],
        add_assistant_prompt: bool,
    ) -> Result<String, ggml::EngineError> {
        let model = self.require_model()?;
        if let Some(template) = self.explicit_chat_template()? {
            return model
                .apply_chat_template_str(&template, messages, add_assistant_prompt)
                .map_err(|source| GGMLLlamaEngineError::ApplyChatTemplate { source }.into());
        }

        model
            .apply_chat_template(None, messages, add_assistant_prompt)
            .map_err(|source| GGMLLlamaEngineError::ApplyChatTemplate { source }.into())
    }

    fn render_prompt(&self, request: &LlamaDispatchRequest) -> String {
        if request.apply_chat_template && !request.chat_messages.is_empty() {
            let add_assistant = infer_add_assistant_prompt(&request.chat_messages);
            match self.apply_chat_template(&request.chat_messages, add_assistant) {
                Ok(applied) => return applied,
                Err(error) => {
                    warn!(
                        error = %error,
                        "failed to apply embedded chat template; falling back to raw prompt"
                    );
                }
            }
        }

        let Some(parsed) = parse_role_prefixed_chat_prompt(&request.prompt) else {
            return request.prompt.clone();
        };

        match self.apply_chat_template(&parsed.messages, parsed.add_assistant_prompt) {
            Ok(applied) => applied,
            Err(error) => {
                warn!(
                    error = %error,
                    "failed to apply llama chat template; falling back to raw prompt"
                );
                request.prompt.clone()
            }
        }
    }

    async fn prepare_managed_session(
        &self,
        session_key: Option<String>,
        full_prompt: String,
        grammar: Option<String>,
    ) -> Result<PreparedSession, ggml::EngineError> {
        let Some(key) = session_key else {
            return Ok(PreparedSession {
                key: None,
                sid: None,
                delta_prompt: full_prompt.clone(),
                full_prompt,
                cached_tokens: 0,
            });
        };

        let plan;

        {
            let mut bindings = self.session_bindings.lock().await;
            plan = plan_session_reuse(&key, bindings.get(&key), &full_prompt, grammar.as_deref())
                .map_err(ggml::EngineError::from)?;
            bindings.insert(key.clone(), SessionBinding::Busy);
        }

        let (sid, delta_prompt, cached_tokens) = match plan {
            SessionReusePlan::CreateFresh { delta_prompt, cached_tokens } => {
                match self.create_session_with_grammar(grammar.clone()).await {
                    Ok(sid) => (Some(sid), delta_prompt, cached_tokens),
                    Err(error) => {
                        self.session_bindings.lock().await.remove(&key);
                        return Err(error);
                    }
                }
            }
            SessionReusePlan::RestoreSnapshot { snapshot, delta_prompt, cached_tokens } => {
                match self.create_session_from_snapshot(snapshot, grammar.clone()).await {
                    Ok(sid) => (Some(sid), delta_prompt, cached_tokens),
                    Err(error) => {
                        self.session_bindings.lock().await.remove(&key);
                        return Err(error);
                    }
                }
            }
        };

        Ok(PreparedSession { key: Some(key), sid, delta_prompt, full_prompt, cached_tokens })
    }

    fn build_usage(
        &self,
        prompt: &str,
        generated: &str,
        cached_tokens: u32,
    ) -> Option<TextGenerationUsage> {
        let model = self.require_model().ok()?;
        let prompt_tokens = u32::try_from(model.tokenize(prompt, false, true).ok()?.len()).ok()?;
        let completion_tokens =
            u32::try_from(model.tokenize(generated, false, true).ok()?.len()).ok()?;
        let cached_tokens = cached_tokens.min(prompt_tokens);

        Some(TextGenerationUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens.saturating_add(completion_tokens),
            prompt_tokens_details: TextPromptTokensDetails { cached_tokens },
            estimated: false,
        })
    }

    async fn commit_managed_session(
        &self,
        key: Option<String>,
        sid: Option<SessionId>,
        full_prompt: &str,
        generated: &str,
        grammar: Option<String>,
    ) -> Result<(), ggml::EngineError> {
        let (Some(key), Some(sid)) = (key, sid) else {
            return Ok(());
        };

        let snapshot = match self.snapshot_session(sid).await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.drop_managed_session(Some(key), Some(sid)).await;
                return Err(error);
            }
        };

        if let Err(error) = self.end_session(sid).await {
            self.session_bindings.lock().await.remove(&key);
            return Err(error);
        }

        let mut cached_prompt = String::with_capacity(full_prompt.len() + generated.len());
        cached_prompt.push_str(full_prompt);
        cached_prompt.push_str(generated);
        self.session_bindings
            .lock()
            .await
            .insert(key, SessionBinding::Ready { snapshot, cached_prompt, grammar });
        Ok(())
    }

    async fn drop_managed_session(&self, key: Option<String>, sid: Option<SessionId>) {
        if let Some(key) = key {
            self.session_bindings.lock().await.remove(&key);
        }

        if let Some(sid) = sid {
            if let Err(error) = self.end_session(sid).await {
                warn!(session_id = sid, error = %error, "failed to end llama session during cleanup");
            }
        }
    }

    pub(crate) async fn dispatch_inference(
        &self,
        request: LlamaDispatchRequest,
    ) -> Result<LlamaDispatchOutput, ggml::EngineError> {
        let prompt = self.render_prompt(&request);
        let max_tokens = request.max_tokens;
        let grammar = request.grammar.clone();
        let session_key = request.session_key.clone();
        let commit_grammar = request.grammar.clone();
        let prepared = self.prepare_managed_session(session_key, prompt, grammar.clone()).await?;

        match self.inference(&prepared.delta_prompt, max_tokens, prepared.sid, grammar).await {
            Ok(text) => {
                let usage = self.build_usage(&prepared.full_prompt, &text, prepared.cached_tokens);
                if let Err(error) = self
                    .commit_managed_session(
                        prepared.key,
                        prepared.sid,
                        &prepared.full_prompt,
                        &text,
                        commit_grammar,
                    )
                    .await
                {
                    warn!(error = %error, "failed to persist llama session snapshot after inference");
                }
                Ok(LlamaDispatchOutput { text, usage })
            }
            Err(error) => {
                self.drop_managed_session(prepared.key, prepared.sid).await;
                Err(error)
            }
        }
    }

    pub(crate) async fn dispatch_inference_stream(
        self: &Arc<Self>,
        request: LlamaDispatchRequest,
        cancel_rx: watch::Receiver<bool>,
    ) -> Result<BaseStreamHandle, ggml::EngineError> {
        let prompt = self.render_prompt(&request);
        let max_tokens = request.max_tokens;
        let grammar = request.grammar.clone();
        let session_key = request.session_key.clone();
        let commit_grammar = request.grammar.clone();
        let prepared = self.prepare_managed_session(session_key, prompt, grammar.clone()).await?;

        let (mut llama_rx, sid) = match self
            .inference_stream(&prepared.delta_prompt, max_tokens, prepared.sid, grammar)
            .await
        {
            Ok(stream) => stream,
            Err(error) => {
                self.drop_managed_session(prepared.key, prepared.sid).await;
                return Err(error);
            }
        };

        let (stream_tx, stream_rx) = mpsc::channel::<BaseStreamChunk>(64);
        let engine = Arc::clone(self);
        tokio::spawn(async move {
            let PreparedSession { key, full_prompt, cached_tokens, .. } = prepared;
            let grammar = commit_grammar;
            let mut generated = String::new();
            let mut completed = false;
            let mut forward_failed = false;
            let mut stream_error = false;
            let mut cancelled = false;
            let mut cancel_rx = cancel_rx;

            loop {
                tokio::select! {
                    cancel_changed = cancel_rx.changed(), if !completed && !stream_error && !forward_failed => {
                        let cancel_requested = if cancel_changed.is_ok() {
                            *cancel_rx.borrow()
                        } else {
                            false
                        };
                        if cancel_requested {
                            cancelled = true;
                            if let Err(error) = engine.cancel_generate(sid).await {
                                warn!(session_id = sid, error = %error, "failed to cancel llama generation");
                            }
                        } else if cancel_changed.is_ok() {
                            continue;
                        }
                        break;
                    }
                    chunk = llama_rx.recv() => {
                        let Some(chunk) = chunk else {
                            break;
                        };

                        match chunk {
                            StreamChunk::Token(text) => {
                                generated.push_str(&text);
                                if stream_tx.send(BaseStreamChunk::Token(text)).await.is_err() {
                                    forward_failed = true;
                                    if !completed && !stream_error {
                                        if let Err(error) = engine.cancel_generate(sid).await {
                                            warn!(
                                                session_id = sid,
                                                error = %error,
                                                "failed to cancel llama generation after downstream disconnect"
                                            );
                                        }
                                    }
                                    break;
                                }
                            }
                            StreamChunk::Done => {
                                completed = true;
                                break;
                            }
                            StreamChunk::Error(error) => {
                                stream_error = true;
                                if stream_tx.send(BaseStreamChunk::Error(error)).await.is_err() {
                                    forward_failed = true;
                                }
                                break;
                            }
                        }
                    }
                }
            }

            if completed && !forward_failed && !stream_error && !cancelled {
                if let Some(usage) = engine.build_usage(&full_prompt, &generated, cached_tokens)
                    && stream_tx
                        .send(BaseStreamChunk::Json(serde_json::json!({ "usage": usage })))
                        .await
                        .is_err()
                {
                    forward_failed = true;
                }
            }

            if completed
                && !forward_failed
                && !stream_error
                && stream_tx.send(BaseStreamChunk::Done).await.is_err()
            {
                forward_failed = true;
            }

            if key.is_some() && completed && !forward_failed && !stream_error && !cancelled {
                if let Err(error) = engine
                    .commit_managed_session(key, Some(sid), &full_prompt, &generated, grammar)
                    .await
                {
                    warn!(error = %error, "failed to persist llama session snapshot after stream");
                }
            } else {
                engine.drop_managed_session(key, Some(sid)).await;
            }
        });

        Ok(stream_rx)
    }

    /// Create a new session on the underlying llama runtime.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn create_session(&self) -> Result<SessionId, ggml::EngineError> {
        let engine = self.require_engine()?;
        engine.create_session().await.map_err(GGMLLlamaEngineError::from).map_err(Into::into)
    }

    /// Create a new session with an optional GBNF grammar constraint.
    pub async fn create_session_with_grammar(
        &self,
        grammar: Option<String>,
    ) -> Result<SessionId, ggml::EngineError> {
        let engine = self.require_engine()?;
        engine
            .create_session_with_grammar(grammar)
            .await
            .map_err(GGMLLlamaEngineError::from)
            .map_err(Into::into)
    }

    async fn create_session_from_snapshot(
        &self,
        snapshot: LlamaSessionSnapshot,
        grammar: Option<String>,
    ) -> Result<SessionId, ggml::EngineError> {
        let engine = self.require_engine()?;
        engine
            .create_session_from_snapshot(snapshot, grammar)
            .await
            .map_err(GGMLLlamaEngineError::from)
            .map_err(Into::into)
    }

    async fn snapshot_session(
        &self,
        session_id: SessionId,
    ) -> Result<LlamaSessionSnapshot, ggml::EngineError> {
        let engine = self.require_engine()?;
        engine
            .snapshot_session(session_id)
            .await
            .map_err(GGMLLlamaEngineError::from)
            .map_err(Into::into)
    }

    /// Append text delta to an existing session.
    pub async fn append_input(
        &self,
        session_id: SessionId,
        text_delta: String,
    ) -> Result<(), ggml::EngineError> {
        let engine = self.require_engine()?;
        engine
            .append_input(session_id, text_delta)
            .await
            .map_err(GGMLLlamaEngineError::from)
            .map_err(Into::into)
    }

    /// Start streaming generation for a session.
    pub async fn generate_stream(
        &self,
        session_id: SessionId,
        max_new_tokens: usize,
    ) -> Result<StreamHandle, ggml::EngineError> {
        let engine = self.require_engine()?;
        engine
            .generate_stream(session_id, max_new_tokens)
            .await
            .map_err(GGMLLlamaEngineError::from)
            .map_err(Into::into)
    }

    /// End a session and release its KV entries.
    pub async fn end_session(&self, session_id: SessionId) -> Result<(), ggml::EngineError> {
        let engine = self.require_engine()?;
        engine.end_session(session_id).await.map_err(GGMLLlamaEngineError::from).map_err(Into::into)
    }

    /// Cancel active generation while keeping session KV state.
    ///
    /// Called from tests and available for future API callers via the backend dispatch path.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) async fn cancel_generate(
        &self,
        session_id: SessionId,
    ) -> Result<(), ggml::EngineError> {
        let engine = self.require_engine()?;
        engine
            .cancel_generate(session_id)
            .await
            .map_err(GGMLLlamaEngineError::from)
            .map_err(Into::into)
    }

    /// Generate text from a prompt by delegating to the shared llama runtime.
    ///
    /// If `session_id` is `None`, creates a temporary session (with the
    /// optional grammar constraint applied to its sampler chain), appends the
    /// full prompt, consumes stream chunks until `Done`, and then ends the
    /// session.
    ///
    /// If `session_id` is `Some(sid)`, appends to the existing session and
    /// returns the output without ending the session (caller is responsible
    /// for cleanup).  `grammar` is ignored when `session_id` is `Some` because
    /// the session's sampler was already built at creation time.
    pub async fn inference(
        &self,
        prompt: &str,
        max_tokens: usize,
        session_id: Option<SessionId>,
        grammar: Option<String>,
    ) -> Result<String, ggml::EngineError> {
        let sid = match session_id {
            Some(sid) => sid,
            None => self.create_session_with_grammar(grammar).await?,
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
    /// If `session_id` is `None`, creates a new temporary session (with the
    /// optional grammar constraint applied to its sampler chain) and returns
    /// both the stream handle and the session ID (caller must end the session
    /// when done).
    ///
    /// If `session_id` is `Some(sid)`, appends to the existing session and
    /// returns the stream handle (caller is responsible for session
    /// management).  `grammar` is ignored when `session_id` is `Some` because
    /// the session's sampler was already built at creation time.
    pub async fn inference_stream(
        &self,
        prompt: &str,
        max_tokens: usize,
        session_id: Option<SessionId>,
        grammar: Option<String>,
    ) -> Result<(StreamHandle, SessionId), ggml::EngineError> {
        let sid = match session_id {
            Some(sid) => sid,
            None => self.create_session_with_grammar(grammar).await?,
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
        let mut write_lock = self.inference_engine.write().map_err(|_| {
            GGMLLlamaEngineError::LockPoisoned { operation: "lock llama engine state" }
        })?;
        *write_lock = None;
        let mut model_write_lock = self.loaded_model.write().map_err(|_| {
            GGMLLlamaEngineError::LockPoisoned { operation: "lock loaded llama model state" }
        })?;
        *model_write_lock = None;
        self.session_bindings.blocking_lock().clear();
        Ok(())
    }

    /// Unload the current model and stop all inference workers.
    pub fn unload(&self) -> Result<(), ggml::EngineError> {
        Ok(self.do_unload()?)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SessionBinding, SessionReusePlan, infer_add_assistant_prompt,
        parse_role_prefixed_chat_prompt, plan_session_reuse,
    };
    use slab_llama::{ChatMessage, LlamaSessionSnapshot};
    use std::sync::Arc;

    fn snapshot() -> LlamaSessionSnapshot {
        LlamaSessionSnapshot { worker_id: 1, n_past: 12, state: Arc::from([1_u8, 2, 3, 4]) }
    }

    #[test]
    fn plan_session_reuse_creates_fresh_when_no_binding_exists() {
        let plan = plan_session_reuse("chat-1", None, "hello", None).expect("plan should succeed");
        match plan {
            SessionReusePlan::CreateFresh { delta_prompt, cached_tokens } => {
                assert_eq!(delta_prompt, "hello");
                assert_eq!(cached_tokens, 0);
            }
            SessionReusePlan::RestoreSnapshot { .. } => panic!("expected fresh session plan"),
        }
    }

    #[test]
    fn plan_session_reuse_rejects_busy_binding() {
        let error = plan_session_reuse("chat-1", Some(&SessionBinding::Busy), "hello", None)
            .expect_err("busy binding should reject concurrent reuse");
        assert!(error.to_string().contains("already active"));
    }

    #[test]
    fn plan_session_reuse_restores_snapshot_for_prompt_suffix() {
        let binding = SessionBinding::Ready {
            snapshot: snapshot(),
            cached_prompt: "hello world".to_owned(),
            grammar: Some("grammar".to_owned()),
        };

        let plan = plan_session_reuse("chat-1", Some(&binding), "hello world!!!", Some("grammar"))
            .expect("plan should succeed");

        match plan {
            SessionReusePlan::RestoreSnapshot { snapshot, delta_prompt, cached_tokens } => {
                assert_eq!(snapshot.worker_id, 1);
                assert_eq!(snapshot.n_past, 12);
                assert_eq!(snapshot.state.as_ref(), &[1, 2, 3, 4]);
                assert_eq!(delta_prompt, "!!!");
                assert_eq!(cached_tokens, 12);
            }
            SessionReusePlan::CreateFresh { .. } => panic!("expected snapshot restore plan"),
        }
    }

    #[test]
    fn plan_session_reuse_invalidates_snapshot_on_grammar_change() {
        let binding = SessionBinding::Ready {
            snapshot: snapshot(),
            cached_prompt: "hello".to_owned(),
            grammar: Some("json".to_owned()),
        };

        let plan = plan_session_reuse("chat-1", Some(&binding), "hello world", Some("tool"))
            .expect("plan should succeed");
        match plan {
            SessionReusePlan::CreateFresh { delta_prompt, cached_tokens } => {
                assert_eq!(delta_prompt, "hello world");
                assert_eq!(cached_tokens, 0);
            }
            SessionReusePlan::RestoreSnapshot { .. } => {
                panic!("expected fresh session when grammar changes")
            }
        }
    }

    #[test]
    fn parse_role_prefixed_chat_prompt_detects_assistant_prefill() {
        let parsed = parse_role_prefixed_chat_prompt("User: hi\nAssistant:")
            .expect("role-prefixed prompt should parse");
        assert_eq!(
            parsed.messages,
            vec![ChatMessage { role: "user".to_owned(), content: "hi".to_owned() }]
        );
        assert!(parsed.add_assistant_prompt);
    }

    #[test]
    fn infer_add_assistant_prompt_respects_last_assistant_message() {
        let user_last = vec![ChatMessage { role: "user".to_owned(), content: "hi".to_owned() }];
        let assistant_last = vec![
            ChatMessage { role: "user".to_owned(), content: "hi".to_owned() },
            ChatMessage { role: "assistant".to_owned(), content: "hello".to_owned() },
        ];
        assert!(infer_add_assistant_prompt(&user_last));
        assert!(!infer_add_assistant_prompt(&assistant_last));
    }
}
