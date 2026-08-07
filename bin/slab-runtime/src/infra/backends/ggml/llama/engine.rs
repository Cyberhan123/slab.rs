use crate::infra::backends::ggml;
use slab_agent_tracing::record_json_from_context;
use slab_llama::{
    Llama, LlamaContextParams, LlamaFtype, LlamaInferenceOutput, LlamaLogitBias, LlamaModel,
    LlamaModelParams, LlamaQuantizeParams, LlamaRuntime, LlamaSamplingOptions,
    LlamaSessionSnapshot, LlamaStopInfo,
};
use slab_runtime_core::backend::{
    StreamChunk as BaseStreamChunk, StreamHandle as BaseStreamHandle,
};
use slab_utils::loader::load_library_from_dir;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch};
use tracing::{info, warn};

use crate::domain::models::{
    GgmlLlamaLoadConfig, GgmlLlamaLoadMetadata, GgmlLlamaQuantizeInput, GgmlLlamaQuantizeOutput,
    TextGenerationMetadata, TextGenerationStreamEvent, TextGenerationUsage,
    TextPromptTokensDetails, TextStopMetadata,
};

use super::kv_cache_store::{CachedSession, KvCacheStore, ModelFingerprint};
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
    pub agent_trace: Option<slab_agent_tracing::AgentTraceContext>,
    /// Encoded image bytes for a multimodal turn. Empty for text-only turns
    /// (the common path). When non-empty AND a projector is loaded, the prompt
    /// is expected to carry one [`MTMD_MEDIA_SENTINEL`] per image.
    pub image_parts: Vec<crate::domain::models::TextGenerationImagePart>,
}

/// Stable sentinel substituted for each image by the app-core prompt renderer.
/// The engine replaces every occurrence with the loaded projector's real media
/// marker before handing the prompt to `mtmd_tokenize`. Must match the constant
/// in `slab-app-core` (`domain::services::chat::local::MTMD_MEDIA_SENTINEL`).
const MTMD_MEDIA_SENTINEL: &str = "<<SLAB_MTMD_MEDIA>>";

/// Tokens decoded per `mtmd_helper_eval_chunks` internal `llama_decode` step.
const MM_BATCH_TOKENS: i32 = 512;

#[derive(Debug, Clone)]
pub(crate) struct LlamaDispatchOutput {
    pub text: String,
    pub usage: Option<TextGenerationUsage>,
    pub finish_reason: Option<String>,
    pub metadata: TextGenerationMetadata,
}

const THINK_OPEN_MARKER: &str = "<think";
const THINK_CLOSE_TAG: &str = "</think>";
const SESSION_BINDING_BUSY_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ParsedThinkingOutput {
    content: String,
    reasoning: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ThinkingDelta {
    content: String,
    reasoning: String,
}

#[derive(Debug, Default)]
struct ThinkingStreamState {
    raw: String,
    emitted_content_len: usize,
    emitted_reasoning_len: usize,
    prefilled_thinking: bool,
}

fn trailing_partial_marker_len(raw: &str, marker: &str) -> usize {
    let max = raw.len().min(marker.len().saturating_sub(1));
    (1..=max).rev().find(|len| raw.ends_with(&marker[..*len])).unwrap_or(0)
}

fn normalize_thinking_content_prefix(prefix: &str) -> &str {
    if prefix.trim().is_empty() { "" } else { prefix }
}

#[cfg(test)]
fn parse_thinking_output(raw: &str, complete: bool) -> ParsedThinkingOutput {
    parse_thinking_output_with_prefill(raw, complete, false)
}

fn parse_generated_thinking_output(
    prompt: &str,
    raw: &str,
    complete: bool,
) -> ParsedThinkingOutput {
    parse_thinking_output_with_prefill(raw, complete, prompt_has_prefilled_thinking(prompt))
}

fn prompt_has_prefilled_thinking(prompt: &str) -> bool {
    let Some(open_start) = prompt.rfind(THINK_OPEN_MARKER) else {
        return false;
    };
    if let Some(close_start) = prompt.rfind(THINK_CLOSE_TAG)
        && close_start > open_start
    {
        return false;
    }

    let after_open_marker = &prompt[open_start..];
    let Some(open_end_rel) = after_open_marker.find('>') else {
        return false;
    };
    after_open_marker[open_end_rel + 1..].trim().is_empty()
}

fn parse_thinking_output_with_prefill(
    raw: &str,
    complete: bool,
    prefilled_thinking: bool,
) -> ParsedThinkingOutput {
    if prefilled_thinking {
        return parse_prefilled_thinking_output(raw, complete);
    }

    let Some(open_start) = raw.find(THINK_OPEN_MARKER) else {
        if complete {
            return ParsedThinkingOutput { content: raw.to_owned(), reasoning: String::new() };
        }

        let stable_end =
            raw.len().saturating_sub(trailing_partial_marker_len(raw, THINK_OPEN_MARKER));
        let stable_content = &raw[..stable_end];
        return ParsedThinkingOutput {
            content: if stable_content.trim().is_empty() {
                String::new()
            } else {
                stable_content.to_owned()
            },
            reasoning: String::new(),
        };
    };

    let content_prefix = normalize_thinking_content_prefix(&raw[..open_start]).to_owned();
    let after_open_marker = &raw[open_start..];
    let Some(open_end_rel) = after_open_marker.find('>') else {
        return ParsedThinkingOutput {
            content: if complete { raw.to_owned() } else { content_prefix },
            reasoning: String::new(),
        };
    };

    let reasoning_start = open_start + open_end_rel + 1;
    let after_open = &raw[reasoning_start..];
    if let Some(close_rel) = after_open.find(THINK_CLOSE_TAG) {
        let close_start = reasoning_start + close_rel;
        let close_end = close_start + THINK_CLOSE_TAG.len();
        let mut content = content_prefix;
        content.push_str(&raw[close_end..]);
        return ParsedThinkingOutput {
            content,
            reasoning: raw[reasoning_start..close_start].to_owned(),
        };
    }

    let stable_reasoning_end = if complete {
        raw.len()
    } else {
        raw.len().saturating_sub(trailing_partial_marker_len(raw, THINK_CLOSE_TAG))
    };
    ParsedThinkingOutput {
        content: content_prefix,
        reasoning: raw[reasoning_start..stable_reasoning_end].to_owned(),
    }
}

fn parse_prefilled_thinking_output(raw: &str, complete: bool) -> ParsedThinkingOutput {
    if let Some(close_start) = raw.find(THINK_CLOSE_TAG) {
        let close_end = close_start + THINK_CLOSE_TAG.len();
        return ParsedThinkingOutput {
            content: raw[close_end..].to_owned(),
            reasoning: raw[..close_start].to_owned(),
        };
    }

    let stable_reasoning_end = if complete {
        raw.len()
    } else {
        raw.len().saturating_sub(trailing_partial_marker_len(raw, THINK_CLOSE_TAG))
    };
    ParsedThinkingOutput {
        content: String::new(),
        reasoning: raw[..stable_reasoning_end].to_owned(),
    }
}

impl ThinkingStreamState {
    fn for_prompt(prompt: &str) -> Self {
        Self { prefilled_thinking: prompt_has_prefilled_thinking(prompt), ..Default::default() }
    }

    fn ingest(&mut self, delta: &str) -> ThinkingDelta {
        if delta.is_empty() {
            return ThinkingDelta::default();
        }
        self.raw.push_str(delta);
        self.emit(false)
    }

    fn finish(&mut self) -> ThinkingDelta {
        self.emit(true)
    }

    fn emit(&mut self, complete: bool) -> ThinkingDelta {
        let parsed =
            parse_thinking_output_with_prefill(&self.raw, complete, self.prefilled_thinking);
        let content = parsed.content.get(self.emitted_content_len..).unwrap_or_default().to_owned();
        let reasoning =
            parsed.reasoning.get(self.emitted_reasoning_len..).unwrap_or_default().to_owned();
        self.emitted_content_len = parsed.content.len();
        self.emitted_reasoning_len = parsed.reasoning.len();
        ThinkingDelta { content, reasoning }
    }
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

fn llama_request_payload(request: &LlamaDispatchRequest) -> serde_json::Value {
    serde_json::json!({
        "prompt": request.prompt,
        "max_tokens": request.max_tokens,
        "session_key": request.session_key,
        "gbnf": request.gbnf,
        "temperature": request.temperature,
        "top_p": request.top_p,
        "top_k": request.top_k,
        "min_p": request.min_p,
        "repetition_penalty": request.repetition_penalty,
        "presence_penalty": request.presence_penalty,
        "ignore_eos": request.ignore_eos,
        "logit_bias": request.logit_bias,
        "stop_sequences": request.stop_sequences,
    })
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
    Busy { request_id: String, started_at: Instant },
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
        Some(SessionBinding::Busy { request_id, started_at }) => {
            if started_at.elapsed() < SESSION_BINDING_BUSY_TTL {
                warn!(
                    session_key = key,
                    request_id, "session binding is already active; rejecting concurrent request"
                );
                return Err(GGMLLlamaEngineError::SessionKeyBusy { key: key.to_owned() });
            }

            warn!(
                session_key = key,
                request_id,
                busy_seconds = started_at.elapsed().as_secs(),
                "session binding exceeded busy TTL; recovering by creating a fresh session"
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
    /// Optional on-disk kv-cache store. When `None`, the engine only
    /// keeps the in-process snapshot cache.
    kv_cache: Mutex<Option<Arc<KvCacheStore>>>,
    /// Fingerprint of the currently loaded model (computed at load); used as the
    /// top-level kv-cache directory. `None` until a model is loaded.
    model_fp: Mutex<Option<ModelFingerprint>>,
    /// Runtime library directory (where llama.dll / mtmd.dll live). Retained so
    /// the mtmd projector can be loaded on demand.
    lib_dir: PathBuf,
    /// Optional multimodal (mtmd) projector loaded when `mmproj_path` is set on
    /// the model load config. `None` for text-only models. Holds the `Mtmd`
    /// library handle (kept resident) and the bound `MtmdContext`.
    mmproj: Mutex<Option<(Arc<slab_mtmd::Mtmd>, Arc<slab_mtmd::MtmdContext>)>>,
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
                kv_cache: Mutex::new(None),
                model_fp: Mutex::new(None),
                lib_dir: lib_dir.to_path_buf(),
                mmproj: Mutex::new(None),
            }))
        })
    }

    fn lock_session_bindings(
        &self,
    ) -> Result<MutexGuard<'_, HashMap<String, SessionBinding>>, GGMLLlamaEngineError> {
        self.session_bindings.lock().map_err(|_| GGMLLlamaEngineError::LockPoisoned {
            operation: "lock llama session bindings",
        })
    }

    /// Install an on-disk kv-cache store, enabling on-disk persistence. When
    /// unset, the engine falls back to the in-process snapshot cache only.
    /// Best-effort: a store is installed at most once; subsequent calls are ignored.
    pub(crate) fn install_kv_cache(&self, store: KvCacheStore) {
        if let Ok(mut guard) = self.kv_cache.lock()
            && guard.is_none()
        {
            *guard = Some(Arc::new(store));
        }
    }

    /// Snapshot the current on-disk kv-cache store + model fingerprint, if both
    /// are available. Used by the restore/persist hooks.
    fn disk_cache_handle(&self) -> Option<(Arc<KvCacheStore>, ModelFingerprint)> {
        let store = self.kv_cache.lock().ok()?.as_ref()?.clone();
        let fp = self.model_fp.lock().ok()?.as_ref()?.clone();
        Some((store, fp))
    }

    /// Best-effort: load a disk snapshot for `session_key` so it can seed an
    /// in-process `Ready` binding on the first turn of a restored session.
    fn load_disk_session(&self, session_key: &str) -> Option<CachedSession> {
        let (store, fp) = self.disk_cache_handle()?;
        store.load(&fp, session_key)
    }

    /// Best-effort: mirror a committed snapshot to disk (fire-and-forget, off the
    /// async hot path). Never fails the turn.
    fn persist_disk_session(
        &self,
        session_key: &str,
        snapshot: &LlamaSessionSnapshot,
        cached_prompt: String,
        grammar: Option<&str>,
    ) {
        let (store, fp) = match self.disk_cache_handle() {
            Some(handle) => handle,
            None => return,
        };
        let snapshot = snapshot.clone();
        let session_key = session_key.to_owned();
        let grammar = grammar.map(str::to_owned);

        // Fire-and-forget on the blocking pool; errors are logged inside `save`.
        // Fall back to a synchronous write when no ambient tokio runtime exists.
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                handle.spawn_blocking(move || {
                    store.save(&fp, &session_key, &snapshot, &cached_prompt, grammar.as_deref());
                });
            }
            Err(_) => {
                store.save(&fp, &session_key, &snapshot, &cached_prompt, grammar.as_deref());
            }
        }
    }

    /// Load a model and start a multi-worker inference engine.
    ///
    /// Any previously loaded model/engine are replaced.
    pub fn load_model_with_workers<P: AsRef<Path>>(
        &self,
        path_to_model: P,
        model_params: LlamaModelParams,
        mut ctx_params: LlamaContextParams,
        num_workers: usize,
        context_length: Option<u32>,
        free_vram_bytes: Option<u64>,
    ) -> Result<GgmlLlamaLoadMetadata, ggml::EngineError> {
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
        self.lock_session_bindings()?.clear();

        let path =
            path_to_model.as_ref().to_str().ok_or(GGMLLlamaEngineError::InvalidModelPathUtf8)?;

        let model =
            Arc::new(self.instance.load_model_from_file(path, model_params).map_err(|source| {
                GGMLLlamaEngineError::LoadModel { model_path: path.to_string(), source }
            })?);
        let training_context_length =
            u32::try_from(model.n_ctx_train()).ok().filter(|value| *value > 0);
        let chat_template = model.chat_template().unwrap_or_default();

        // Resolve the context window: an explicit value, or `auto` = the largest
        // context that fits in GPU VRAM (capped at the model's training context).
        ctx_params.n_ctx = context_length.unwrap_or_else(|| {
            resolve_auto_n_ctx(
                training_context_length,
                model.model_size(),
                model.n_layer().max(0) as u32,
                model.n_head_kv().max(0) as u32,
                model.n_embd().max(0) as u32,
                model.n_head().max(0) as u32,
                free_vram_bytes,
            )
        });
        if ctx_params.n_batch > ctx_params.n_ctx {
            ctx_params.n_batch = ctx_params.n_ctx;
        }
        if ctx_params.n_ubatch > ctx_params.n_ctx {
            ctx_params.n_ubatch = ctx_params.n_ctx;
        }

        let engine = LlamaRuntime::start(num_workers, Arc::clone(&model), ctx_params)
            .map_err(GGMLLlamaEngineError::from)?;
        let loaded_context_length = engine.context_length();
        let context_length = (loaded_context_length > 0).then_some(loaded_context_length);

        // Compute the model fingerprint before `model` is moved into the slot —
        // it keys the on-disk kv-cache.
        let model_fp = ModelFingerprint::compute(path, model.n_params(), model.model_size());
        if let Ok(mut guard) = self.model_fp.lock() {
            *guard = Some(model_fp);
        }

        *write_lock = Some(engine);
        *model_write_lock = Some(model);
        Ok(GgmlLlamaLoadMetadata { context_length, training_context_length, chat_template })
    }

    pub(crate) fn load_model_from_config(
        &self,
        config: &GgmlLlamaLoadConfig,
    ) -> Result<GgmlLlamaLoadMetadata, ggml::EngineError> {
        let ctx_params = LlamaContextParams {
            kv_unified: true,
            flash_attn: config.flash_attn,
            ..Default::default()
        };
        // `context_length` (None = auto) is resolved inside load_model_with_workers
        // once the model is loaded and its native training context is known.
        let metadata = self.load_model_with_workers(
            &config.model_path,
            LlamaModelParams::default(),
            ctx_params,
            config.engine_workers,
            config.context_length,
            config.free_vram_bytes,
        )?;

        // Load the multimodal projector when an mmproj path is configured. Best
        // effort: a null init (wrong file / unsupported projector) logs a
        // warning and downgrades to text-only rather than failing the model load.
        if let Some(mmproj_path) = config.mmproj_path.as_ref() {
            match self.load_mmproj(mmproj_path) {
                Ok(supports_vision) => {
                    info!(
                        mmproj = %mmproj_path.display(),
                        supports_vision, "multimodal projector loaded"
                    );
                }
                Err(error) => {
                    warn!(
                        mmproj = %mmproj_path.display(),
                        error = %error,
                        "failed to load multimodal projector; falling back to text-only"
                    );
                    if let Ok(mut guard) = self.mmproj.lock() {
                        *guard = None;
                    }
                }
            }
        } else if let Ok(mut guard) = self.mmproj.lock() {
            *guard = None;
        }

        Ok(metadata)
    }

    /// Load (or replace) the mtmd projector bound to the currently loaded model.
    /// Returns whether the projector supports vision.
    fn load_mmproj(&self, mmproj_path: &Path) -> Result<bool, ggml::EngineError> {
        let model = self.require_model()?;
        let mtmd = slab_mtmd::Mtmd::new(&self.lib_dir).map_err(|source| {
            GGMLLlamaEngineError::InitializeDynamicLibrary {
                path: self.lib_dir.join("mtmd"),
                source,
            }
        })?;
        let ctx = slab_mtmd::MtmdContext::init_from_file(
            &mtmd,
            mmproj_path,
            model.as_ref(),
            slab_mtmd::MtmdContextParams::default(),
        )
        .map_err(|error| GGMLLlamaEngineError::MultimodalLoad {
            mmproj_path: mmproj_path.display().to_string(),
            message: error.to_string(),
        })?;
        let supports_vision = ctx.supports_vision();
        if let Ok(mut guard) = self.mmproj.lock() {
            *guard = Some((Arc::new(mtmd), Arc::new(ctx)));
        }
        Ok(supports_vision)
    }

    /// Clone the loaded projector pair (library + context), if any (for
    /// multimodal prefill).
    fn require_mmproj(
        &self,
    ) -> Result<(Arc<slab_mtmd::Mtmd>, Arc<slab_mtmd::MtmdContext>), GGMLLlamaEngineError> {
        let guard = self.mmproj.lock().map_err(|_| GGMLLlamaEngineError::LockPoisoned {
            operation: "lock mmproj projector",
        })?;
        guard.as_ref().map(|(mtmd, ctx)| (Arc::clone(mtmd), Arc::clone(ctx))).ok_or_else(|| {
            GGMLLlamaEngineError::MultimodalLoad {
                mmproj_path: String::new(),
                message: "no multimodal projector loaded".to_owned(),
            }
        })
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
        request: &LlamaDispatchRequest,
        full_prompt: String,
        logit_bias: &[LlamaLogitBias],
    ) -> Result<PreparedSession, ggml::EngineError> {
        let Some(key) = request.session_key.clone() else {
            return Ok(PreparedSession {
                key: None,
                sid: None,
                delta_prompt: full_prompt.clone(),
                full_prompt,
                cached_tokens: 0,
            });
        };

        // Best-effort: try to restore a disk snapshot BEFORE taking the bindings
        // lock so disk I/O doesn't block other sessions under the lock.
        let disk_session = self.load_disk_session(&key);

        let plan;

        {
            let mut bindings = self.lock_session_bindings()?;
            // Pre-warm the in-process cache from disk if no live binding exists;
            // plan_session_reuse's `Ready` arm then handles the prefix-delta check.
            if bindings.get(&key).is_none()
                && let Some(cached) = disk_session
            {
                bindings.insert(
                    key.clone(),
                    SessionBinding::Ready {
                        snapshot: cached.snapshot,
                        cached_prompt: cached.cached_prompt,
                        grammar: cached.grammar,
                    },
                );
            }
            plan =
                plan_session_reuse(&key, bindings.get(&key), &full_prompt, request.gbnf.as_deref())
                    .map_err(ggml::EngineError::from)?;
            let request_id = request
                .agent_trace
                .as_ref()
                .map(|trace| {
                    let turn = trace
                        .turn_index
                        .map(|index| index.to_string())
                        .unwrap_or_else(|| "unknown".to_owned());
                    format!("{}:{turn}", trace.session_id)
                })
                .unwrap_or_else(|| "untraced".to_owned());
            bindings.insert(
                key.clone(),
                SessionBinding::Busy { request_id, started_at: Instant::now() },
            );
        }

        let options = LlamaSamplingOptions {
            gbnf: request.gbnf.clone(),
            temperature: request.temperature,
            top_p: request.top_p,
            top_k: request.top_k,
            min_p: request.min_p,
            repetition_penalty: request.repetition_penalty,
            presence_penalty: request.presence_penalty,
            ignore_eos: request.ignore_eos,
            logit_bias: logit_bias.to_vec(),
        };

        let (sid, delta_prompt, cached_tokens) = match plan {
            SessionReusePlan::CreateFresh { delta_prompt, cached_tokens } => {
                match self.create_session_with_options(options.clone()).await {
                    Ok(sid) => (Some(sid), delta_prompt, cached_tokens),
                    Err(error) => {
                        self.lock_session_bindings()?.remove(&key);
                        return Err(error);
                    }
                }
            }
            SessionReusePlan::RestoreSnapshot { snapshot, delta_prompt, cached_tokens } => {
                match self.create_session_from_snapshot(snapshot, options).await {
                    Ok(sid) => (Some(sid), delta_prompt, cached_tokens),
                    Err(error) => {
                        self.lock_session_bindings()?.remove(&key);
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
            self.lock_session_bindings()?.remove(&key);
            return Err(error);
        }

        let mut cached_prompt = String::with_capacity(full_prompt.len() + generated.len());
        cached_prompt.push_str(full_prompt);
        cached_prompt.push_str(generated);

        // Best-effort disk mirror before the snapshot is moved.
        self.persist_disk_session(&key, &snapshot, cached_prompt.clone(), gbnf.as_deref());

        self.lock_session_bindings()?
            .insert(key, SessionBinding::Ready { snapshot, cached_prompt, grammar: gbnf });
        Ok(())
    }

    async fn drop_managed_session(&self, key: Option<String>, sid: Option<SessionId>) {
        if let Some(key) = key {
            match self.lock_session_bindings() {
                Ok(mut bindings) => {
                    bindings.remove(&key);
                }
                Err(error) => {
                    warn!(%error, "failed to remove llama managed session binding");
                }
            }
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
        let commit_gbnf = request.gbnf.clone();
        let stop_sequences = request.stop_sequences.clone();
        let agent_trace = request.agent_trace.clone();
        if let Some(trace_context) = agent_trace.as_ref() {
            record_json_from_context(
                trace_context,
                "slab-runtime",
                "llama_request",
                llama_request_payload(&request),
            );
        }
        let logit_bias = self.resolve_logit_bias(request.logit_bias.as_ref())?;
        let prepared = self.prepare_managed_session(&request, prompt, &logit_bias).await?;

        match self
            .inference(
                &prepared.delta_prompt,
                max_tokens,
                prepared.sid,
                gbnf,
                request.ignore_eos,
                &logit_bias,
                &request.image_parts,
            )
            .await
        {
            Ok(output) => {
                // Apply stop sequence trimming to the generated text before committing.
                let (trimmed_text, stop_matched) =
                    apply_stop_sequences(&output.text, &stop_sequences);
                if stop_matched && let Some(trace_context) = agent_trace.as_ref() {
                    record_json_from_context(
                        trace_context,
                        "slab-runtime",
                        "llama_stop_matched",
                        serde_json::json!({
                            "mode": "text",
                            "stop_sequences": stop_sequences,
                        }),
                    );
                }
                let usage =
                    self.build_usage(&prepared.full_prompt, &trimmed_text, prepared.cached_tokens);
                let finish_reason = if stop_matched {
                    Some("stop".to_owned())
                } else {
                    output.stop.as_ref().map(|stop| stop.finish_reason.clone())
                };
                let parsed =
                    parse_generated_thinking_output(&prepared.full_prompt, &trimmed_text, true);
                let mut metadata =
                    output.stop.as_ref().map(stop_info_to_metadata).unwrap_or_default();
                let reasoning = parsed.reasoning.trim();
                if !reasoning.is_empty() {
                    metadata.reasoning_content = Some(reasoning.to_owned());
                }
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
                    if let Some(trace_context) = agent_trace.as_ref() {
                        record_json_from_context(
                            trace_context,
                            "slab-runtime",
                            "llama_session_commit_failed",
                            serde_json::json!({ "error": error.to_string() }),
                        );
                    }
                    warn!(error = %error, "failed to persist llama session snapshot after inference");
                } else if let Some(trace_context) = agent_trace.as_ref() {
                    record_json_from_context(
                        trace_context,
                        "slab-runtime",
                        "llama_session_committed",
                        serde_json::json!({ "mode": "text" }),
                    );
                }
                if let Some(trace_context) = agent_trace.as_ref() {
                    record_json_from_context(
                        trace_context,
                        "slab-runtime",
                        "llama_response",
                        serde_json::json!({
                            "text": parsed.content,
                            "raw_text": output.text,
                            "trimmed_text": trimmed_text,
                            "finish_reason": finish_reason,
                            "usage": usage,
                            "metadata": metadata,
                        }),
                    );
                }
                Ok(LlamaDispatchOutput { text: parsed.content, usage, finish_reason, metadata })
            }
            Err(error) => {
                if let Some(trace_context) = agent_trace.as_ref() {
                    record_json_from_context(
                        trace_context,
                        "slab-runtime",
                        "llama_error",
                        serde_json::json!({ "error": error.to_string() }),
                    );
                }
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
        let commit_gbnf = request.gbnf.clone();
        let stop_sequences = request.stop_sequences.clone();
        let agent_trace = request.agent_trace.clone();
        if let Some(trace_context) = agent_trace.as_ref() {
            record_json_from_context(
                trace_context,
                "slab-runtime",
                "llama_request",
                llama_request_payload(&request),
            );
        }
        let logit_bias = self.resolve_logit_bias(request.logit_bias.as_ref())?;
        let prepared = self.prepare_managed_session(&request, prompt, &logit_bias).await?;

        let (mut llama_rx, sid) = match self
            .inference_stream(
                &prepared.delta_prompt,
                max_tokens,
                prepared.sid,
                gbnf,
                request.ignore_eos,
                &logit_bias,
                &request.image_parts,
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
            let mut thinking_state = ThinkingStreamState::for_prompt(&full_prompt);
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
                                if let Some(trace_context) = agent_trace.as_ref() {
                                    record_json_from_context(
                                        trace_context,
                                        "slab-runtime",
                                        "llama_cancel_failed",
                                        serde_json::json!({
                                            "session_id": sid,
                                            "error": error.to_string(),
                                        }),
                                    );
                                }
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
                                if let Some(trace_context) = agent_trace.as_ref() {
                                    record_json_from_context(
                                        trace_context,
                                        "slab-runtime",
                                        "llama_stream_token",
                                        serde_json::json!({ "text": text }),
                                    );
                                }
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
                                        if let Some(trace_context) = agent_trace.as_ref() {
                                            record_json_from_context(
                                                trace_context,
                                                "slab-runtime",
                                                "llama_stop_matched",
                                                serde_json::json!({
                                                    "mode": "stream",
                                                    "stop_index": stop_index,
                                                    "stop_sequences": stop_sequences,
                                                }),
                                            );
                                        }
                                        let safe_end = stop_index;
                                        if safe_end > forwarded_len {
                                            let forward_text = generated[forwarded_len..safe_end].to_owned();
                                            forwarded_len = safe_end;
                                            if forward_thinking_delta(
                                                &stream_tx,
                                                thinking_state.ingest(&forward_text),
                                            )
                                            .await
                                            .is_err()
                                            {
                                                forward_failed = true;
                                            }
                                        }
                                        // Truncate generated to the stop boundary for session commit.
                                        generated.truncate(safe_end);
                                        // Cancel the backend generation.
                                        if let Err(error) = engine.cancel_generate(sid).await {
                                            if let Some(trace_context) = agent_trace.as_ref() {
                                                record_json_from_context(
                                                    trace_context,
                                                    "slab-runtime",
                                                    "llama_cancel_failed",
                                                    serde_json::json!({
                                                        "session_id": sid,
                                                        "error": error.to_string(),
                                                    }),
                                                );
                                            }
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
                                        if forward_thinking_delta(
                                            &stream_tx,
                                            thinking_state.ingest(&forward_text),
                                        )
                                        .await
                                        .is_err()
                                        {
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
                                    if forward_thinking_delta(&stream_tx, thinking_state.ingest(&text))
                                        .await
                                        .is_err()
                                    {
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
                                if let Some(trace_context) = agent_trace.as_ref() {
                                    record_json_from_context(
                                        trace_context,
                                        "slab-runtime",
                                        "llama_stream_stop",
                                        serde_json::json!({
                                            "finish_reason": stop.finish_reason,
                                            "stop_token_id": stop.stop_token_id,
                                            "stop_token_text": stop.stop_token_text,
                                            "stop_token_kind": stop.stop_token_kind,
                                        }),
                                    );
                                }
                            }
                            StreamChunk::Error(error) => {
                                stream_error = true;
                                if let Some(trace_context) = agent_trace.as_ref() {
                                    record_json_from_context(
                                        trace_context,
                                        "slab-runtime",
                                        "llama_stream_error",
                                        serde_json::json!({ "error": error }),
                                    );
                                }
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
                if forward_thinking_delta(&stream_tx, thinking_state.ingest(&tail)).await.is_err() {
                    forward_failed = true;
                }
            }

            let effectively_completed = completed || stop_matched;

            if effectively_completed
                && !forward_failed
                && !stream_error
                && forward_thinking_delta(&stream_tx, thinking_state.finish()).await.is_err()
            {
                forward_failed = true;
            }

            if effectively_completed && !forward_failed && !stream_error && !cancelled {
                let finish_reason = terminal_finish_reason
                    .clone()
                    .or_else(|| stop_matched.then(|| "stop".to_owned()));
                if let Some(finish_reason) = finish_reason {
                    if let Some(trace_context) = agent_trace.as_ref() {
                        record_json_from_context(
                            trace_context,
                            "slab-runtime",
                            "llama_stream_finish",
                            serde_json::json!({
                                "finish_reason": finish_reason,
                                "metadata": terminal_metadata,
                                "generated": generated,
                            }),
                        );
                    }
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
            {
                if let Some(trace_context) = agent_trace.as_ref() {
                    record_json_from_context(
                        trace_context,
                        "slab-runtime",
                        "llama_stream_usage",
                        serde_json::json!({ "usage": usage }),
                    );
                }
                if stream_tx
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
            }

            // Resolve the managed session (Busy -> Ready, or drop) BEFORE sending
            // the stream `Done` marker. The client starts its next inference as
            // soon as it observes `Done`; committing after it sent `Done` left a
            // window where a rapid back-to-back inference (e.g. after an
            // auto-allowed tool such as `plan`, which does not gate on approval)
            // saw the session key still Busy and was rejected with SessionKeyBusy.
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
                    if let Some(trace_context) = agent_trace.as_ref() {
                        record_json_from_context(
                            trace_context,
                            "slab-runtime",
                            "llama_session_commit_failed",
                            serde_json::json!({ "error": error.to_string() }),
                        );
                    }
                    warn!(error = %error, "failed to persist llama session snapshot after stream");
                } else if let Some(trace_context) = agent_trace.as_ref() {
                    record_json_from_context(
                        trace_context,
                        "slab-runtime",
                        "llama_session_committed",
                        serde_json::json!({ "mode": "stream" }),
                    );
                }
            } else {
                if let Some(trace_context) = agent_trace.as_ref() {
                    record_json_from_context(
                        trace_context,
                        "slab-runtime",
                        "llama_session_dropped",
                        serde_json::json!({
                            "completed": effectively_completed,
                            "forward_failed": forward_failed,
                            "stream_error": stream_error,
                            "cancelled": cancelled,
                        }),
                    );
                }
                engine.drop_managed_session(key, Some(sid)).await;
            }

            if effectively_completed && !forward_failed && !stream_error {
                // `Done` is the client's stream-end signal. Sent only after the
                // session was committed above, so a follow-on inference sees a
                // Ready binding. Ignore send errors: `forward_failed` is no
                // longer read after this point.
                let _ = stream_tx.send(BaseStreamChunk::Done).await;
            }
        });

        Ok(stream_rx)
    }

    /// Create a new session with optional GBNF and sampling overrides.
    pub async fn create_session_with_options(
        &self,
        options: LlamaSamplingOptions,
    ) -> Result<SessionId, ggml::EngineError> {
        let engine = self.require_engine()?;
        engine
            .create_session_with_options(options)
            .await
            .map_err(GGMLLlamaEngineError::from)
            .map_err(Into::into)
    }

    async fn create_session_from_snapshot(
        &self,
        snapshot: LlamaSessionSnapshot,
        options: LlamaSamplingOptions,
    ) -> Result<SessionId, ggml::EngineError> {
        let engine = self.require_engine()?;
        engine
            .create_session_from_snapshot(snapshot, options)
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
    /// Multimodal prefill: tokenize `prompt` interleaved with `image_parts`
    /// using the loaded mtmd projector, then drive `mtmd_helper_eval_chunks`
    /// against the session's live context via the `run_with_context` escape
    /// hatch. Advances the session's `n_past` so subsequent generation continues
    /// from the new position. Degrades to a plain text append when no projector
    /// is loaded.
    async fn prefill_multimodal(
        &self,
        sid: SessionId,
        prompt: &str,
        image_parts: &[crate::domain::models::TextGenerationImagePart],
    ) -> Result<(), ggml::EngineError> {
        let (mtmd, mtmd_ctx) = match self.require_mmproj() {
            Ok(pair) => pair,
            Err(_) => {
                tracing::warn!(
                    "image parts present but no mmproj projector loaded; treating turn as text"
                );
                return self.append_input(sid, prompt.to_string()).await;
            }
        };

        // Substitute the app-core sentinel with the projector's real media
        // marker (one per image, in order).
        let marker = mtmd_ctx.marker();
        let resolved_marker = if marker.is_empty() { MTMD_MEDIA_SENTINEL } else { marker };
        let prompt = prompt.replace(MTMD_MEDIA_SENTINEL, resolved_marker);

        // Build bitmaps from the encoded image bytes (mtmd decodes via projector).
        let bitmaps: Vec<slab_mtmd::MtmdBitmap> = image_parts
            .iter()
            .map(|part| slab_mtmd::MtmdBitmap::from_buf(&mtmd_ctx, &part.data, false))
            .collect::<slab_mtmd::Result<_>>()
            .map_err(|error| GGMLLlamaEngineError::MultimodalLoad {
                mmproj_path: String::new(),
                message: format!("bitmap decode failed: {error}"),
            })?;
        let bitmap_refs: Vec<&slab_mtmd::MtmdBitmap> = bitmaps.iter().collect();

        let mut chunks = slab_mtmd::MtmdInputChunks::new(&mtmd);
        let input_text = slab_mtmd::MtmdInputText::new(&prompt, true, true).map_err(|error| {
            GGMLLlamaEngineError::MultimodalLoad {
                mmproj_path: String::new(),
                message: format!("mtmd input text failed: {error}"),
            }
        })?;
        mtmd_ctx.tokenize(&input_text, &bitmap_refs, &mut chunks).map_err(|error| {
            GGMLLlamaEngineError::MultimodalLoad {
                mmproj_path: String::new(),
                message: format!("mtmd tokenize failed: {error}"),
            }
        })?;

        let engine = self.require_engine()?;
        let mtmd_ctx_for_closure = Arc::clone(&mtmd_ctx);
        engine
            .run_with_context(
                sid,
                Box::new(move |rc: &mut slab_llama::RunContext| {
                    let mut new_n_past = rc.n_past;
                    if let Err(error) = mtmd_ctx_for_closure.eval_chunks_raw(
                        rc.ctx as *mut std::ffi::c_void,
                        &chunks,
                        rc.n_past,
                        rc.seq_id,
                        MM_BATCH_TOKENS,
                        true,
                        &mut new_n_past,
                    ) {
                        return Err(error.to_string());
                    }
                    rc.set_new_n_past(new_n_past);
                    Ok(())
                }),
            )
            .await
            .map_err(GGMLLlamaEngineError::from)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn inference(
        &self,
        prompt: &str,
        max_tokens: usize,
        session_id: Option<SessionId>,
        gbnf: Option<String>,
        ignore_eos: bool,
        logit_bias: &[LlamaLogitBias],
        image_parts: &[crate::domain::models::TextGenerationImagePart],
    ) -> Result<LlamaInferenceOutput, ggml::EngineError> {
        let sid = match session_id {
            Some(sid) => sid,
            None => {
                self.create_session_with_options(LlamaSamplingOptions {
                    gbnf,
                    ignore_eos,
                    logit_bias: logit_bias.to_vec(),
                    ..LlamaSamplingOptions::default()
                })
                .await?
            }
        };
        let should_end = session_id.is_none();

        let prefill = if !image_parts.is_empty() {
            self.prefill_multimodal(sid, prompt, image_parts).await
        } else {
            self.append_input(sid, prompt.to_string()).await
        };
        if let Err(error) = prefill {
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
    #[allow(clippy::too_many_arguments)]
    pub async fn inference_stream(
        &self,
        prompt: &str,
        max_tokens: usize,
        session_id: Option<SessionId>,
        gbnf: Option<String>,
        ignore_eos: bool,
        logit_bias: &[LlamaLogitBias],
        image_parts: &[crate::domain::models::TextGenerationImagePart],
    ) -> Result<(StreamHandle, SessionId), ggml::EngineError> {
        let sid = match session_id {
            Some(sid) => sid,
            None => {
                self.create_session_with_options(LlamaSamplingOptions {
                    gbnf,
                    ignore_eos,
                    logit_bias: logit_bias.to_vec(),
                    ..LlamaSamplingOptions::default()
                })
                .await?
            }
        };

        let prefill = if !image_parts.is_empty() {
            self.prefill_multimodal(sid, prompt, image_parts).await
        } else {
            self.append_input(sid, prompt.to_string()).await
        };
        if let Err(error) = prefill {
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
        self.lock_session_bindings()?.clear();
        Ok(())
    }

    /// Unload the current model and stop all inference workers.
    /// Quantize `input.input_path` into `input.output_path` using the engine's
    /// llama library handle. Does not need a loaded inference context — only the
    /// library handle initialised at construction (so the backend must have been
    /// loaded at least once).
    pub(crate) async fn quantize(
        &self,
        input: GgmlLlamaQuantizeInput,
    ) -> Result<GgmlLlamaQuantizeOutput, ggml::EngineError> {
        let params = LlamaQuantizeParams {
            nthread: input.nthread.unwrap_or(0),
            ftype: LlamaFtype::from_raw(input.ftype),
            allow_requantize: input.allow_requantize,
            quantize_output_tensor: input.quantize_output_tensor,
            only_copy: input.only_copy,
            pure: input.pure,
            keep_split: input.keep_split,
            dry_run: input.dry_run,
            ..LlamaQuantizeParams::default()
        };
        let layers_processed = self
            .instance
            .model_quantize(&input.input_path, &input.output_path, &params)
            .map_err(|source| GGMLLlamaEngineError::Quantize {
                input_path: input.input_path.clone(),
                output_path: input.output_path.clone(),
                source,
            })?;
        Ok(GgmlLlamaQuantizeOutput { layers_processed, output_path: input.output_path })
    }

    pub fn unload(&self) -> Result<(), ggml::EngineError> {
        Ok(self.do_unload()?)
    }
}

async fn forward_thinking_delta(
    stream_tx: &mpsc::Sender<BaseStreamChunk>,
    delta: ThinkingDelta,
) -> Result<(), mpsc::error::SendError<BaseStreamChunk>> {
    if !delta.reasoning.is_empty() {
        stream_tx.send(BaseStreamChunk::Json(reasoning_event_payload(delta.reasoning))).await?;
    }
    if !delta.content.is_empty() {
        stream_tx.send(BaseStreamChunk::Token(delta.content)).await?;
    }
    Ok(())
}

fn reasoning_event_payload(reasoning: String) -> serde_json::Value {
    serde_json::to_value(TextGenerationStreamEvent {
        metadata: Some(TextGenerationMetadata {
            reasoning_content: Some(reasoning),
            ..Default::default()
        }),
        ..Default::default()
    })
    .expect("llama stream reasoning event should serialize")
}

/// VRAM budget reserved above the KV cache when sizing an `auto` context.
const AUTO_CONTEXT_VRAM_BUFFER: u64 = 2 * 1024 * 1024 * 1024;
/// Conservative fallback context (no VRAM signal or degenerate model dims).
const AUTO_CONTEXT_FALLBACK: u32 = 8192;
/// `auto` contexts are floored to a multiple of this many tokens.
const AUTO_CONTEXT_QUANTUM: u32 = 512;
/// KV cache element size (f16 k + f16 v are llama.cpp's defaults).
const F16_BYTES: u64 = 2;

/// Resolve an `auto` context length: the largest `n_ctx` whose KV cache fits in
/// `free_vram_bytes` (minus the model weights and a 2 GB buffer), floored to a
/// 512-token quantum and capped at the model's native training context.
///
/// With no VRAM signal (or degenerate dimensions) it falls back to
/// `min(n_ctx_train, AUTO_CONTEXT_FALLBACK)` to stay OOM-safe.
pub(crate) fn resolve_auto_n_ctx(
    n_ctx_train: Option<u32>,
    model_size_bytes: u64,
    n_layer: u32,
    n_head_kv: u32,
    n_embd: u32,
    n_head: u32,
    free_vram_bytes: Option<u64>,
) -> u32 {
    let cap = n_ctx_train.unwrap_or(AUTO_CONTEXT_FALLBACK);
    let fallback = cap.min(AUTO_CONTEXT_FALLBACK);

    let Some(free_vram) = free_vram_bytes else {
        return fallback;
    };

    // KV bytes per token = 2 (k+v) · n_layer · n_head_kv · head_dim · sizeof(f16).
    // checked_div also covers the degenerate case (bytes_per_token == 0).
    let head_dim = n_embd.checked_div(n_head).unwrap_or(0);
    let bytes_per_token = n_layer as u64 * 2 * n_head_kv as u64 * head_dim as u64 * F16_BYTES;
    let budget =
        free_vram.saturating_sub(model_size_bytes).saturating_sub(AUTO_CONTEXT_VRAM_BUFFER);
    let Some(max_for_vram) = budget.checked_div(bytes_per_token).map(|value| value as u32) else {
        return fallback;
    };
    let quantized = (max_for_vram / AUTO_CONTEXT_QUANTUM) * AUTO_CONTEXT_QUANTUM;
    quantized.clamp(AUTO_CONTEXT_QUANTUM, cap)
}

#[cfg(test)]
mod tests {
    use super::{
        ParsedThinkingOutput, SESSION_BINDING_BUSY_TTL, SessionBinding, SessionReusePlan,
        ThinkingDelta, ThinkingStreamState, parse_generated_thinking_output, parse_thinking_output,
        plan_session_reuse,
    };
    use slab_llama::LlamaSessionSnapshot;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

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
    fn plan_session_reuse_rejects_active_busy_binding() {
        let binding =
            SessionBinding::Busy { request_id: "request-1".to_owned(), started_at: Instant::now() };

        let error = plan_session_reuse("chat-1", Some(&binding), "hello", None)
            .expect_err("active busy binding should reject concurrent requests");

        assert!(
            matches!(error, super::GGMLLlamaEngineError::SessionKeyBusy { key } if key == "chat-1")
        );
    }

    #[test]
    fn plan_session_reuse_recovers_from_stale_busy_binding() {
        let binding = SessionBinding::Busy {
            request_id: "request-1".to_owned(),
            started_at: Instant::now() - SESSION_BINDING_BUSY_TTL - Duration::from_secs(1),
        };

        let plan = plan_session_reuse("chat-1", Some(&binding), "hello", None)
            .expect("stale busy binding should recover with a fresh session");
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

    #[test]
    fn parse_thinking_output_extracts_reasoning_block() {
        let parsed = parse_thinking_output("<think>step one</think>\n\nfinal answer", true);
        assert_eq!(
            parsed,
            ParsedThinkingOutput {
                content: "\n\nfinal answer".to_owned(),
                reasoning: "step one".to_owned(),
            }
        );
    }

    #[test]
    fn parse_thinking_output_ignores_whitespace_prefix_before_think() {
        let parsed = parse_thinking_output("\n\n<think>step one</think>\n\nfinal answer", true);
        assert_eq!(
            parsed,
            ParsedThinkingOutput {
                content: "\n\nfinal answer".to_owned(),
                reasoning: "step one".to_owned(),
            }
        );
    }

    #[test]
    fn parse_thinking_output_holds_partial_open_marker_while_streaming() {
        let parsed = parse_thinking_output("answer<th", false);
        assert_eq!(
            parsed,
            ParsedThinkingOutput { content: "answer".to_owned(), reasoning: String::new() }
        );
    }

    #[test]
    fn parse_generated_thinking_output_handles_prompt_prefilled_think() {
        let prompt = "<|im_start|>assistant\n<think>\n";
        let parsed = parse_generated_thinking_output(prompt, "chain</think>\n\nfinal answer", true);

        assert_eq!(
            parsed,
            ParsedThinkingOutput {
                content: "\n\nfinal answer".to_owned(),
                reasoning: "chain".to_owned(),
            }
        );
    }

    #[test]
    fn parse_generated_thinking_output_ignores_closed_prompt_think() {
        let prompt = "<|im_start|>assistant\n<think>\n\n</think>\n\n";
        let parsed = parse_generated_thinking_output(prompt, "final answer", true);

        assert_eq!(
            parsed,
            ParsedThinkingOutput { content: "final answer".to_owned(), reasoning: String::new() }
        );
    }

    #[test]
    fn thinking_stream_state_routes_reasoning_and_content() {
        let mut state = ThinkingStreamState::default();

        assert_eq!(state.ingest("\n\n<th"), ThinkingDelta::default());
        assert_eq!(
            state.ingest("ink>chain"),
            ThinkingDelta { reasoning: "chain".to_owned(), content: String::new() }
        );
        assert_eq!(state.ingest("</th"), ThinkingDelta::default());
        assert_eq!(
            state.ingest("ink>\n\nanswer"),
            ThinkingDelta { reasoning: String::new(), content: "\n\nanswer".to_owned() }
        );
        assert_eq!(state.finish(), ThinkingDelta::default());
    }

    #[test]
    fn thinking_stream_state_routes_prompt_prefilled_reasoning_and_content() {
        let prompt = "<|im_start|>assistant\n<think>\n";
        let mut state = ThinkingStreamState::for_prompt(prompt);

        assert_eq!(
            state.ingest("chain</th"),
            ThinkingDelta { reasoning: "chain".to_owned(), content: String::new() }
        );
        assert_eq!(
            state.ingest("ink>\n\nfinal answer"),
            ThinkingDelta { reasoning: String::new(), content: "\n\nfinal answer".to_owned() }
        );
        assert_eq!(state.finish(), ThinkingDelta::default());
    }

    #[test]
    fn thinking_stream_state_treats_closed_prompt_think_as_content() {
        let prompt = "<|im_start|>assistant\n<think>\n\n</think>\n\n";
        let mut state = ThinkingStreamState::for_prompt(prompt);

        assert_eq!(
            state.ingest("final answer"),
            ThinkingDelta { reasoning: String::new(), content: "final answer".to_owned() }
        );
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
    use super::{apply_stop_sequences, resolve_auto_n_ctx, trailing_partial_stop_len};

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

    #[test]
    fn auto_context_falls_back_when_no_vram() {
        // head_dim = 4096/32 = 128 → bytes_per_token = 32·2·8·128·2 = 131072.
        // No VRAM signal → min(n_ctx_train, 8192).
        assert_eq!(resolve_auto_n_ctx(Some(32768), 5_000_000_000, 32, 8, 4096, 32, None), 8192);
        assert_eq!(resolve_auto_n_ctx(Some(4096), 5_000_000_000, 32, 8, 4096, 32, None), 4096);
        // Unknown training context + no VRAM → fallback default.
        assert_eq!(resolve_auto_n_ctx(None, 5_000_000_000, 32, 8, 4096, 32, None), 8192);
    }

    #[test]
    fn auto_context_sizes_to_vram_and_caps_at_training_context() {
        // free 10 GiB − model 5 GiB − buffer 2 GiB = 3 GiB; ÷131072 = 24576.
        let sized = resolve_auto_n_ctx(
            Some(32768),
            5 * 1024 * 1024 * 1024,
            32,
            8,
            4096,
            32,
            Some(10 * 1024 * 1024 * 1024),
        );
        assert_eq!(sized, 24576);

        // Capped at n_ctx_train when VRAM would allow more.
        let capped = resolve_auto_n_ctx(
            Some(8192),
            5 * 1024 * 1024 * 1024,
            32,
            8,
            4096,
            32,
            Some(64 * 1024 * 1024 * 1024),
        );
        assert_eq!(capped, 8192);
    }

    #[test]
    fn auto_context_floors_to_quantum_with_a_minimum() {
        // Budget too small for even one full KV slot → clamped to the 512 minimum.
        let n_ctx =
            resolve_auto_n_ctx(Some(32768), 5_000_000_000, 32, 8, 4096, 32, Some(5_500_000_000));
        assert!(n_ctx >= 512);
        assert_eq!(n_ctx % 512, 0);
    }
}
