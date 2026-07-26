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
use crate::domain::models::{ChatCompletionCommand, ChatCompletionOutput, CommonChatParams};
use crate::domain::services::chat::ChatService;
use crate::error::AppCoreError;

/// Default fraction of `context_length` at which auto-compaction fires.
const DEFAULT_THRESHOLD_RATIO: f32 = 0.80;
/// Default fraction of `context_length` retained verbatim after compaction
/// (the recent trailing window that is not summarized).
const DEFAULT_TARGET_RATIO: f32 = 0.60;
/// Token budget for the summary completion itself.
const DEFAULT_SUMMARY_MAX_TOKENS: u32 = 512;
/// Fallback fixed threshold (tokens) when a model's context window is unknown.
const DEFAULT_FALLBACK_THRESHOLD_TOKENS: usize = 12_000;

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
    summary_max_tokens: u32,
    fallback_threshold_tokens: usize,
    fallback: SlidingWindowCompactPort,
}

impl SummarizingCompactPort {
    /// Construct with default ratios (threshold 80%, target 60%, summary 512 tokens).
    pub fn new(state: ModelState) -> Self {
        Self {
            state,
            threshold_ratio: DEFAULT_THRESHOLD_RATIO,
            target_ratio: DEFAULT_TARGET_RATIO,
            summary_max_tokens: DEFAULT_SUMMARY_MAX_TOKENS,
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

        if !ctx.force {
            let threshold = match resolve_context_length(&self.state, ctx.model_id).await {
                Some(context_length) if context_length > 0 => {
                    ((context_length as f32) * self.threshold_ratio) as usize
                }
                _ => self.fallback_threshold_tokens,
            };
            if estimate_tokens(messages) < threshold {
                return Ok(CompactOutcome::Skipped { reason: "below threshold".into() });
            }
        }

        let context_length = resolve_context_length(&self.state, ctx.model_id).await;
        let keep_target = match context_length {
            Some(context_length) if context_length > 0 => {
                ((context_length as f32) * self.target_ratio) as usize
            }
            _ => self.fallback_threshold_tokens,
        };

        let system_msg = messages.first().filter(|message| message.role == "system").cloned();
        let system_end = if system_msg.is_some() { 1 } else { 0 };
        let keep_start = recent_window_start(messages, keep_target).max(system_end);

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
                max_tokens: Some(self.summary_max_tokens),
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
    let ctx = CompactContext { model_id, summary_instructions: None, force };
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

/// Render a slice of messages as a flat `role: text` transcript for summarization.
fn render_transcript(messages: &[ConversationMessage]) -> String {
    messages
        .iter()
        .map(|message| format!("{}: {}", message.role, message.rendered_text()))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Best-effort resolution of a model's context window (tokens).
///
/// 1. The model catalog's recorded `context_window` (covers cloud + local with
///    a manifest entry). 2. The configured local llama per-seq context length.
/// 3. `None` when neither is known.
async fn resolve_context_length(state: &ModelState, model_id: &str) -> Option<u32> {
    if let Some(context_window) =
        crate::domain::services::model::context_window_for(state, model_id).await
    {
        return Some(context_window);
    }
    state.pmid().config().runtime.llama.context_length
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
}
