use async_trait::async_trait;
use slab_types::{ConversationContentPart, ConversationMessage, ConversationMessageContent};

use crate::error::AgentError;

/// Per-call context handed to [`CompactPort::compact`].
///
/// Pure-local policies (e.g. [`NoopCompactPort`], [`SlidingWindowCompactPort`])
/// ignore every field. The summarizing policy uses `model_id` to route the
/// summarization LLM call and `summary_instructions` to override the default
/// recap prompt. `force` bypasses the policy's threshold (used by manual
/// `/compact`); auto-compaction from the turn loop leaves it `false`.
/// Progress callback fired by a compacting policy right before it begins the
/// (potentially slow) summarization LLM call — after every skip gate has
/// passed. The turn loop installs an impl that emits a `ContextCompacting`
/// notification so the client can show an in-progress indicator. Pure-local
/// policies never invoke it.
pub trait CompactProgress: Send + Sync {
    fn on_compacting<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>>;
}

/// Per-call context handed to [`CompactPort::compact`].
///
/// Pure-local policies (e.g. [`NoopCompactPort`], [`SlidingWindowCompactPort`])
/// ignore every field. The summarizing policy uses `model_id` to route the
/// summarization LLM call and `summary_instructions` to override the default
/// recap prompt. `force` bypasses the policy's threshold (used by manual
/// `/compact`); auto-compaction from the turn loop leaves it `false`.
/// `progress` is fired once a summarization actually begins (auto path only).
///
/// Not `Copy`: `progress` holds an `Arc<dyn CompactProgress>`.
#[derive(Clone, Default)]
pub struct CompactContext<'a> {
    /// Model id used for summarization (ignored by pure-local policies).
    pub model_id: &'a str,
    /// Optional override for the summarization instruction.
    pub summary_instructions: Option<&'a str>,
    /// When `true`, compact unconditionally (manual `/compact`); otherwise the
    /// policy's threshold gates compaction (auto, from the turn loop).
    pub force: bool,
    /// Optional in-progress callback (auto-compaction only).
    pub progress: Option<std::sync::Arc<dyn CompactProgress + 'a>>,
    /// Host-injected memory-pressure hint (0-1 fill ratio, e.g. GPU VRAM).
    /// Opaque to pure-local policies; context-length-aware host policies use
    /// it as an OR-trigger so a memory squeeze compacts history even below
    /// the token threshold. `None` lets the host policy self-query.
    pub memory_pressure_hint: Option<f64>,
}

impl<'a> std::fmt::Debug for CompactContext<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompactContext")
            .field("model_id", &self.model_id)
            .field("summary_instructions", &self.summary_instructions)
            .field("force", &self.force)
            .field("progress", &self.progress.as_ref().map(|_| "<CompactProgress>"))
            .field("memory_pressure_hint", &self.memory_pressure_hint)
            .finish()
    }
}

/// Estimates the token pressure of agent history and optionally compacts it.
#[async_trait]
pub trait CompactPort: Send + Sync {
    /// Human-readable policy name for trace metadata.
    fn policy_name(&self) -> &'static str {
        "custom"
    }

    /// Return the threshold at which compaction should be considered.
    ///
    /// Only advisory for trace metadata when the policy is context-length-aware
    /// (see [`Self::compact`], which performs the real gating decision).
    fn threshold_tokens(&self) -> usize;

    /// Estimate token usage for the current message history.
    fn estimate_tokens(&self, messages: &[ConversationMessage]) -> usize;

    /// Compact messages when implemented by the host.
    ///
    /// Each policy applies its own threshold gate internally (unless `ctx.force`
    /// is set) and returns [`CompactOutcome::Skipped`] when it decides not to
    /// act. This keeps the gating decision co-located with the policy — which
    /// matters for context-length-aware policies that need the model id — so the
    /// turn loop does not have to know how to compute a threshold.
    async fn compact(
        &self,
        _messages: &[ConversationMessage],
        _ctx: &CompactContext<'_>,
    ) -> Result<CompactOutcome, AgentError> {
        Ok(CompactOutcome::Skipped { reason: "no compact provider configured".to_owned() })
    }
}

#[derive(Debug, Clone)]
pub enum CompactOutcome {
    Replaced { messages: Vec<ConversationMessage>, output_tokens: usize, replaced_messages: usize },
    Skipped { reason: String },
}

#[derive(Debug, Clone)]
pub struct NoopCompactPort {
    threshold_tokens: usize,
}

impl NoopCompactPort {
    pub fn new(threshold_tokens: usize) -> Self {
        Self { threshold_tokens }
    }
}

impl Default for NoopCompactPort {
    fn default() -> Self {
        Self::new(16_000)
    }
}

#[async_trait]
impl CompactPort for NoopCompactPort {
    fn policy_name(&self) -> &'static str {
        "noop"
    }

    fn threshold_tokens(&self) -> usize {
        self.threshold_tokens
    }

    fn estimate_tokens(&self, messages: &[ConversationMessage]) -> usize {
        estimate_tokens(messages)
    }

    async fn compact(
        &self,
        _messages: &[ConversationMessage],
        _ctx: &CompactContext<'_>,
    ) -> Result<CompactOutcome, AgentError> {
        Ok(CompactOutcome::Skipped { reason: "noop policy never compacts".to_owned() })
    }
}

#[derive(Debug, Clone)]
pub struct SlidingWindowCompactPort {
    threshold_tokens: usize,
    target_tokens: usize,
}

impl SlidingWindowCompactPort {
    pub fn new(threshold_tokens: usize, target_tokens: usize) -> Self {
        Self { threshold_tokens, target_tokens: target_tokens.min(threshold_tokens) }
    }
}

impl Default for SlidingWindowCompactPort {
    fn default() -> Self {
        Self::new(16_000, 12_000)
    }
}

#[async_trait]
impl CompactPort for SlidingWindowCompactPort {
    fn policy_name(&self) -> &'static str {
        "sliding_window"
    }

    fn threshold_tokens(&self) -> usize {
        self.threshold_tokens
    }

    fn estimate_tokens(&self, messages: &[ConversationMessage]) -> usize {
        estimate_tokens(messages)
    }

    async fn compact(
        &self,
        messages: &[ConversationMessage],
        ctx: &CompactContext<'_>,
    ) -> Result<CompactOutcome, AgentError> {
        if !ctx.force && estimate_tokens(messages) < self.threshold_tokens {
            return Ok(CompactOutcome::Skipped { reason: "below threshold".to_owned() });
        }

        let mut compacted = trailing_window(messages, self.target_tokens);
        remove_leading_orphan_tool_results(&mut compacted);

        if let Some(system) = messages.first().filter(|message| message.role == "system")
            && compacted.first() != Some(system)
        {
            compacted.insert(0, system.clone());
            trim_to_target_after_system(&mut compacted, self.target_tokens);
            remove_leading_orphan_tool_results(&mut compacted);
        }

        if compacted.is_empty() || compacted.len() >= messages.len() {
            return Ok(CompactOutcome::Skipped {
                reason: "sliding window kept the full history".to_owned(),
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

pub fn estimate_tokens(messages: &[ConversationMessage]) -> usize {
    let chars = messages.iter().map(estimate_message_chars).sum::<usize>();
    chars.div_ceil(4)
}

pub fn estimate_message_tokens(message: &ConversationMessage) -> usize {
    estimate_message_chars(message).div_ceil(4)
}

/// Fixed char budget an image part contributes to token estimates. The
/// vision projector encodes images to a bounded embedding sequence — the
/// base64 payload is not prompt text, and counting it serialized a 1MB
/// image as ~342k "tokens", blowing the auto-compaction threshold on a
/// single screenshot. ~1024 tokens at the 4-chars-per-token heuristic.
const IMAGE_PART_CHAR_BUDGET: usize = 4096;

fn estimate_part_chars(part: &ConversationContentPart) -> usize {
    match part {
        ConversationContentPart::Text { text }
        | ConversationContentPart::InputText { text }
        | ConversationContentPart::OutputText { text }
        | ConversationContentPart::Refusal { text } => text.chars().count(),
        ConversationContentPart::Image { .. } => IMAGE_PART_CHAR_BUDGET,
        // Structured text: keep the serialized JSON length (tool results
        // and raw values are real prompt material for the model).
        ConversationContentPart::ToolResult { value, .. }
        | ConversationContentPart::Json { value } => {
            serde_json::to_string(value).map_or(0, |text| text.chars().count())
        }
    }
}

pub fn estimate_message_chars(message: &ConversationMessage) -> usize {
    match &message.content {
        ConversationMessageContent::Text(text) => text.chars().count(),
        ConversationMessageContent::Parts(parts) => {
            parts.iter().map(estimate_part_chars).sum::<usize>()
        }
    }
}

pub fn trailing_window(
    messages: &[ConversationMessage],
    target_tokens: usize,
) -> Vec<ConversationMessage> {
    let mut selected = Vec::new();
    let mut tokens = 0usize;
    for message in messages.iter().rev() {
        let message_tokens = estimate_message_tokens(message);
        if !selected.is_empty() && tokens + message_tokens > target_tokens {
            break;
        }
        tokens += message_tokens;
        selected.push(message.clone());
    }
    selected.reverse();
    selected
}

pub fn remove_leading_orphan_tool_results(messages: &mut Vec<ConversationMessage>) {
    let index =
        if messages.first().is_some_and(|message| message.role == "system") { 1 } else { 0 };
    while messages.get(index).is_some_and(|message| message.role == "tool") {
        messages.remove(index);
    }
}

pub fn trim_to_target_after_system(messages: &mut Vec<ConversationMessage>, target_tokens: usize) {
    while messages.len() > 1 && estimate_tokens(messages) > target_tokens {
        messages.remove(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slab_types::{ConversationContentPart, ConversationMessage};

    fn parts_message(parts: Vec<ConversationContentPart>) -> ConversationMessage {
        ConversationMessage {
            role: "user".to_owned(),
            content: ConversationMessageContent::Parts(parts),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }

    fn image_part(image_url: Option<String>) -> ConversationContentPart {
        ConversationContentPart::Image { image_url, mime_type: None, detail: None }
    }

    #[test]
    fn image_parts_estimate_bounded_instead_of_base64_length() {
        // A ~1MB base64 payload used to count as ~342k tokens.
        let big_payload = "x".repeat(1_000_000);
        let message = parts_message(vec![
            ConversationContentPart::Text { text: "hello".to_owned() },
            image_part(Some(format!("data:image/png;base64,{big_payload}"))),
            image_part(None),
        ]);

        assert_eq!(estimate_message_chars(&message), "hello".len() + 2 * IMAGE_PART_CHAR_BUDGET);
        assert!(estimate_message_tokens(&message) < 2_100);
    }

    #[test]
    fn text_like_parts_count_chars_verbatim() {
        let message = parts_message(vec![
            ConversationContentPart::Text { text: "four".to_owned() },
            ConversationContentPart::InputText { text: "アイウ".to_owned() },
            ConversationContentPart::OutputText { text: String::new() },
            ConversationContentPart::Refusal { text: "x".to_owned() },
        ]);

        // four + three chars + zero + one char, written per-part.
        assert_eq!(estimate_message_chars(&message), "four".chars().count() + 3 + 1);
    }

    #[test]
    fn tool_result_and_json_parts_count_serialized_value() {
        let tool_value = serde_json::json!({"a": 1});
        let json_value = serde_json::json!([1, 2, 3]);
        let expected = serde_json::to_string(&tool_value).unwrap().chars().count()
            + serde_json::to_string(&json_value).unwrap().chars().count();

        let message = parts_message(vec![
            ConversationContentPart::ToolResult {
                tool_call_id: Some("call-1".to_owned()),
                value: tool_value,
            },
            ConversationContentPart::Json { value: json_value },
        ]);

        assert_eq!(estimate_message_chars(&message), expected);
    }
}
