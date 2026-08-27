//! Conversation compaction policy that summarizes older turns via the model.
//!
//! [`SummarizingCompactPort`] implements [`slab_agent::CompactPort`]. When the
//! estimated token pressure crosses the model's context window (or when forced
//! by a manual `/compact`), it asks the configured chat model to recap the
//! older portion of the thread into a single `slab_compact` system message and
//! keeps the leading system prompt + a recent trailing window verbatim. If the
//! summarization LLM call fails or yields no text, it transparently falls back
//! to [`slab_agent::SlidingWindowCompactPort`] (pure trailing-window trim).
//!
//! ## Recursion guard
//! The summarization call goes through [`ChatService::create_chat_completion`],
//! which itself never compacts. The port therefore cannot re-enter compaction.

use async_trait::async_trait;
use slab_agent::{
    CompactContext, CompactOutcome, CompactPort, SlidingWindowCompactPort, estimate_message_tokens,
    estimate_tokens,
};
use slab_types::{ConversationMessage, ConversationMessageContent};
use tracing::warn;

use crate::context::ModelState;
use crate::domain::models::{
    ChatCompletionCommand, ChatCompletionOutput, CommonChatParams, UnifiedModel, UnifiedModelKind,
};
use crate::domain::services::chat::ChatService;
use crate::error::AppCoreError;
use crate::infra::db::ModelStore;

/// Default fraction of `context_length` at which auto-compaction fires.
const DEFAULT_THRESHOLD_RATIO: f32 = 0.80;
/// Default fraction of `context_length` retained verbatim after compaction
/// (the recent trailing window that is not summarized).
const DEFAULT_TARGET_RATIO: f32 = 0.60;
/// Fallback fixed threshold (tokens) when a local model's context window is
/// unknown.
const DEFAULT_FALLBACK_THRESHOLD_TOKENS: usize = 12_000;
/// Fallback keep target (tokens) when the context window is unknown —
/// strictly below [`DEFAULT_FALLBACK_THRESHOLD_TOKENS`]. An equal
/// threshold/keep pair re-fires compaction on the very next turn: the kept
/// window lands exactly at the trigger line, looping a compaction marker
/// into every turn.
const DEFAULT_FALLBACK_KEEP_TARGET_TOKENS: usize = 9_000;
/// Assumed context window (tokens) for a cloud model with no recorded
/// window. Modern cloud models are >= 128k, so the local 12k fallback would
/// compact absurdly early (a 1M-window model compacting at 14k tokens).
/// Over-shooting is recoverable — the turn loop force-compacts on a provider
/// context-length error — while premature compaction destroys context.
const CLOUD_FALLBACK_CONTEXT_TOKENS: u32 = 128_000;
/// Memory-pressure fill ratio (0-1, scheduler-reported GPU VRAM) at which
/// auto-compaction fires even below the token threshold. High by design — a
/// transient spike must not re-summarize history; only a sustained squeeze
/// (or a genuinely tight card) should.
const MEMORY_PRESSURE_COMPACT_THRESHOLD: f64 = 0.90;

/// Auto-compaction gate: the token threshold OR an explicit memory-pressure
/// signal (`None` never fabricates pressure).
fn should_compact(
    estimated_tokens: usize,
    threshold_tokens: usize,
    memory_pressure: Option<f64>,
) -> bool {
    estimated_tokens >= threshold_tokens
        || memory_pressure.is_some_and(|pressure| pressure >= MEMORY_PRESSURE_COMPACT_THRESHOLD)
}

const SUMMARY_SYSTEM_INSTRUCTION: &str = "You are a conversation summarizer. Read the \
conversation transcript and write a concise, faithful recap that preserves every decision, fact, \
open question, and in-flight task. Write in the same language as the conversation. Do not add \
new information or opinions. Output only the summary.";

const SUMMARY_USER_PREFIX: &str = "Summarize the following earlier conversation so it can replace \
it as compact context. Keep all technical specifics, file names, commands, and decisions:\n\n";

/// Marks synthetic summary messages so future compactions can detect and reuse
/// a prior recap instead of re-summarizing from scratch.
pub const SUMMARY_MESSAGE_NAME: &str = "slab_compact";

/// Compaction policy that summarizes older turns via the model, with a
/// trailing-window trim fallback.
pub struct SummarizingCompactPort {
    state: ModelState,
    threshold_ratio: f32,
    target_ratio: f32,
    fallback_threshold_tokens: usize,
    fallback: SlidingWindowCompactPort,
}

impl SummarizingCompactPort {
    /// Construct with default ratios (threshold 80%, target 60%).
    pub fn new(state: ModelState) -> Self {
        Self {
            state,
            threshold_ratio: DEFAULT_THRESHOLD_RATIO,
            target_ratio: DEFAULT_TARGET_RATIO,
            fallback_threshold_tokens: DEFAULT_FALLBACK_THRESHOLD_TOKENS,
            fallback: SlidingWindowCompactPort::default(),
        }
    }
}

#[async_trait]
impl CompactPort for SummarizingCompactPort {
    fn policy_name(&self) -> &'static str {
        "summarizing"
    }

    fn threshold_tokens(&self) -> usize {
        self.fallback_threshold_tokens
    }

    fn estimate_tokens(&self, messages: &[ConversationMessage]) -> usize {
        estimate_tokens(messages)
    }

    async fn compact(
        &self,
        messages: &[ConversationMessage],
        ctx: &CompactContext<'_>,
    ) -> Result<CompactOutcome, slab_agent::AgentError> {
        if messages.len() < 2 {
            return Ok(CompactOutcome::Skipped { reason: "not enough messages to compact".into() });
        }

        // One resolve feeds both the threshold gate and the keep target.
        let resolved = resolve_window(&self.state, ctx.model_id).await;
        let (threshold, keep_target) = effective_limits(
            &resolved,
            self.threshold_ratio,
            self.target_ratio,
            self.fallback_threshold_tokens,
        );

        if !ctx.force {
            // Dual gate: the token threshold, OR memory pressure — an
            // injected hint wins, else the policy self-queries the scheduler's
            // cached gauge (no probe on this per-turn path). Cloud sessions
            // never self-query: the gauge measures the host's GPUs, which a
            // cloud model does not occupy (compaction frees no VRAM there).
            let memory_pressure = ctx.memory_pressure_hint.or_else(|| {
                (!resolved.is_cloud)
                    .then(|| self.state.gpu_scheduler().gpu_memory_pressure())
                    .flatten()
            });
            if !should_compact(estimate_tokens(messages), threshold, memory_pressure) {
                return Ok(CompactOutcome::Skipped { reason: "below threshold".into() });
            }
        }

        let system_msg = messages.first().filter(|message| message.role == "system").cloned();
        let system_end = if system_msg.is_some() { 1 } else { 0 };
        let keep_start =
            skip_orphan_tool_results(messages, recent_window_start(messages, keep_target))
                .max(system_end);

        // Nothing older than the kept window to summarize.
        if keep_start <= system_end {
            return Ok(CompactOutcome::Skipped { reason: "nothing to summarize".into() });
        }

        let to_summarize = &messages[system_end..keep_start];
        let transcript = render_transcript(to_summarize);
        if transcript.trim().is_empty() {
            return Ok(CompactOutcome::Skipped { reason: "nothing to summarize".into() });
        }

        let instruction = ctx
            .summary_instructions
            .map(str::to_owned)
            .unwrap_or_else(|| SUMMARY_USER_PREFIX.to_owned());

        // All skip gates have passed — a (potentially slow) summarization LLM
        // call is about to run. Notify the host so it can surface an in-progress
        // "compacting context" indicator. Pure-local fallback paths never reach
        // here, so this fires exactly once per actual summarization.
        if let Some(progress) = ctx.progress.as_ref() {
            progress.on_compacting().await;
        }

        let summary = match self.summarize(ctx.model_id, &instruction, &transcript).await {
            Ok(summary) if !summary.trim().is_empty() => summary,
            Ok(_) => {
                warn!("compaction summarizer returned empty content; falling back to trim");
                return self.fallback.compact(messages, ctx).await;
            }
            Err(error) => {
                warn!(%error, "compaction summarizer failed; falling back to trim");
                return self.fallback.compact(messages, ctx).await;
            }
        };

        let summary_message = ConversationMessage {
            role: "system".into(),
            content: ConversationMessageContent::Text(format!(
                "Summary of earlier conversation:\n{summary}"
            )),
            name: Some(SUMMARY_MESSAGE_NAME.to_owned()),
            tool_call_id: None,
            tool_calls: Vec::new(),
        };

        let mut compacted = Vec::with_capacity(messages.len() - to_summarize.len() + 1);
        if let Some(system) = system_msg {
            compacted.push(system);
        }
        compacted.push(summary_message);
        compacted.extend_from_slice(&messages[keep_start..]);

        if compacted.len() >= messages.len() {
            return Ok(CompactOutcome::Skipped {
                reason: "compaction did not shrink the message set".into(),
            });
        }

        let output_tokens = estimate_tokens(&compacted);
        Ok(CompactOutcome::Replaced {
            replaced_messages: messages.len() - compacted.len(),
            messages: compacted,
            output_tokens,
        })
    }
}

impl SummarizingCompactPort {
    /// Ask the chat model to summarize `transcript`, returning the recap text.
    async fn summarize(
        &self,
        model_id: &str,
        instruction: &str,
        transcript: &str,
    ) -> Result<String, AppCoreError> {
        let command = ChatCompletionCommand {
            id: None,
            model: model_id.to_owned(),
            messages: vec![
                ConversationMessage {
                    role: "system".into(),
                    content: ConversationMessageContent::Text(SUMMARY_SYSTEM_INSTRUCTION.into()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                },
                ConversationMessage {
                    role: "user".into(),
                    content: ConversationMessageContent::Text(format!("{instruction}{transcript}")),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                },
            ],
            tools: Vec::new(),
            agent_trace: None,
            continue_generation: false,
            common: CommonChatParams {
                // No explicit cap: `resolve_sampling` only puts *request-level*
                // caps on the wire, and cloud reasoning models burn reasoning
                // tokens against a small `max_tokens` — a 512 cap comes back
                // as empty content and this compaction silently degrades to
                // the destructive trim fallback. Local models fall back to
                // the built-in default cap.
                max_tokens: None,
                temperature: Some(0.0),
                top_p: None,
                top_k: None,
                min_p: None,
                presence_penalty: None,
                repetition_penalty: None,
                n: 1,
                stream: false,
                stop: Vec::new(),
                stream_options: Default::default(),
            },
            local: Default::default(),
            cloud: Default::default(),
        };

        let output = ChatService::new(self.state.clone()).create_chat_completion(command).await?;
        let ChatCompletionOutput::Json(result) = output else {
            return Err(AppCoreError::Internal(
                "summarize expected a non-streaming completion".into(),
            ));
        };
        let Some(choice) = result.choices.into_iter().next() else {
            return Err(AppCoreError::Internal("summarize completion had no choices".into()));
        };
        Ok(choice.message.rendered_text())
    }
}

/// Run a compaction pass in place, returning the outcome.
///
/// `force` should be `true` for a manual `/compact` and `false` for auto
/// compaction from a turn loop or HTTP path. Each policy applies its own
/// threshold gate internally (unless forced).
pub async fn maybe_compact_messages(
    compact: &dyn CompactPort,
    model_id: &str,
    messages: &mut Vec<ConversationMessage>,
    force: bool,
) -> Result<CompactOutcome, AppCoreError> {
    let ctx = CompactContext {
        model_id,
        summary_instructions: None,
        force,
        progress: None,
        // The host policy self-queries the scheduler when the hint is unset.
        memory_pressure_hint: None,
    };
    let outcome = compact.compact(messages, &ctx).await.map_err(AppCoreError::from)?;
    if let CompactOutcome::Replaced { messages: compacted, .. } = outcome.clone() {
        *messages = compacted;
    }
    Ok(outcome)
}

/// Index of the first message in the trailing window that fits within
/// `target_tokens` (inclusive). Returns `messages.len()` when nothing fits.
fn recent_window_start(messages: &[ConversationMessage], target_tokens: usize) -> usize {
    let mut tokens = 0usize;
    let mut start = messages.len();
    for (index, message) in messages.iter().enumerate().rev() {
        let message_tokens = estimate_message_tokens(message);
        if start != messages.len() && tokens + message_tokens > target_tokens {
            break;
        }
        tokens += message_tokens;
        start = index;
    }
    start
}

/// Advance a keep-window start past consecutive `role == "tool"` messages.
///
/// A boundary landing between an assistant `tool_calls` message and its tool
/// results would keep orphan results (their pairing assistant got summarized
/// away) — OpenAI-compatible providers reject those with a 400 and the turn
/// fails. Advancing folds the orphans into the summarized region instead;
/// intact pairs earlier in the window are untouched.
fn skip_orphan_tool_results(messages: &[ConversationMessage], mut start: usize) -> usize {
    while messages.get(start).is_some_and(|message| message.role == "tool") {
        start += 1;
    }
    start
}

/// Render a slice of messages as a flat `role: text` transcript for summarization.
fn render_transcript(messages: &[ConversationMessage]) -> String {
    messages
        .iter()
        .map(|message| format!("{}: {}", message.role, message.rendered_text()))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Best-effort resolution of a model's context window (tokens) plus whether
/// the model routes to a cloud provider.
///
/// 1. The scheduler's ledger: the engine-resolved `n_ctx` for the resident
///    local model — what `auto` actually sized to (workers + projector
///    accounted). Cloud models never appear here. 2. A point lookup in the
///    model catalog: the recorded `context_window` (curated cloud entries,
///    local manifests) and the model's kind. 3. The configured local llama
///    per-seq context length (fixed values only) — local models only, so a
///    small local setting never leaks into a cloud model's threshold.
async fn resolve_window(state: &ModelState, model_id: &str) -> ResolvedWindow {
    if let Some(resolved) = state.gpu_scheduler().effective_context_budget(model_id).await {
        return ResolvedWindow { window: Some(resolved), is_cloud: false };
    }
    if let Ok(Some(record)) = state.store().get_model(model_id).await {
        let Ok(model) = UnifiedModel::try_from(record) else {
            return ResolvedWindow::local_unknown(state);
        };
        if model.kind == UnifiedModelKind::Cloud {
            return ResolvedWindow { window: model.spec.context_window, is_cloud: true };
        }
        if let Some(context_window) = model.spec.context_window {
            return ResolvedWindow { window: Some(context_window), is_cloud: false };
        }
    }
    ResolvedWindow::local_unknown(state)
}

/// Resolved context window plus model class for compaction limit selection.
struct ResolvedWindow {
    /// Advertised/engine-resolved context window (tokens), when known.
    window: Option<u32>,
    /// Whether the model serves from a cloud provider (never in the GPU
    /// ledger; host GPU state is irrelevant to its sessions).
    is_cloud: bool,
}

impl ResolvedWindow {
    /// Local model with no recorded window: the llama fixed setting is the
    /// last resort (an `auto` setting resolves to `None`).
    fn local_unknown(state: &ModelState) -> Self {
        Self {
            window: state
                .pmid()
                .config()
                .runtime
                .llama
                .context_length
                .and_then(|spec| spec.as_fixed_u32()),
            is_cloud: false,
        }
    }
}

/// `(threshold, keep_target)` for a resolved window.
///
/// The keep target is strictly below the threshold in every branch — an
/// equal pair re-fires compaction on the next turn (the kept window lands at
/// the trigger line). A cloud model with no recorded window assumes the
/// modern cloud floor instead of the local 12k fallback; a genuine overflow
/// is recovered by the turn loop's force-compaction-on-context-error path.
fn effective_limits(
    resolved: &ResolvedWindow,
    threshold_ratio: f32,
    target_ratio: f32,
    fallback_threshold_tokens: usize,
) -> (usize, usize) {
    if let Some(window) = resolved.window.filter(|window| *window > 0) {
        return (
            (window as f32 * threshold_ratio) as usize,
            (window as f32 * target_ratio) as usize,
        );
    }
    if resolved.is_cloud {
        return (
            (CLOUD_FALLBACK_CONTEXT_TOKENS as f32 * threshold_ratio) as usize,
            (CLOUD_FALLBACK_CONTEXT_TOKENS as f32 * target_ratio) as usize,
        );
    }
    (fallback_threshold_tokens, DEFAULT_FALLBACK_KEEP_TARGET_TOKENS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_message(role: &str, text: &str) -> ConversationMessage {
        ConversationMessage {
            role: role.into(),
            content: ConversationMessageContent::Text(text.into()),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }

    #[test]
    fn recent_window_start_keeps_trailing_messages_within_budget() {
        let messages = vec![
            text_message("system", "sys"),           // 1 token
            text_message("user", "hello world"),     // 3 tokens
            text_message("assistant", "hi there"),   // 2 tokens
            text_message("user", "again more text"), // 4 tokens
        ];
        // Budget of 6 tokens keeps the trailing two messages (2 + 4 = 6).
        let start = recent_window_start(&messages, 6);
        assert_eq!(start, 2, "expected keep_start = 2 (keeps indices 2..4)");
    }

    #[test]
    fn recent_window_start_returns_len_when_nothing_fits() {
        let messages = vec![text_message("user", &"x".repeat(200))];
        assert_eq!(recent_window_start(&messages, 1), 0);
    }

    #[test]
    fn render_transcript_joins_role_and_text() {
        let messages = vec![text_message("user", "hi"), text_message("assistant", "hello")];
        let transcript = render_transcript(&messages);
        assert!(transcript.contains("user: hi"));
        assert!(transcript.contains("assistant: hello"));
    }

    /// S1-b integration: the compaction threshold's context resolution is
    /// scheduler-first — the ledger's engine-resolved `n_ctx` (what `auto`
    /// sized to) beats the catalog and the fixed config fallback. Without
    /// this an `auto` context compacts at the fixed 12k fallback threshold.
    #[tokio::test]
    async fn resolve_window_prefers_ledger_over_catalog_and_config() {
        use crate::test_support::{TestAppCore, ready_local_llama_command};

        let app = TestAppCore::new().await;

        // Catalog tier: a local model with a recorded context window.
        let model_path = app.model_cache_dir.join("compact-ctx.gguf");
        std::fs::write(&model_path, b"gguf").expect("write model fixture");
        let mut command = ready_local_llama_command("catalog-model", &model_path);
        command.spec.context_window = Some(2048);
        app.model.create_model(command).await.expect("create catalog model");

        // No ledger entry yet: the catalog tier answers.
        let resolved = resolve_window(&app.model_state, "catalog-model").await;
        assert_eq!((resolved.window, resolved.is_cloud), (Some(2048), false));

        // A resident ledger entry with the engine-resolved n_ctx wins over
        // every fallback tier.
        app.model_state
            .gpu_scheduler()
            .ledger()
            .note_model_loaded(
                None,
                slab_gpu_memory_scheduler::LedgerEntry {
                    backend: slab_types::RuntimeBackendId::GgmlLlama,
                    model_id: Some("catalog-model".to_owned()),
                    model_path: model_path.to_string_lossy().into_owned(),
                    num_workers: 2,
                    resolved_context_length: Some(4096),
                    mmproj_resident: false,
                    weights_bytes: None,
                    mmproj_bytes: None,
                    measured_delta_bytes: None,
                    recorded_at: chrono::Utc::now(),
                },
            )
            .await;
        let resolved = resolve_window(&app.model_state, "catalog-model").await;
        assert_eq!(
            (resolved.window, resolved.is_cloud),
            (Some(4096), false),
            "ledger beats the catalog's 2048"
        );

        // Clearing the entry falls the resolver back to the catalog tier.
        app.model_state
            .gpu_scheduler()
            .ledger()
            .note_model_unloaded(slab_types::RuntimeBackendId::GgmlLlama)
            .await;
        let resolved = resolve_window(&app.model_state, "catalog-model").await;
        assert_eq!(
            (resolved.window, resolved.is_cloud),
            (Some(2048), false),
            "catalog tier after the ledger entry clears"
        );
    }

    /// The local llama runtime setting is the last resort for LOCAL models
    /// only — a small fixed local value must never leak into a cloud model's
    /// threshold (it used to compact cloud sessions after a few turns).
    #[tokio::test]
    async fn resolve_window_skips_llama_tier_for_cloud_models() {
        use crate::test_support::{TestAppCore, cloud_chat_model_command};
        use slab_config::{UpdateSettingCommand, UpdateSettingOperation};

        let app = TestAppCore::new().await;
        app.model
            .create_model(cloud_chat_model_command("cloud-ctx", "openai-main"))
            .await
            .expect("create cloud model");

        // A fixed local llama context that would otherwise be the last-resort
        // tier for an unknown-window model.
        app.pmid
            .update_setting(
                "runtime.ggml.backends.llama.context_length",
                UpdateSettingCommand {
                    op: UpdateSettingOperation::Set,
                    value: Some(serde_json::json!(4096).into()),
                },
            )
            .await
            .expect("set llama context length");

        let cloud = resolve_window(&app.model_state, "cloud-ctx").await;
        assert_eq!((cloud.window, cloud.is_cloud), (None, true));

        // A local model absent from the catalog still gets the fixed setting.
        let local = resolve_window(&app.model_state, "ghost-local").await;
        assert_eq!((local.window, local.is_cloud), (Some(4096), false));
    }

    /// Effective limits: known window scales by ratio; unknown cloud assumes
    /// the modern cloud floor (NOT the 12k local fallback); unknown local
    /// keeps a keep target strictly below the threshold so a fallback
    /// compaction quiesces instead of re-firing every turn.
    #[test]
    fn effective_limits_branches_by_window_and_class() {
        let ratios = (DEFAULT_THRESHOLD_RATIO, DEFAULT_TARGET_RATIO);
        let fallback_threshold = DEFAULT_FALLBACK_THRESHOLD_TOKENS;

        let windowed = ResolvedWindow { window: Some(1_000_000), is_cloud: false };
        assert_eq!(
            effective_limits(&windowed, ratios.0, ratios.1, fallback_threshold),
            (800_000, 600_000)
        );

        let cloud_unknown = ResolvedWindow { window: None, is_cloud: true };
        let (threshold, keep) =
            effective_limits(&cloud_unknown, ratios.0, ratios.1, fallback_threshold);
        assert_eq!(threshold, (CLOUD_FALLBACK_CONTEXT_TOKENS as f32 * ratios.0) as usize);
        assert_eq!(keep, (CLOUD_FALLBACK_CONTEXT_TOKENS as f32 * ratios.1) as usize);
        assert!(threshold > fallback_threshold, "cloud default must dwarf the local fallback");

        let local_unknown = ResolvedWindow { window: None, is_cloud: false };
        let (threshold, keep) =
            effective_limits(&local_unknown, ratios.0, ratios.1, fallback_threshold);
        assert_eq!(threshold, fallback_threshold);
        assert_eq!(keep, DEFAULT_FALLBACK_KEEP_TARGET_TOKENS);
        assert!(keep < threshold, "fallback keep target must sit below the trigger");
    }

    /// The bug report: a 1M-window cloud model compacted at ~14k tokens. With
    /// the cloud default in place a ~60k-token history on an unknown-window
    /// cloud model stays far below the threshold.
    #[tokio::test]
    async fn cloud_unknown_window_does_not_compact_at_local_fallback() {
        use crate::test_support::{TestAppCore, cloud_chat_model_command};

        let app = TestAppCore::new().await;
        app.model
            .create_model(cloud_chat_model_command("cloud-wide", "openai-main"))
            .await
            .expect("create cloud model");

        let port = SummarizingCompactPort::new(app.model_state.clone());
        let mut messages = vec![text_message("system", "sys")];
        for _ in 0..8 {
            messages.push(text_message("user", &"x".repeat(28_000))); // ~7k tokens each
        }
        // ~56k tokens: above the old 12k fallback, far below 0.8 * 128k.

        let outcome = port
            .compact(
                &messages,
                &CompactContext {
                    model_id: "cloud-wide",
                    summary_instructions: None,
                    force: false,
                    progress: None,
                    memory_pressure_hint: None,
                },
            )
            .await
            .expect("compact check");
        assert!(
            matches!(&outcome, CompactOutcome::Skipped { reason } if reason == "below threshold"),
            "cloud session must not compact below the cloud threshold: {outcome:?}"
        );
    }

    /// Fallback compaction quiesces: after compacting an unknown-window local
    /// history, the result sits below the 12k trigger, so the next turn's
    /// gate skips (the old equal threshold/keep pair re-fired every turn).
    #[tokio::test]
    async fn fallback_compact_quiesces_second_pass() {
        use crate::test_support::TestAppCore;

        let app = TestAppCore::new().await;
        let port = SummarizingCompactPort::new(app.model_state.clone());
        let ctx = |model_id: &'static str| CompactContext {
            model_id,
            summary_instructions: None,
            force: false,
            progress: None,
            memory_pressure_hint: None,
        };

        // Unknown-window local model (no catalog row, auto llama setting) and
        // a ~28k-token history: above the 12k trigger.
        let messages: Vec<ConversationMessage> = std::iter::once(text_message("system", "sys"))
            .chain((0..4).map(|_| text_message("user", &"x".repeat(28_000))))
            .collect();

        let first = port.compact(&messages, &ctx("ghost-local")).await.expect("first compact");
        let CompactOutcome::Replaced { messages: compacted, .. } = first else {
            panic!("first pass must compact above the fallback threshold: {first:?}");
        };

        let second = port.compact(&compacted, &ctx("ghost-local")).await.expect("second compact");
        assert!(
            matches!(&second, CompactOutcome::Skipped { reason } if reason == "below threshold"),
            "compacted history must sit below the re-trigger threshold: {second:?}"
        );
    }

    /// Host GPU pressure is a LOCAL signal: a cloud session must not compact
    /// because the host's card is nearly full (compacting frees no VRAM
    /// there), while a resident local model under the same gauge compacts.
    #[tokio::test]
    async fn cloud_sessions_ignore_host_gpu_pressure() {
        use crate::test_support::{FixedGpuProbe, TestAppCore, cloud_chat_model_command};
        use std::sync::Arc;

        let probe =
            Arc::new(FixedGpuProbe { total_memory_bytes: 10_000, used_memory_bytes: 9_600 });
        let app = TestAppCore::new_with_gpu_probe(probe).await;
        app.gpu_scheduler.refresh_now().await; // cached gauge at 96% fill
        assert!(
            app.model_state.gpu_scheduler().gpu_memory_pressure().is_some_and(|p| p >= 0.90),
            "gauge fixture must read as high pressure"
        );

        app.model
            .create_model(cloud_chat_model_command("cloud-pressured", "openai-main"))
            .await
            .expect("create cloud model");
        let mut cloud_history = vec![text_message("system", "sys")];
        for _ in 0..8 {
            cloud_history.push(text_message("user", &"x".repeat(28_000))); // ~56k total
        }

        let port = SummarizingCompactPort::new(app.model_state.clone());
        let outcome = port
            .compact(
                &cloud_history,
                &CompactContext {
                    model_id: "cloud-pressured",
                    summary_instructions: None,
                    force: false,
                    progress: None,
                    memory_pressure_hint: None,
                },
            )
            .await
            .expect("cloud compact check");
        assert!(
            matches!(&outcome, CompactOutcome::Skipped { reason } if reason == "below threshold"),
            "host GPU pressure must not compact a cloud session: {outcome:?}"
        );

        // Contrast: a resident local model under the same gauge compacts well
        // below its token threshold.
        app.model_state
            .gpu_scheduler()
            .ledger()
            .note_model_loaded(
                None,
                slab_gpu_memory_scheduler::LedgerEntry {
                    backend: slab_types::RuntimeBackendId::GgmlLlama,
                    model_id: Some("local-pressured".to_owned()),
                    model_path: "pressured.gguf".to_owned(),
                    num_workers: 1,
                    // Window 80k: token gate (64k) stays closed on the ~56k
                    // history while the keep window (48k) leaves older turns
                    // to summarize — the compaction is purely pressure-driven.
                    resolved_context_length: Some(80_000),
                    mmproj_resident: false,
                    weights_bytes: None,
                    mmproj_bytes: None,
                    measured_delta_bytes: None,
                    recorded_at: chrono::Utc::now(),
                },
            )
            .await;
        let outcome = port
            .compact(
                &cloud_history,
                &CompactContext {
                    model_id: "local-pressured",
                    summary_instructions: None,
                    force: false,
                    progress: None,
                    memory_pressure_hint: None,
                },
            )
            .await
            .expect("local compact check");
        assert!(
            matches!(outcome, CompactOutcome::Replaced { .. }),
            "resident local model under pressure must compact: {outcome:?}"
        );
    }

    #[test]
    fn skip_orphan_tool_results_advances_past_leading_tools() {
        let messages = vec![
            text_message("system", "sys"),
            text_message("assistant", "call it"),
            text_message("tool", "result-a"),
            text_message("tool", "result-b"),
            text_message("user", "next"),
        ];
        assert_eq!(skip_orphan_tool_results(&messages, 2), 4, "both orphan results skipped");
        assert_eq!(skip_orphan_tool_results(&messages, 3), 4, "single orphan skipped");
        assert_eq!(skip_orphan_tool_results(&messages, 4), 4, "non-tool start untouched");
        assert_eq!(skip_orphan_tool_results(&messages, 0), 0, "system message untouched");
    }

    #[test]
    fn skip_orphan_tool_results_runs_off_the_end() {
        let messages = vec![
            text_message("system", "sys"),
            text_message("tool", "result"),
            text_message("tool", "tail"),
        ];
        assert_eq!(skip_orphan_tool_results(&messages, 1), 3);
        assert_eq!(skip_orphan_tool_results(&messages, 3), 3, "start at len stays at len");
    }

    /// A keep boundary landing between an assistant `tool_calls` message and
    /// its tool results must not keep orphan results — the summarize path
    /// folds them into the summarized region instead. Orphaned tool results
    /// get cloud APIs a 400 and fail the turn after every compaction.
    #[tokio::test]
    async fn summarizing_compact_never_keeps_leading_orphan_tool_results() {
        use crate::domain::ports::RuntimeTextGenerationResponse;
        use crate::test_support::{TestAppCore, ready_local_llama_command};

        let app = TestAppCore::new().await;
        // Scripted summarize success so the summarizing path (not the trim
        // fallback, which already strips orphans) runs end-to-end.
        app.runtime.set_scripted_chat(RuntimeTextGenerationResponse {
            text: "recap of earlier turns".to_owned(),
            ..Default::default()
        });
        // Window 2048 -> threshold 1638, keep_target 1228. History est
        // ~6.7k tokens fires the gate; the raw trailing window (done + two
        // 100-token tool results = 201 <= 1228 < +2000-token assistant call)
        // lands exactly ON the first tool result.
        app.model_state
            .gpu_scheduler()
            .ledger()
            .note_model_loaded(
                None,
                slab_gpu_memory_scheduler::LedgerEntry {
                    backend: slab_types::RuntimeBackendId::GgmlLlama,
                    model_id: Some("boundary-model".to_owned()),
                    model_path: "boundary.gguf".to_owned(),
                    num_workers: 1,
                    resolved_context_length: Some(2048),
                    mmproj_resident: false,
                    weights_bytes: None,
                    mmproj_bytes: None,
                    measured_delta_bytes: None,
                    recorded_at: chrono::Utc::now(),
                },
            )
            .await;
        let model_path = app.model_cache_dir.join("boundary.gguf");
        std::fs::write(&model_path, b"gguf").expect("write model fixture");
        app.model
            .create_model(ready_local_llama_command("boundary-model", &model_path))
            .await
            .expect("create boundary model");

        let port = SummarizingCompactPort::new(app.model_state.clone());
        let mut messages = vec![text_message("system", "sys")];
        for _ in 0..3 {
            messages.push(text_message("user", &"x".repeat(6_000))); // ~1500 tokens each
        }
        messages.push(text_message("assistant", &"c".repeat(8_000))); // ~2000 tokens
        messages.push(text_message("tool", &"r".repeat(400))); // ~100 tokens
        messages.push(text_message("tool", &"r".repeat(400))); // ~100 tokens
        messages.push(text_message("assistant", "done"));

        let outcome = port
            .compact(
                &messages,
                &CompactContext {
                    model_id: "boundary-model",
                    summary_instructions: None,
                    force: false,
                    progress: None,
                    memory_pressure_hint: None,
                },
            )
            .await
            .expect("compact");
        let CompactOutcome::Replaced { messages: compacted, .. } = outcome else {
            panic!("history above threshold must compact: {outcome:?}");
        };
        // [system, slab_compact summary, kept tail] — the kept tail starts
        // with the final assistant turn, never an orphaned tool result.
        assert_eq!(compacted.len(), 3, "kept tail = final assistant turn only: {compacted:#?}");
        assert!(compacted[1].name.as_deref() == Some(SUMMARY_MESSAGE_NAME));
        assert_eq!(compacted[2].role, "assistant");
    }

    #[test]
    fn should_compact_fires_on_either_gate() {
        assert!(should_compact(100, 100, None), "token gate at threshold");
        assert!(!should_compact(99, 100, None), "below threshold without pressure");
        assert!(!should_compact(0, 100, Some(0.5)), "pressure below the trigger");
        assert!(should_compact(0, 100, Some(0.90)), "pressure at the trigger");
        assert!(should_compact(0, 100, Some(0.95)), "pressure above the trigger");
        assert!(!should_compact(0, 100, None), "no signal never fabricates pressure");
    }

    /// Dual gate end-to-end: memory pressure compacts below the token
    /// threshold. The ledger's large n_ctx keeps the token gate closed; the
    /// unavailable gateway forces the summarizer onto its sliding-window
    /// fallback, which still compacts because the pressure gate opened.
    #[tokio::test]
    async fn memory_pressure_hint_compacts_below_token_threshold() {
        use crate::test_support::TestAppCore;

        let app = TestAppCore::new().await;
        app.model_state
            .gpu_scheduler()
            .ledger()
            .note_model_loaded(
                None,
                slab_gpu_memory_scheduler::LedgerEntry {
                    backend: slab_types::RuntimeBackendId::GgmlLlama,
                    model_id: Some("pressure-model".to_owned()),
                    model_path: "pressure.gguf".to_owned(),
                    num_workers: 1,
                    resolved_context_length: Some(65_536),
                    mmproj_resident: false,
                    weights_bytes: None,
                    mmproj_bytes: None,
                    measured_delta_bytes: None,
                    recorded_at: chrono::Utc::now(),
                },
            )
            .await;

        let port = SummarizingCompactPort::new(app.model_state.clone());
        let mut messages = vec![text_message("system", "sys")];
        for _ in 0..6 {
            messages.push(text_message("user", &"x".repeat(28_000)));
        }
        // ~42k tokens: below the 80% token threshold (~52k), above the 60%
        // keep window (~39k) so there is history to compact, and above the
        // fallback's fixed 16k.

        let below = port
            .compact(
                &messages,
                &CompactContext {
                    model_id: "pressure-model",
                    summary_instructions: None,
                    force: false,
                    progress: None,
                    memory_pressure_hint: None,
                },
            )
            .await
            .expect("compact without pressure");
        assert!(
            matches!(&below, CompactOutcome::Skipped { reason } if reason == "below threshold"),
            "token gate alone must not fire: {below:?}"
        );

        let pressured = port
            .compact(
                &messages,
                &CompactContext {
                    model_id: "pressure-model",
                    summary_instructions: None,
                    force: false,
                    progress: None,
                    memory_pressure_hint: Some(0.95),
                },
            )
            .await
            .expect("compact under pressure");
        assert!(
            matches!(pressured, CompactOutcome::Replaced { .. }),
            "pressure gate compacts below the token threshold: {pressured:?}"
        );
    }
}
