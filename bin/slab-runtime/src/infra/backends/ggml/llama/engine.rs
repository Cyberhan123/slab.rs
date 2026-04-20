use crate::infra::backends::ggml;
use slab_llama::{
    Llama, LlamaContextParams, LlamaInferenceOutput, LlamaLogitBias, LlamaModel, LlamaModelParams,
    LlamaRuntime, LlamaSessionSnapshot, LlamaStopInfo,
};
use slab_runtime_core::backend::{
    StreamChunk as BaseStreamChunk, StreamHandle as BaseStreamHandle,
};
use slab_utils::loader::load_library_from_dir;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use tokio::sync::{Mutex, mpsc, watch};
use tracing::{info, warn};

use super::contract::{
    GgmlLlamaLoadConfig, TextGenerationMetadata, TextGenerationStreamEvent, TextGenerationUsage,
    TextPromptTokensDetails, TextStopMetadata,
};

use super::{GGMLLlamaEngineError, SessionId, StreamChunk, StreamHandle};

#[derive(Debug, Clone)]
pub(crate) struct LlamaDispatchRequest {
    pub prompt: String,
    pub max_tokens: usize,
    pub session_key: Option<String>,
    pub gbnf: Option<String>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub min_p: Option<f32>,
    pub repetition_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub ignore_eos: bool,
    pub logit_bias: Option<serde_json::Value>,
    pub stop_sequences: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct LlamaDispatchOutput {
    pub text: String,
    pub usage: Option<TextGenerationUsage>,
    pub finish_reason: Option<String>,
    pub metadata: TextGenerationMetadata,
}

fn stop_info_to_metadata(stop: &LlamaStopInfo) -> TextGenerationMetadata {
    TextGenerationMetadata {
        stop: Some(TextStopMetadata {
            token_id: stop.stop_token_id,
            token_text: stop.stop_token_text.clone(),
            token_kind: stop.stop_token_kind.clone(),
        }),
        ..Default::default()
    }
}

fn resolve_logit_bias_value(value: &serde_json::Value) -> Option<f32> {
    if let Some(bias) = value.as_f64() {
        Some(bias as f32)
    } else if matches!(value, serde_json::Value::Bool(false)) {
        Some(f32::NEG_INFINITY)
    } else {
        None
    }
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

fn plan_session_reuse(
    key: &str,
    existing: Option<&SessionBinding>,
    full_prompt: &str,
    gbnf: Option<&str>,
) -> Result<SessionReusePlan, GGMLLlamaEngineError> {
    match existing {
        None => Ok(SessionReusePlan::CreateFresh {
            delta_prompt: full_prompt.to_owned(),
            cached_tokens: 0,
        }),
        Some(SessionBinding::Busy) => {
            // A previous request may have left this key in Busy state due to a
            // crash, cancelled task, or unclean shutdown. Instead of permanently
            // blocking all future requests on this conversation, we recover by
            // discarding the stale binding and creating a fresh session.  The
            // only cost is losing the KV cache for one turn.
            warn!(
                session_key = key,
                "session binding is stuck in Busy state; recovering by creating a fresh session"
            );
            Ok(SessionReusePlan::CreateFresh {
                delta_prompt: full_prompt.to_owned(),
                cached_tokens: 0,
            })
        }
        Some(SessionBinding::Ready { snapshot, cached_prompt, grammar: cached_grammar }) => {
            if cached_grammar.as_deref() != gbnf {
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
    session_bindings: Mutex<HashMap<String, SessionBinding>>,
}

// # Safety
//
// `GGMLLlamaEngine` is `Send` and `Sync` because all mutable state is guarded by
// interior mutability primitives that provide thread-safe access:
//
// 1. **`instance: Arc<Llama>`** - The underlying `Llama` wraps a dlopen2-generated
//    handle that holds a read-only table of function pointers loaded once at startup.
//    The function pointer table is never mutated after creation, making concurrent
//    reads from multiple threads safe.
//
// 2. **`inference_engine: RwLock<Option<LlamaRuntime>>`** - The runtime engine
//    handle is protected by a `RwLock`, allowing multiple concurrent readers or
//    exclusive writer access. The `LlamaRuntime` type itself is not `Send + Sync`,
//    but the `RwLock` ensures that only one thread can access it mutably at a time.
//
// 3. **`loaded_model: RwLock<Option<Arc<LlamaModel>>>`** - Similar to the inference
//    engine, the loaded model handle is protected by a `RwLock`.
//
// 4. **`session_bindings: Mutex<HashMap<...>>`** - Session bindings are protected
//    by a `Mutex`, providing exclusive access during mutations.
//
// The combination of these interior mutability primitives ensures that all accesses
// to the mutable state are properly synchronized, allowing `GGMLLlamaEngine` to be
// safely shared across threads.
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
        Ok(())
    }

    pub(crate) fn load_model_from_config(
        &self,
        config: &GgmlLlamaLoadConfig,
    ) -> Result<(), ggml::EngineError> {
        let mut ctx_params = LlamaContextParams {
            kv_unified: true,
            flash_attn: config.flash_attn,
            ..Default::default()
        };
        if let Some(context_length) = config.context_length {
            ctx_params.n_ctx = context_length;
            if ctx_params.n_batch > context_length {
                ctx_params.n_batch = context_length;
            }
            if ctx_params.n_ubatch > context_length {
                ctx_params.n_ubatch = context_length;
            }
        }

        self.load_model_with_workers(
            &config.model_path,
            LlamaModelParams::default(),
            ctx_params,
            config.engine_workers,
        )
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

    fn append_string_logit_bias(
        model: &LlamaModel,
        text: &str,
        bias: f32,
        logit_bias: &mut Vec<LlamaLogitBias>,
    ) {
        match model.tokenize(text, false, true) {
            Ok(tokens) => {
                logit_bias.extend(tokens.into_iter().map(|token| LlamaLogitBias { token, bias }));
            }
            Err(error) => {
                warn!(text, %error, "failed to tokenize logit_bias string; ignoring entry");
            }
        }
    }

    fn resolve_logit_bias(
        &self,
        raw_logit_bias: Option<&serde_json::Value>,
    ) -> Result<Vec<LlamaLogitBias>, ggml::EngineError> {
        let Some(raw_logit_bias) = raw_logit_bias else {
            return Ok(Vec::new());
        };

        let model = self.require_model()?;
        let n_vocab = model.n_vocab();
        let mut logit_bias = Vec::new();

        match raw_logit_bias {
            serde_json::Value::Array(entries) => {
                for entry in entries {
                    let serde_json::Value::Array(pair) = entry else {
                        continue;
                    };
                    if pair.len() != 2 {
                        continue;
                    }

                    let Some(bias) = resolve_logit_bias_value(&pair[1]) else {
                        continue;
                    };

                    if let Some(token) =
                        pair[0].as_i64().and_then(|token| i32::try_from(token).ok())
                    {
                        if token >= 0 && token < n_vocab {
                            logit_bias.push(LlamaLogitBias { token, bias });
                        }
                    } else if let Some(text) = pair[0].as_str() {
                        Self::append_string_logit_bias(&model, text, bias, &mut logit_bias);
                    }
                }
            }
            serde_json::Value::Object(entries) => {
                for (key, value) in entries {
                    let Some(bias) = resolve_logit_bias_value(value) else {
                        continue;
                    };

                    if let Ok(token) = key.parse::<i32>() {
                        if token >= 0 && token < n_vocab {
                            logit_bias.push(LlamaLogitBias { token, bias });
                        }
                    } else {
                        Self::append_string_logit_bias(&model, key, bias, &mut logit_bias);
                    }
                }
            }
            _ => {
                warn!("unsupported logit_bias JSON shape; expected array or object");
            }
        }

        Ok(logit_bias)
    }

    async fn prepare_managed_session(
        &self,
        session_key: Option<String>,
        full_prompt: String,
        gbnf: Option<String>,
        temperature: Option<f32>,
        top_p: Option<f32>,
        top_k: Option<i32>,
        min_p: Option<f32>,
        repetition_penalty: Option<f32>,
        presence_penalty: Option<f32>,
        ignore_eos: bool,
        logit_bias: &[LlamaLogitBias],
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
            plan = plan_session_reuse(&key, bindings.get(&key), &full_prompt, gbnf.as_deref())
                .map_err(ggml::EngineError::from)?;
            bindings.insert(key.clone(), SessionBinding::Busy);
        }

        let (sid, delta_prompt, cached_tokens) = match plan {
            SessionReusePlan::CreateFresh { delta_prompt, cached_tokens } => {
                match self
                    .create_session_with_options(
                        gbnf.clone(),
                        temperature,
                        top_p,
                        top_k,
                        min_p,
                        repetition_penalty,
                        presence_penalty,
                        ignore_eos,
                        logit_bias.to_vec(),
                    )
                    .await
                {
                    Ok(sid) => (Some(sid), delta_prompt, cached_tokens),
                    Err(error) => {
                        self.session_bindings.lock().await.remove(&key);
                        return Err(error);
                    }
                }
            }
            SessionReusePlan::RestoreSnapshot { snapshot, delta_prompt, cached_tokens } => {
                match self
                    .create_session_from_snapshot(
                        snapshot,
                        gbnf.clone(),
                        temperature,
                        top_p,
                        top_k,
                        min_p,
                        repetition_penalty,
                        presence_penalty,
                        ignore_eos,
                        logit_bias.to_vec(),
                    )
                    .await
                {
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
        gbnf: Option<String>,
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
            .insert(key, SessionBinding::Ready { snapshot, cached_prompt, grammar: gbnf });
        Ok(())
    }

    async fn drop_managed_session(&self, key: Option<String>, sid: Option<SessionId>) {
        if let Some(key) = key {
            self.session_bindings.lock().await.remove(&key);
        }

        if let Some(sid) = sid
            && let Err(error) = self.end_session(sid).await
        {
            warn!(session_id = sid, error = %error, "failed to end llama session during cleanup");
        }
    }

    pub(crate) async fn dispatch_inference(
        &self,
        request: LlamaDispatchRequest,
    ) -> Result<LlamaDispatchOutput, ggml::EngineError> {
        let prompt = request.prompt.clone();
        let max_tokens = request.max_tokens;
        let gbnf = request.gbnf.clone();
        let session_key = request.session_key.clone();
        let commit_gbnf = request.gbnf.clone();
        let stop_sequences = request.stop_sequences.clone();
        let logit_bias = self.resolve_logit_bias(request.logit_bias.as_ref())?;
        let prepared = self
            .prepare_managed_session(
                session_key,
                prompt,
                gbnf.clone(),
                request.temperature,
                request.top_p,
                request.top_k,
                request.min_p,
                request.repetition_penalty,
                request.presence_penalty,
                request.ignore_eos,
                &logit_bias,
            )
            .await?;

        match self
            .inference(
                &prepared.delta_prompt,
                max_tokens,
                prepared.sid,
                gbnf,
                request.ignore_eos,
                &logit_bias,
            )
            .await
        {
            Ok(output) => {
                // Apply stop sequence trimming to the generated text before committing.
                let (trimmed_text, stop_matched) =
                    apply_stop_sequences(&output.text, &stop_sequences);
                let usage =
                    self.build_usage(&prepared.full_prompt, &trimmed_text, prepared.cached_tokens);
                let finish_reason = if stop_matched {
                    Some("stop".to_owned())
                } else {
                    output.stop.as_ref().map(|stop| stop.finish_reason.clone())
                };
                let metadata = output.stop.as_ref().map(stop_info_to_metadata).unwrap_or_default();
                if let Err(error) = self
                    .commit_managed_session(
                        prepared.key,
                        prepared.sid,
                        &prepared.full_prompt,
                        &trimmed_text,
                        commit_gbnf,
                    )
                    .await
                {
                    warn!(error = %error, "failed to persist llama session snapshot after inference");
                }
                Ok(LlamaDispatchOutput { text: trimmed_text, usage, finish_reason, metadata })
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
        let prompt = request.prompt.clone();
        let max_tokens = request.max_tokens;
        let gbnf = request.gbnf.clone();
        let session_key = request.session_key.clone();
        let commit_gbnf = request.gbnf.clone();
        let stop_sequences = request.stop_sequences.clone();
        let logit_bias = self.resolve_logit_bias(request.logit_bias.as_ref())?;
        let prepared = self
            .prepare_managed_session(
                session_key,
                prompt,
                gbnf.clone(),
                request.temperature,
                request.top_p,
                request.top_k,
                request.min_p,
                request.repetition_penalty,
                request.presence_penalty,
                request.ignore_eos,
                &logit_bias,
            )
            .await?;

        let (mut llama_rx, sid) = match self
            .inference_stream(
                &prepared.delta_prompt,
                max_tokens,
                prepared.sid,
                gbnf,
                request.ignore_eos,
                &logit_bias,
            )
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
            let gbnf = commit_gbnf;
            let mut generated = String::new();
            let mut completed = false;
            let mut forward_failed = false;
            let mut stream_error = false;
            let mut cancelled = false;
            let mut stop_matched = false;
            let mut terminal_finish_reason: Option<String> = None;
            let mut terminal_metadata = TextGenerationMetadata::default();
            // Tracks how many bytes of `generated` have been forwarded downstream.
            // When a stop sequence is partially accumulated we hold back the
            // uncertain tail so that we never forward text that may need to be
            // trimmed later.
            let mut forwarded_len: usize = 0;
            let mut cancel_rx = cancel_rx;

            loop {
                tokio::select! {
                    cancel_changed = cancel_rx.changed(), if !completed && !stream_error && !forward_failed && !stop_matched => {
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

                                // Check for stop sequences in the accumulated output.
                                if !stop_sequences.is_empty() {
                                    if let Some((stop_index, _)) = stop_sequences
                                        .iter()
                                        .filter(|s| !s.is_empty())
                                        .filter_map(|s| generated.find(s.as_str()).map(|i| (i, s)))
                                        .min_by_key(|(i, _)| *i)
                                    {
                                        // Found a stop sequence — forward text up to it, then cancel.
                                        stop_matched = true;
                                        let safe_end = stop_index;
                                        if safe_end > forwarded_len {
                                            let forward_text = generated[forwarded_len..safe_end].to_owned();
                                            forwarded_len = safe_end;
                                            if stream_tx.send(BaseStreamChunk::Token(forward_text)).await.is_err() {
                                                forward_failed = true;
                                            }
                                        }
                                        // Truncate generated to the stop boundary for session commit.
                                        generated.truncate(safe_end);
                                        // Cancel the backend generation.
                                        if let Err(error) = engine.cancel_generate(sid).await {
                                            warn!(
                                                session_id = sid,
                                                error = %error,
                                                "failed to cancel llama generation after stop sequence match"
                                            );
                                        }
                                        break;
                                    }

                                    // Hold back a trailing partial match to avoid forwarding
                                    // text that might be the start of a stop sequence.
                                    let hold_back = trailing_partial_stop_len(&generated, &stop_sequences);
                                    let safe_end = generated.len().saturating_sub(hold_back);
                                    if safe_end > forwarded_len {
                                        let forward_text = generated[forwarded_len..safe_end].to_owned();
                                        forwarded_len = safe_end;
                                        if stream_tx.send(BaseStreamChunk::Token(forward_text)).await.is_err() {
                                            forward_failed = true;
                                            if !completed
                                                && !stream_error
                                                && let Err(error) = engine.cancel_generate(sid).await
                                            {
                                                warn!(
                                                    session_id = sid,
                                                    error = %error,
                                                    "failed to cancel llama generation after downstream disconnect"
                                                );
                                            }
                                            break;
                                        }
                                    }
                                } else {
                                    // No stop sequences — forward directly.
                                    if stream_tx.send(BaseStreamChunk::Token(text)).await.is_err() {
                                        forward_failed = true;
                                        if !completed
                                            && !stream_error
                                            && let Err(error) = engine.cancel_generate(sid).await
                                        {
                                            warn!(
                                                session_id = sid,
                                                error = %error,
                                                "failed to cancel llama generation after downstream disconnect"
                                            );
                                        }
                                        break;
                                    }
                                    forwarded_len = generated.len();
                                }
                            }
                            StreamChunk::Done => {
                                completed = true;
                                break;
                            }
                            StreamChunk::Stop(stop) => {
                                terminal_finish_reason = Some(stop.finish_reason.clone());
                                terminal_metadata = stop_info_to_metadata(&stop);
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

            // Flush any remaining held-back text after generation completes (no stop matched).
            if !stop_matched && !forward_failed && !stream_error && forwarded_len < generated.len()
            {
                let tail = generated[forwarded_len..].to_owned();
                if stream_tx.send(BaseStreamChunk::Token(tail)).await.is_err() {
                    forward_failed = true;
                }
            }

            let effectively_completed = completed || stop_matched;

            if effectively_completed && !forward_failed && !stream_error && !cancelled {
                let finish_reason = terminal_finish_reason
                    .clone()
                    .or_else(|| stop_matched.then(|| "stop".to_owned()));
                if let Some(finish_reason) = finish_reason {
                    let event = TextGenerationStreamEvent {
                        delta: Some(String::new()),
                        done: Some(true),
                        finish_reason: Some(finish_reason),
                        usage: None,
                        metadata: (!terminal_metadata.is_empty())
                            .then_some(terminal_metadata.clone()),
                    };
                    let payload = serde_json::to_value(event)
                        .expect("llama stream terminal event should serialize");
                    if stream_tx.send(BaseStreamChunk::Json(payload)).await.is_err() {
                        forward_failed = true;
                    }
                }
            }

            if effectively_completed
                && !forward_failed
                && !stream_error
                && !cancelled
                && let Some(usage) = engine.build_usage(&full_prompt, &generated, cached_tokens)
                && stream_tx
                    .send(BaseStreamChunk::Json(
                        serde_json::to_value(TextGenerationStreamEvent {
                            usage: Some(usage),
                            ..Default::default()
                        })
                        .expect("llama stream usage event should serialize"),
                    ))
                    .await
                    .is_err()
            {
                forward_failed = true;
            }

            if effectively_completed
                && !forward_failed
                && !stream_error
                && stream_tx.send(BaseStreamChunk::Done).await.is_err()
            {
                forward_failed = true;
            }

            if key.is_some()
                && effectively_completed
                && !forward_failed
                && !stream_error
                && !cancelled
            {
                if let Err(error) = engine
                    .commit_managed_session(key, Some(sid), &full_prompt, &generated, gbnf)
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

    /// Create a new session with optional GBNF and sampling overrides.
    pub async fn create_session_with_options(
        &self,
        gbnf: Option<String>,
        temperature: Option<f32>,
        top_p: Option<f32>,
        top_k: Option<i32>,
        min_p: Option<f32>,
        repetition_penalty: Option<f32>,
        presence_penalty: Option<f32>,
        ignore_eos: bool,
        logit_bias: Vec<LlamaLogitBias>,
    ) -> Result<SessionId, ggml::EngineError> {
        let engine = self.require_engine()?;
        engine
            .create_session_with_options(
                gbnf,
                temperature,
                top_p,
                top_k,
                min_p,
                repetition_penalty,
                presence_penalty,
                ignore_eos,
                logit_bias,
            )
            .await
            .map_err(GGMLLlamaEngineError::from)
            .map_err(Into::into)
    }

    async fn create_session_from_snapshot(
        &self,
        snapshot: LlamaSessionSnapshot,
        gbnf: Option<String>,
        temperature: Option<f32>,
        top_p: Option<f32>,
        top_k: Option<i32>,
        min_p: Option<f32>,
        repetition_penalty: Option<f32>,
        presence_penalty: Option<f32>,
        ignore_eos: bool,
        logit_bias: Vec<LlamaLogitBias>,
    ) -> Result<SessionId, ggml::EngineError> {
        let engine = self.require_engine()?;
        engine
            .create_session_from_snapshot(
                snapshot,
                gbnf,
                temperature,
                top_p,
                top_k,
                min_p,
                repetition_penalty,
                presence_penalty,
                ignore_eos,
                logit_bias,
            )
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
    /// optional GBNF constraint applied to its sampler chain), appends the
    /// full prompt, consumes stream chunks until `Done`, and then ends the
    /// session.
    ///
    /// If `session_id` is `Some(sid)`, appends to the existing session and
    /// returns the output without ending the session (caller is responsible
    /// for cleanup).  `gbnf`, `ignore_eos`, and `logit_bias` are ignored when
    /// `session_id` is `Some` because the session's sampler was already built
    /// at creation time.
    pub async fn inference(
        &self,
        prompt: &str,
        max_tokens: usize,
        session_id: Option<SessionId>,
        gbnf: Option<String>,
        ignore_eos: bool,
        logit_bias: &[LlamaLogitBias],
    ) -> Result<LlamaInferenceOutput, ggml::EngineError> {
        let sid = match session_id {
            Some(sid) => sid,
            None => {
                self.create_session_with_options(
                    gbnf,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    ignore_eos,
                    logit_bias.to_vec(),
                )
                .await?
            }
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
        let mut terminal_stop: Option<LlamaStopInfo> = None;
        let mut stream_error: Option<GGMLLlamaEngineError> = None;

        while let Some(chunk) = stream.recv().await {
            match chunk {
                StreamChunk::Token(piece) => output.push_str(&piece),
                StreamChunk::Stop(stop) => {
                    terminal_stop = Some(stop);
                }
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

        Ok(LlamaInferenceOutput { text: output, stop: terminal_stop })
    }

    /// Generate text from a prompt as an async stream.
    ///
    /// If `session_id` is `None`, creates a new temporary session (with the
    /// optional GBNF constraint applied to its sampler chain) and returns
    /// both the stream handle and the session ID (caller must end the session
    /// when done).
    ///
    /// If `session_id` is `Some(sid)`, appends to the existing session and
    /// returns the stream handle (caller is responsible for session
    /// management).  `gbnf`, `ignore_eos`, and `logit_bias` are ignored when
    /// `session_id` is `Some` because the session's sampler was already built
    /// at creation time.
    pub async fn inference_stream(
        &self,
        prompt: &str,
        max_tokens: usize,
        session_id: Option<SessionId>,
        gbnf: Option<String>,
        ignore_eos: bool,
        logit_bias: &[LlamaLogitBias],
    ) -> Result<(StreamHandle, SessionId), ggml::EngineError> {
        let sid = match session_id {
            Some(sid) => sid,
            None => {
                self.create_session_with_options(
                    gbnf,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    ignore_eos,
                    logit_bias.to_vec(),
                )
                .await?
            }
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
    use super::{SessionBinding, SessionReusePlan, plan_session_reuse};
    use slab_llama::LlamaSessionSnapshot;
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
    fn plan_session_reuse_recovers_from_busy_binding() {
        let plan = plan_session_reuse("chat-1", Some(&SessionBinding::Busy), "hello", None)
            .expect("busy binding should recover with a fresh session");
        match plan {
            SessionReusePlan::CreateFresh { delta_prompt, cached_tokens } => {
                assert_eq!(delta_prompt, "hello");
                assert_eq!(cached_tokens, 0);
            }
            SessionReusePlan::RestoreSnapshot { .. } => {
                panic!("expected fresh session when recovering from busy binding")
            }
        }
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
}

// ---------------------------------------------------------------------------
// Stop-sequence helpers
// ---------------------------------------------------------------------------

/// Trim `text` at the earliest occurrence of any stop sequence.
/// Returns the trimmed text and whether a stop was matched.
fn apply_stop_sequences(text: &str, stop_sequences: &[String]) -> (String, bool) {
    if stop_sequences.is_empty() {
        return (text.to_owned(), false);
    }
    if let Some((idx, _)) = stop_sequences
        .iter()
        .filter(|s| !s.is_empty())
        .filter_map(|s| text.find(s.as_str()).map(|i| (i, s)))
        .min_by_key(|(i, _)| *i)
    {
        (text[..idx].to_owned(), true)
    } else {
        (text.to_owned(), false)
    }
}

/// Return the length of the longest suffix of `generated` that is a *proper
/// prefix* of any stop sequence. This is how much text we must hold back
/// during streaming to avoid forwarding a partial stop match.
fn trailing_partial_stop_len(generated: &str, stop_sequences: &[String]) -> usize {
    let mut max_hold = 0usize;
    for stop in stop_sequences.iter().filter(|s| !s.is_empty()) {
        // Only inspect suffixes that start on UTF-8 char boundaries so the
        // caller can safely slice `generated[..generated.len() - hold_back]`.
        for start in generated.char_indices().map(|(idx, _)| idx) {
            let tail = &generated[start..];
            if tail.len() < stop.len() && stop.starts_with(tail) {
                max_hold = max_hold.max(tail.len());
            }
        }
    }
    max_hold
}

#[cfg(test)]
mod stop_sequence_tests {
    use super::{apply_stop_sequences, trailing_partial_stop_len};

    #[test]
    fn apply_stop_sequences_trims_at_earliest_match() {
        let stop_sequences = vec!["</think>".to_owned(), "###".to_owned()];
        let (trimmed, stop_matched) =
            apply_stop_sequences("answer</think>ignored###later", &stop_sequences);

        assert!(stop_matched);
        assert_eq!(trimmed, "answer");
    }

    #[test]
    fn trailing_partial_stop_len_respects_utf8_boundaries() {
        let generated = " <think>\n我";
        let stop_sequences = vec!["我是".to_owned()];
        let hold_back = trailing_partial_stop_len(generated, &stop_sequences);
        let safe_end = generated.len().saturating_sub(hold_back);

        assert_eq!(hold_back, "我".len());
        assert!(generated.is_char_boundary(safe_end));
        assert_eq!(&generated[safe_end..], "我");
    }
}
