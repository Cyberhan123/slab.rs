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

use std::collections::HashMap;

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
/// Exponential-moving-average weight of a fresh estimate-vs-actual sample in
/// the estimator calibration. 0.2 converges in ~5 samples without chasing a
/// single outlier.
const CALIBRATION_EMA_ALPHA: f64 = 0.2;
/// Calibration ratio clamp — the chars/4 heuristic under-estimates CJK-heavy
/// content by 2-3x, so 4x headroom covers the worst realistic drift while a
/// buggy provider report cannot collapse estimates to zero.
const CALIBRATION_RATIO_MIN: f64 = 0.5;
const CALIBRATION_RATIO_MAX: f64 = 4.0;

/// Auto-compaction gate: the token threshold OR an explicit memory-pressure
/// signal (`None` never fabricates pressure).
fn should_compact(
    estimated_tokens: usize,
    threshold_tokens: usize,
    memory_pressure: Option<f64>,
) -> bool {
    estimated_tokens >= threshold_tokens || memory_pressure_compact(memory_pressure)
}

/// The memory-pressure leg of the compaction gate on its own.
fn memory_pressure_compact(memory_pressure: Option<f64>) -> bool {
    memory_pressure.is_some_and(|pressure| pressure >= MEMORY_PRESSURE_COMPACT_THRESHOLD)
}

/// Fraction of the context window at which the deterministic micro tier (old
/// tool results -> structured stubs) starts firing — below the LLM-summarize
/// threshold so the cheap clearing happens first and the expensive tier fires
/// later (or never).
const MICRO_THRESHOLD_RATIO: f32 = 0.55;
/// Most-recent assistant tool-call batches whose results stay verbatim.
const KEEP_TOOL_BATCHES: usize = 5;
/// Tool results below this size are not worth stubbing.
const STUB_MIN_BYTES: usize = 512;
/// Head-of-content excerpt kept in a stub.
const STUB_EXCERPT_CHARS: usize = 200;
/// Max file references kept in a stub (grep/glob results).
const STUB_MAX_REFS: usize = 20;
/// Marker identifying an already-stubbed tool result (idempotence guard).
/// Detected by parsing — serde_json does not preserve key order.
const STUB_MARKER_FIELD: &str = "slab_stub";

/// Whether a tool result text is already a micro-compaction stub.
fn is_stub(text: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|value| value.get(STUB_MARKER_FIELD).and_then(|value| value.as_bool()))
        .unwrap_or(false)
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
    /// EMA of actual/estimated prompt-token ratio, fed by
    /// [`CompactPort::note_usage`] and applied in `estimate_tokens`. The port
    /// is constructed once per process and shared by every thread, so the
    /// calibration is process-global by construction (bounded by the clamp).
    calibration: std::sync::Mutex<f64>,
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
            calibration: std::sync::Mutex::new(1.0),
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
        let base = estimate_tokens(messages);
        let ratio = self.calibration.lock().map(|value| *value).unwrap_or(1.0);
        ((base as f64) * ratio).ceil() as usize
    }

    fn note_usage(&self, estimated_tokens: usize, actual_prompt_tokens: usize) {
        if estimated_tokens == 0 || actual_prompt_tokens == 0 {
            return;
        }
        let ratio = actual_prompt_tokens as f64 / estimated_tokens as f64;
        if !ratio.is_finite() {
            return;
        }
        if let Ok(mut current) = self.calibration.lock() {
            *current = (*current * (1.0 - CALIBRATION_EMA_ALPHA) + ratio * CALIBRATION_EMA_ALPHA)
                .clamp(CALIBRATION_RATIO_MIN, CALIBRATION_RATIO_MAX);
        }
    }

    async fn compact(
        &self,
        messages: &[ConversationMessage],
        ctx: &CompactContext<'_>,
    ) -> Result<CompactOutcome, slab_agent::AgentError> {
        if messages.len() < 2 {
            return Ok(CompactOutcome::Skipped { reason: "not enough messages to compact".into() });
        }

        // One resolve feeds the tier gates and the keep target.
        let resolved = resolve_window(&self.state, ctx.model_id).await;
        let (micro_threshold, threshold, keep_target) = effective_limits(
            &resolved,
            self.threshold_ratio,
            self.target_ratio,
            self.fallback_threshold_tokens,
        );

        // Dual gate: the token threshold, OR memory pressure — an injected
        // hint wins, else the policy self-queries the scheduler's cached
        // gauge (no probe on this per-turn path). Cloud sessions never
        // self-query: the gauge measures the host's GPUs, which a cloud model
        // does not occupy (compaction frees no VRAM there). The entry gate
        // sits at the MICRO threshold — below it nothing fires at all.
        let memory_pressure = ctx.memory_pressure_hint.or_else(|| {
            (!resolved.is_cloud).then(|| self.state.gpu_scheduler().gpu_memory_pressure()).flatten()
        });
        let pressured = memory_pressure_compact(memory_pressure);
        if !ctx.force && !pressured && self.estimate_tokens(messages) < micro_threshold {
            return Ok(CompactOutcome::Skipped { reason: "below threshold".into() });
        }

        // Tier 1 (deterministic): stub old tool results beyond the most
        // recent tool batches — zero LLM cost, conversation text preserved
        // verbatim, pairing untouched (only message CONTENT shrinks, so no
        // orphan tool results can appear).
        let (micro, stubbed) = micro_compact(messages);
        let micro_estimate = self.estimate_tokens(&micro);

        // Tier 2 (LLM summarize): only when the deterministic pass is not
        // enough — still above the macro threshold or memory pressure demands
        // actually freeing context (the classic dual gate, now on the
        // post-micro estimate). The summarize transcript then runs over the
        // stubbed set, which is cheaper to recap.
        let escalate = ctx.force || should_compact(micro_estimate, threshold, memory_pressure);
        if !escalate {
            if stubbed > 0 {
                return Ok(CompactOutcome::Replaced {
                    messages: micro,
                    replaced_messages: 0,
                    output_tokens: micro_estimate,
                });
            }
            return Ok(CompactOutcome::Skipped { reason: "nothing to micro-compact".into() });
        }

        let system_msg = micro.first().filter(|message| message.role == "system").cloned();
        let system_end = if system_msg.is_some() { 1 } else { 0 };
        let keep_start = skip_orphan_tool_results(&micro, recent_window_start(&micro, keep_target))
            .max(system_end);

        // Nothing older than the kept window to summarize — the deterministic
        // pass may still have shrunk the set.
        if keep_start <= system_end {
            if stubbed > 0 {
                return Ok(CompactOutcome::Replaced {
                    messages: micro,
                    replaced_messages: 0,
                    output_tokens: micro_estimate,
                });
            }
            return Ok(CompactOutcome::Skipped { reason: "nothing to summarize".into() });
        }

        let to_summarize = &micro[system_end..keep_start];
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
                return self.fallback.compact(&micro, ctx).await;
            }
            Err(error) => {
                warn!(%error, "compaction summarizer failed; falling back to trim");
                return self.fallback.compact(&micro, ctx).await;
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

        let mut compacted = Vec::with_capacity(micro.len() - to_summarize.len() + 1);
        if let Some(system) = system_msg {
            compacted.push(system);
        }
        compacted.push(summary_message);
        compacted.extend_from_slice(&micro[keep_start..]);

        if compacted.len() >= micro.len() {
            return Ok(CompactOutcome::Skipped {
                reason: "compaction did not shrink the message set".into(),
            });
        }

        let output_tokens = self.estimate_tokens(&compacted);
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

/// Deterministic micro-compaction tier: replace the CONTENT of old tool
/// messages (beyond the most recent [`KEEP_TOOL_BATCHES`] assistant
/// tool-call batches) with a structured stub. Every message, its role and
/// its `tool_call_id` survive, so pairing is untouched (no orphan tool
/// results can appear) and conversation text is preserved verbatim — this
/// tier never rewrites user/assistant messages.
///
/// Returns the new message set and how many tool results were stubbed.
fn micro_compact(messages: &[ConversationMessage]) -> (Vec<ConversationMessage>, usize) {
    // Tool messages carry only `tool_call_id`; resolve tool names from the
    // assistant `tool_calls` that produced them.
    let mut tool_names: HashMap<&str, &str> = HashMap::new();
    for message in messages {
        for call in &message.tool_calls {
            if let Some(id) = call.id.as_deref() {
                tool_names.insert(id, call.function.name.as_str());
            }
        }
    }

    // Batch = one assistant message carrying tool_calls plus the tool results
    // that follow it. Everything before the (len - KEEP_TOOL_BATCHES)-th
    // batch start is stubbable.
    let batch_starts: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.tool_calls.is_empty())
        .map(|(i, _)| i)
        .collect();
    let protected_start = batch_starts
        .len()
        .checked_sub(KEEP_TOOL_BATCHES)
        .and_then(|index| batch_starts.get(index))
        .copied()
        .unwrap_or(usize::MAX);

    let mut stubbed = 0usize;
    let mut compacted = Vec::with_capacity(messages.len());
    for (index, message) in messages.iter().enumerate() {
        // Content only — `Message::rendered_text` would append the
        // `tool_call_id:` line and break the JSON-based stub helpers.
        let text = message.content.rendered_text();
        if message.role == "tool"
            && index < protected_start
            && text.len() > STUB_MIN_BYTES
            && !is_stub(&text)
        {
            compacted.push(stub_tool_message(message, &text, &tool_names));
            stubbed += 1;
        } else {
            compacted.push(message.clone());
        }
    }
    (compacted, stubbed)
}

/// Build the stub replacement for one old tool result: tool name, original
/// byte size, a head excerpt, file references (for grep/glob JSON results)
/// and the spill artifact reference when the result carried one.
fn stub_tool_message(
    message: &ConversationMessage,
    text: &str,
    tool_names: &HashMap<&str, &str>,
) -> ConversationMessage {
    let tool = message
        .tool_call_id
        .as_deref()
        .and_then(|id| tool_names.get(id).copied())
        .unwrap_or("unknown");
    let mut stub = serde_json::json!({
        "slab_stub": true,
        "tool": tool,
        "original_bytes": text.len(),
        "excerpt": excerpt_chars(text, STUB_EXCERPT_CHARS),
    });
    let refs = extract_file_refs(text, STUB_MAX_REFS);
    if !refs.is_empty() {
        stub["refs"] = serde_json::json!(refs);
    }
    if let Some(artifact) = extract_artifact_ref(text) {
        stub["artifact"] = serde_json::json!(artifact);
    }
    ConversationMessage {
        role: message.role.clone(),
        content: ConversationMessageContent::Text(stub.to_string()),
        name: message.name.clone(),
        tool_call_id: message.tool_call_id.clone(),
        tool_calls: Vec::new(),
    }
}

/// First `max_chars` characters of the content, marked with an ellipsis when
/// clipped.
fn excerpt_chars(text: &str, max_chars: usize) -> String {
    let mut excerpt: String = text.chars().take(max_chars).collect();
    if text.chars().count() > max_chars {
        excerpt.push('…');
    }
    excerpt
}

/// Pull `file[:line]` references out of a grep/file_glob JSON result so the
/// stub keeps the pointers the model actually needs.
fn extract_file_refs(text: &str, max_refs: usize) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let Some(entries) = value.get("matches").and_then(|matches| matches.as_array()) else {
        return Vec::new();
    };
    let mut refs = Vec::new();
    for entry in entries {
        if refs.len() >= max_refs {
            break;
        }
        if let Some(file) = entry.get("file").and_then(|file| file.as_str()) {
            let line = entry.get("line").and_then(|line| line.as_u64());
            refs.push(match line {
                Some(line) => format!("{file}:{line}"),
                None => file.to_owned(),
            });
        } else if let Some(path) = entry.get("path").and_then(|path| path.as_str()) {
            refs.push(path.to_owned());
        }
    }
    refs
}

/// Forward the spill artifact reference (if any) from the original result so
/// the stub still points at the full output on disk.
fn extract_artifact_ref(text: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(text).ok()?;
    for key in
        ["stdout_artifact", "stderr_artifact", "full_results_artifact", "full_content_artifact"]
    {
        if let Some(reference) = value.get(key).and_then(|value| value.as_str()) {
            return Some(reference.to_owned());
        }
    }
    None
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

/// `(micro_threshold, macro_threshold, keep_target)` for a resolved window.
///
/// The micro threshold is where the deterministic tool-result-stubbing tier
/// starts; the macro threshold is where the LLM summarize tier fires. The
/// keep target is strictly below the macro threshold in every branch — an
/// equal pair re-fires compaction on the next turn (the kept window lands at
/// the trigger line). A cloud model with no recorded window assumes the
/// modern cloud floor instead of the local 12k fallback; a genuine overflow
/// is recovered by the turn loop's force-compaction-on-context-error path.
fn effective_limits(
    resolved: &ResolvedWindow,
    threshold_ratio: f32,
    target_ratio: f32,
    fallback_threshold_tokens: usize,
) -> (usize, usize, usize) {
    if let Some(window) = resolved.window.filter(|window| *window > 0) {
        let window = window as f32;
        return (
            (window * MICRO_THRESHOLD_RATIO) as usize,
            (window * threshold_ratio) as usize,
            (window * target_ratio) as usize,
        );
    }
    if resolved.is_cloud {
        let window = CLOUD_FALLBACK_CONTEXT_TOKENS as f32;
        return (
            (window * MICRO_THRESHOLD_RATIO) as usize,
            (window * threshold_ratio) as usize,
            (window * target_ratio) as usize,
        );
    }
    (
        (fallback_threshold_tokens as f32 * MICRO_THRESHOLD_RATIO) as usize,
        fallback_threshold_tokens,
        DEFAULT_FALLBACK_KEEP_TARGET_TOKENS,
    )
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
            (550_000, 800_000, 600_000)
        );

        let cloud_unknown = ResolvedWindow { window: None, is_cloud: true };
        let (micro, threshold, keep) =
            effective_limits(&cloud_unknown, ratios.0, ratios.1, fallback_threshold);
        assert_eq!(micro, (CLOUD_FALLBACK_CONTEXT_TOKENS as f32 * MICRO_THRESHOLD_RATIO) as usize);
        assert_eq!(threshold, (CLOUD_FALLBACK_CONTEXT_TOKENS as f32 * ratios.0) as usize);
        assert_eq!(keep, (CLOUD_FALLBACK_CONTEXT_TOKENS as f32 * ratios.1) as usize);
        assert!(threshold > fallback_threshold, "cloud default must dwarf the local fallback");
        assert!(micro < threshold, "micro tier must fire before the summarize tier");

        let local_unknown = ResolvedWindow { window: None, is_cloud: false };
        let (micro, threshold, keep) =
            effective_limits(&local_unknown, ratios.0, ratios.1, fallback_threshold);
        assert_eq!(threshold, fallback_threshold);
        assert_eq!(keep, DEFAULT_FALLBACK_KEEP_TARGET_TOKENS);
        assert!(keep < threshold, "fallback keep target must sit below the trigger");
        assert!(micro < keep, "micro tier must fire before the fallback summarize tier");
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
        // Quiescence = the second pass must SKIP. With the micro tier the
        // text-only trimmed window can sit between the micro and macro
        // thresholds, where the deterministic pass finds nothing to stub —
        // still a Skipped, so no compaction loop.
        assert!(
            matches!(&second, CompactOutcome::Skipped { .. }),
            "compacted history must not re-trigger compaction: {second:?}"
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
            matches!(&below, CompactOutcome::Skipped { .. }),
            "token gate alone must not fire (below the macro threshold the text-only history has nothing to micro-compact either): {below:?}"
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

    /// Estimator calibration: repeated note_usage samples converge the EMA
    /// toward the actual ratio and lift `estimate_tokens` accordingly.
    #[tokio::test]
    async fn calibration_ema_converges_toward_actual_ratio() {
        use crate::test_support::TestAppCore;

        let app = TestAppCore::new().await;
        let port = SummarizingCompactPort::new(app.model_state.clone());
        let messages = vec![text_message("user", &"x".repeat(4_000))]; // ~1000 estimated

        let before = port.estimate_tokens(&messages);
        // Actual is 2.5x the estimate (CJK-like under-estimation).
        for _ in 0..5 {
            port.note_usage(before, (before as f64 * 2.5) as usize);
        }
        let after = port.estimate_tokens(&messages);
        assert!(
            after > before * 2,
            "calibration must lift estimates toward the actual ratio: {before} -> {after}"
        );
        assert!(after < before * 4, "must stay under the clamp: {before} -> {after}");
    }

    #[tokio::test]
    async fn calibration_is_clamped() {
        use crate::test_support::TestAppCore;

        let app = TestAppCore::new().await;
        let port = SummarizingCompactPort::new(app.model_state.clone());

        // A pathological provider report cannot push the ratio past the max.
        for _ in 0..10 {
            port.note_usage(10, 100_000);
        }
        let messages = vec![text_message("user", &"y".repeat(400))];
        assert_eq!(port.estimate_tokens(&messages), 100 * CALIBRATION_RATIO_MAX as usize);
    }

    #[tokio::test]
    async fn calibration_ignores_degenerate_samples() {
        use crate::test_support::TestAppCore;

        let app = TestAppCore::new().await;
        let port = SummarizingCompactPort::new(app.model_state.clone());

        port.note_usage(0, 500); // zero estimate
        port.note_usage(500, 0); // zero actual

        let messages = vec![text_message("user", &"z".repeat(400))];
        assert_eq!(port.estimate_tokens(&messages), 100, "ratio must stay at 1.0");
    }

    // ── Deterministic micro tier (context-budget system) ─────────────────────

    use slab_types::{ConversationToolCall, ConversationToolFunction};

    fn tool_call_message(index: usize) -> ConversationMessage {
        ConversationMessage {
            role: "assistant".into(),
            content: ConversationMessageContent::Text(String::new()),
            name: None,
            tool_call_id: None,
            tool_calls: vec![ConversationToolCall {
                id: Some(format!("call-{index}")),
                r#type: "function".into(),
                function: ConversationToolFunction { name: "grep".into(), arguments: "{}".into() },
            }],
        }
    }

    /// A ~`chars`-sized grep-shaped tool result (parseable JSON so stub
    /// reference extraction has real `matches[].file` entries to keep).
    fn tool_result_text(index: usize, padding: usize) -> String {
        format!(
            "{{\"matches\":[{{\"file\":\"src/file{index}.rs\",\"line\":{},\"text\":\"needle\"}}],\"total\":1,\"truncated\":false,\"tail\":\"{}\"}}",
            index + 1,
            "p".repeat(padding)
        )
    }

    fn tool_batch_history(batches: usize, result_padding: usize) -> Vec<ConversationMessage> {
        let mut messages = vec![text_message("system", "sys")];
        for index in 0..batches {
            messages.push(text_message("user", &format!("query {index}")));
            messages.push(tool_call_message(index));
            let mut result = text_message("tool", &tool_result_text(index, result_padding));
            result.tool_call_id = Some(format!("call-{index}"));
            messages.push(result);
        }
        messages.push(text_message("user", "final question"));
        messages
    }

    async fn ledger_window(app: &crate::test_support::TestAppCore, model_id: &str, window: u32) {
        use crate::test_support::ready_local_llama_command;

        app.model_state
            .gpu_scheduler()
            .ledger()
            .note_model_loaded(
                None,
                slab_gpu_memory_scheduler::LedgerEntry {
                    backend: slab_types::RuntimeBackendId::GgmlLlama,
                    model_id: Some(model_id.to_owned()),
                    model_path: format!("{model_id}.gguf"),
                    num_workers: 1,
                    resolved_context_length: Some(window),
                    mmproj_resident: false,
                    weights_bytes: None,
                    mmproj_bytes: None,
                    measured_delta_bytes: None,
                    recorded_at: chrono::Utc::now(),
                },
            )
            .await;
        let model_path = app.model_cache_dir.join(format!("{model_id}.gguf"));
        std::fs::write(&model_path, b"gguf").expect("write model fixture");
        app.model
            .create_model(ready_local_llama_command(model_id, &model_path))
            .await
            .expect("create model");
    }

    fn auto_ctx(model_id: &'static str) -> CompactContext<'static> {
        CompactContext {
            model_id,
            summary_instructions: None,
            force: false,
            progress: None,
            memory_pressure_hint: None,
        }
    }

    /// Acceptance: the deterministic micro tier stubs OLD tool results into
    /// structured placeholders while the most recent batches (and every
    /// user/assistant message) survive verbatim. No LLM call is involved —
    /// no scripted chat is wired, so a wrong escalation into the summarize
    /// path would fail ChatService and fall back to the destructive trim,
    /// failing the verbatim assertions.
    #[tokio::test]
    async fn micro_compact_stubs_old_tool_results_and_keeps_recent_batches() {
        use crate::test_support::TestAppCore;

        let app = TestAppCore::new().await;
        // Window 8192 -> micro ~4505, macro ~6553, keep ~4915.
        ledger_window(&app, "micro-model", 8192).await;
        let port = SummarizingCompactPort::new(app.model_state.clone());

        // 8 batches of ~1k-token grep results -> ~8k tokens: above the micro
        // gate, and stubbing the 3 oldest batches lands below the macro gate.
        let messages = tool_batch_history(8, 3_900);
        let outcome = port.compact(&messages, &auto_ctx("micro-model")).await.expect("compact");
        let CompactOutcome::Replaced { messages: compacted, replaced_messages, output_tokens } =
            outcome
        else {
            panic!("deterministic tier must replace: {outcome:?}");
        };
        assert_eq!(replaced_messages, 0, "no messages removed — only content shrinks");
        assert_eq!(compacted.len(), messages.len(), "message count preserved");
        assert!(output_tokens < port.estimate_tokens(&messages), "estimate must shrink");

        // Batches 0..3 stubbed with the marker, tool name, size and file refs;
        // batches 3..8 verbatim.
        let tool_messages: Vec<String> = compacted
            .iter()
            .filter(|message| message.role == "tool")
            .map(|message| message.content.rendered_text())
            .collect();
        assert_eq!(tool_messages.len(), 8);
        for (batch, text) in tool_messages.iter().enumerate() {
            if batch < 3 {
                let stub: serde_json::Value = serde_json::from_str(text).expect("stub json");
                assert_eq!(
                    stub[STUB_MARKER_FIELD], true,
                    "old batch {batch} must be stubbed: {text}"
                );
                assert_eq!(stub["tool"], "grep");
                assert_eq!(stub["original_bytes"], tool_result_text(batch, 3_900).len());
                assert!(
                    stub["refs"].as_array().is_some_and(|refs| !refs.is_empty()),
                    "grep file refs preserved: {text}"
                );
            } else {
                assert_eq!(
                    *text,
                    tool_result_text(batch, 3_900),
                    "recent batch {batch} must stay verbatim"
                );
            }
        }

        // Conversation text byte-identical; tool_call_ids intact (pairing
        // survives — no orphan tool results can appear).
        for (original, compacted_message) in messages.iter().zip(compacted.iter()) {
            if original.role != "tool" {
                assert_eq!(original.content, compacted_message.content);
            } else {
                assert_eq!(original.tool_call_id, compacted_message.tool_call_id);
            }
        }

        // Task continues: a second pass quiesces (already-stubbed results are
        // skipped by the marker check, everything else is recent or small).
        let second = port.compact(&compacted, &auto_ctx("micro-model")).await.expect("second");
        assert!(matches!(second, CompactOutcome::Skipped { .. }), "must quiesce: {second:?}");
    }

    /// When the deterministic pass alone cannot get below the macro
    /// threshold, the tier escalates to the LLM summarize — over the
    /// micro-compacted set, keeping [system, slab_compact summary, tail].
    #[tokio::test]
    async fn micro_compact_escalates_to_summarize_above_macro_threshold() {
        use crate::domain::ports::RuntimeTextGenerationResponse;
        use crate::test_support::TestAppCore;

        let app = TestAppCore::new().await;
        app.runtime.set_scripted_chat(RuntimeTextGenerationResponse {
            text: "recap of earlier turns".to_owned(),
            ..Default::default()
        });
        // Window 8192: 8 batches x ~3.5k tokens = ~28k. Stubbing 3 batches
        // leaves ~18k — still above the ~6.5k macro gate.
        ledger_window(&app, "escalate-model", 8192).await;
        let port = SummarizingCompactPort::new(app.model_state.clone());

        let messages = tool_batch_history(8, 13_900);
        let outcome = port.compact(&messages, &auto_ctx("escalate-model")).await.expect("compact");
        let CompactOutcome::Replaced { messages: compacted, .. } = outcome else {
            panic!("escalated tier must replace: {outcome:?}");
        };

        // [system, slab_compact summary, trailing window]
        assert_eq!(compacted[0].role, "system");
        assert_eq!(compacted[1].name.as_deref(), Some(SUMMARY_MESSAGE_NAME));
        assert!(compacted[1].rendered_text().contains("recap of earlier turns"));
        assert!(compacted.len() < messages.len(), "summarize must shrink the set");

        // No leading orphan tool results after the swap.
        let ids: Vec<&str> = compacted
            .iter()
            .flat_map(|message| message.tool_calls.iter().filter_map(|call| call.id.as_deref()))
            .collect();
        for message in &compacted {
            if message.role == "tool" {
                let id = message.tool_call_id.as_deref().expect("tool id");
                assert!(ids.contains(&id), "orphan tool result survived: {id}");
            }
        }
    }
}
